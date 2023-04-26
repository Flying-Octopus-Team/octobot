use std::fmt::Display;

use crate::database::pagination::Paginate;
use crate::database::schema::member;
use crate::database::PG_POOL;
use crate::diesel::ExpressionMethods;
use crate::diesel::RunQueryDsl;
use crate::discord::Context;
use crate::discord::Error;
use crate::SETTINGS;
use diesel::backend;
use diesel::backend::Backend;
use diesel::deserialize::FromSql;
use diesel::serialize::Output;
use diesel::serialize::ToSql;
use diesel::sql_types::Integer;
use diesel::QueryDsl;
use diesel::Table;
use poise::serenity_prelude as serenity;
use poise::SlashArgument;
use serenity::http::CacheHttp;
use serenity::model::prelude::RoleId;
use tracing::error;
use uuid::Uuid;

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Debug, Eq)]
#[diesel(table_name = member)]
pub struct Member {
    id: Uuid,
    display_name: String,
    discord_id: Option<String>,
    trello_id: Option<String>,
    trello_report_card_id: Option<String>,
    role: MemberRole,
    wiki_id: Option<i64>,
}

#[derive(
    Copy, Clone, Default, Debug, FromSqlRow, PartialEq, Eq, AsExpression, poise::ChoiceParameter,
)]
#[diesel(sql_type = diesel::sql_types::Integer)]
pub enum MemberRole {
    #[name = "Ex-Member"]
    ExMember = -1,
    #[name = "Member"]
    #[default]
    Member = 0,
    #[name = "Apprentice"]
    Apprentice = 1, // if you add more roles, make sure to update the FromSql implementation below
}

impl MemberRole {
    pub fn discord_role(&self) -> Option<RoleId> {
        match self {
            MemberRole::Member => Some(SETTINGS.discord.member_role),
            MemberRole::Apprentice => Some(SETTINGS.discord.apprentice_role),
            MemberRole::ExMember => None,
        }
    }

    pub async fn add_role(&self, cache_http: &impl CacheHttp, member_id: u64) -> Result<(), Error> {
        if let Some(role_id) = self.discord_role() {
            cache_http
                .http()
                .add_member_role(SETTINGS.discord.server_id.0, member_id, role_id.0, None)
                .await?;
        }

        Ok(())
    }

    pub async fn remove_role(
        &self,
        cache_http: &impl CacheHttp,
        member_id: u64,
    ) -> Result<(), Error> {
        if let Some(role_id) = self.discord_role() {
            cache_http
                .http()
                .remove_member_role(SETTINGS.discord.server_id.0, member_id, role_id.0, None)
                .await?;
        }

        Ok(())
    }

    pub async fn swap_roles(
        add_role: MemberRole,
        remove_role: MemberRole,
        cache_http: &impl CacheHttp,
        member_id: u64,
    ) -> Result<(), Error> {
        add_role.add_role(cache_http, member_id).await?;
        remove_role.remove_role(cache_http, member_id).await?;
        Ok(())
    }
}

impl<DB> FromSql<Integer, DB> for MemberRole
where
    DB: Backend,
    i32: FromSql<Integer, DB>,
{
    fn from_sql(bytes: backend::RawValue<DB>) -> diesel::deserialize::Result<Self> {
        match i32::from_sql(bytes)? {
            -1 => Ok(MemberRole::ExMember),
            0 => Ok(MemberRole::Member),
            1 => Ok(MemberRole::Apprentice),
            x => Err(format!("Unrecognized member role: {}", x).into()),
        }
    }
}

impl<DB> ToSql<Integer, DB> for MemberRole
where
    DB: Backend,
    i32: ToSql<Integer, DB>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> diesel::serialize::Result {
        match self {
            MemberRole::ExMember => (-1).to_sql(out),
            MemberRole::Member => 0.to_sql(out),
            MemberRole::Apprentice => 1.to_sql(out),
        }
    }
}

impl Member {
    pub fn new(
        display_name: String,
        discord_id: Option<String>,
        trello_id: Option<String>,
        trello_report_card_id: Option<String>,
        role: MemberRole,
        wiki_id: Option<i64>,
    ) -> Member {
        Member {
            id: Uuid::new_v4(),
            display_name,
            discord_id,
            trello_id,
            trello_report_card_id,
            role,
            wiki_id,
        }
    }

    pub fn insert(&self) -> Result<Self, Error> {
        Ok(diesel::insert_into(member::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self, Error> {
        Ok(diesel::update(self)
            .set(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    /// Sets users role to Ex-member and removes their discord role
    pub fn delete(&self) -> Result<bool, Error> {
        use crate::database::schema::member::dsl::*;

        Ok(diesel::update(member.filter(id.eq(self.id)))
            .set((
                role.eq(MemberRole::ExMember),
                discord_id.eq(None::<String>),
                trello_id.eq(None::<String>),
                trello_report_card_id.eq(None::<String>),
            ))
            .execute(&mut PG_POOL.get()?)
            .map(|rows| rows != 0)?)
    }

    pub async fn unassign_wiki_group(&self, group_id: i64) -> Result<(), Error> {
        crate::wiki::unassign_user_group(crate::wiki::unassign_user_group::Variables {
            user_id: self.wiki_id.expect("User must have a wiki id"),
            group_id,
        })
        .await
    }

    pub async fn assign_wiki_group(&self, group_id: i64) -> Result<(), Error> {
        crate::wiki::assign_user_group(crate::wiki::assign_user_group::Variables {
            user_id: self.wiki_id.expect("User must have a wiki id"),
            group_id,
        })
        .await
    }

    pub fn hard_delete(&self) -> Result<bool, Error> {
        use crate::database::schema::member::dsl::*;

        Ok(diesel::delete(member.filter(id.eq(self.id)))
            .execute(&mut PG_POOL.get()?)
            .map(|rows| rows != 0)?)
    }

    pub fn list(page: i64, per_page: Option<i64>) -> Result<(Vec<Self>, i64), Error> {
        use crate::database::schema::member::dsl::*;

        let mut query = member
            .select(member::all_columns())
            .into_boxed()
            .paginate(page);

        if let Some(per_page) = per_page {
            query = query.per_page(per_page);
        };

        let (vec, total_pages) = query.load_and_count_pages(&mut PG_POOL.get().unwrap())?;
        Ok((vec, total_pages))
    }

    pub fn find_by_id(find_id: impl Into<Uuid>) -> Result<Self, Error> {
        use crate::database::schema::member::dsl::*;

        let uuid = find_id.into();

        Ok(member.find(uuid).get_result(&mut PG_POOL.get()?)?)
    }

    pub fn find_by_discord_id(find_id: impl Into<String>) -> Result<Self, Error> {
        use crate::database::schema::member::dsl::*;

        let dc_id = find_id.into();

        Ok(member
            .filter(discord_id.eq(dc_id))
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn discord_id(&self) -> Option<&String> {
        self.discord_id.as_ref()
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub(crate) fn from_discord_id(user_id: String, ctx: Context<'_>) -> Result<Self, Error> {
        let member_id = match user_id.parse::<u64>() {
            Ok(id) => id,
            Err(_) => {
                let error_msg = format!("Invalid member id: {}", user_id);
                error!("{}", error_msg);
                return Err(anyhow!(error_msg));
            }
        };
        let guild_member = match ctx
            .cache()
            .unwrap()
            .member(SETTINGS.discord.server_id, member_id)
        {
            Some(guild_member) => guild_member,
            None => {
                let error_msg = format!("Member not found in the guild: {}", member_id);
                error!("{}", error_msg);
                return Err(anyhow!(error_msg));
            }
        };

        let result = match Member::find_by_discord_id(guild_member.user.id.to_string()) {
            Ok(result) => result,
            Err(why) => {
                let error_msg = format!(
                    "Member not found in database: {}\nReason: {}",
                    member_id, why
                );
                error!("{}", error_msg);
                return Err(anyhow!(error_msg));
            }
        };

        Ok(result)
    }

    pub(crate) fn name(&self) -> String {
        self.display_name.clone()
    }

    pub(crate) fn set_name(&mut self, new_name: String) -> Result<(), Error> {
        self.display_name = new_name;

        match self.update() {
            Ok(_) => Ok(()),
            Err(why) => {
                let error_msg = format!("Failed to update member name: {}", why);
                error!("{}", error_msg);
                Err(anyhow!(error_msg))
            }
        }
    }

    pub fn set_discord_id(&mut self, new_id: String) -> Result<(), Error> {
        self.discord_id = Some(new_id);

        match self.update() {
            Ok(_) => Ok(()),
            Err(why) => {
                let error_msg = format!("Failed to update member discord id: {}", why);
                error!("{}", error_msg);
                Err(anyhow!(error_msg))
            }
        }
    }

    pub fn set_trello_id(&mut self, new_id: String) -> Result<(), Error> {
        self.trello_id = Some(new_id);

        match self.update() {
            Ok(_) => Ok(()),
            Err(why) => {
                let error_msg = format!("Failed to update member trello id: {}", why);
                error!("{}", error_msg);
                Err(anyhow!(error_msg))
            }
        }
    }

    pub fn set_trello_report_card_id(&mut self, new_id: String) -> Result<(), Error> {
        self.trello_report_card_id = Some(new_id);

        match self.update() {
            Ok(_) => Ok(()),
            Err(why) => {
                let error_msg = format!("Failed to update member trello report card id: {}", why);
                error!("{}", error_msg);
                Err(anyhow!(error_msg))
            }
        }
    }

    pub fn set_role(&mut self, new_role: MemberRole) -> Result<(), Error> {
        self.role = new_role;

        match self.update() {
            Ok(_) => Ok(()),
            Err(why) => {
                let error_msg = format!("Failed to update member role: {}", why);
                error!("{}", error_msg);
                Err(anyhow!(error_msg))
            }
        }
    }

    pub fn set_wiki_id(&mut self, new_id: i64) -> Result<(), Error> {
        self.wiki_id = Some(new_id);

        match self.update() {
            Ok(_) => Ok(()),
            Err(why) => {
                let error_msg = format!("Failed to update member wiki id: {}", why);
                error!("{}", error_msg);
                Err(anyhow!(error_msg))
            }
        }
    }

    pub fn role(&self) -> MemberRole {
        self.role
    }

    pub fn wiki_id(&self) -> Option<i64> {
        self.wiki_id
    }
}

impl Display for Member {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let discord_id = if let Some(discord_id) = &self.discord_id {
            discord_id.to_string()
        } else {
            "None".to_string()
        };
        let trello_id = if let Some(trello_id) = &self.trello_id {
            trello_id.to_string()
        } else {
            "None".to_string()
        };
        let trello_report_card_id = if let Some(trello_report_card_id) = &self.trello_report_card_id
        {
            trello_report_card_id.to_string()
        } else {
            "None".to_string()
        };

        let wiki_id = if let Some(wiki_id) = self.wiki_id {
            wiki_id.to_string()
        } else {
            "None".to_string()
        };
        write!(
            f,
            "Member: {} ({}) Discord ID: {}, Trello ID: {}, Trello Report Card ID: {}, Member Role: {}, Wiki ID: {}",
            self.display_name, self.id.simple(), discord_id, trello_id, trello_report_card_id, self.role, wiki_id
        )
    }
}

impl PartialEq for Member {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[async_trait::async_trait]
impl SlashArgument for Member {
    async fn extract(
        _ctx: &serenity::Context,
        _interaction: poise::ApplicationCommandOrAutocompleteInteraction<'_>,
        value: &serenity::json::Value,
    ) -> Result<Self, poise::SlashArgError> {
        let id = match value {
            serenity::json::Value::String(id) => match Uuid::parse_str(id) {
                Ok(id) => id,
                Err(why) => {
                    let error_msg = format!("Failed to parse member id: {}", id);
                    error!("{}", error_msg);
                    return Err(poise::SlashArgError::Parse {
                        error: Box::new(why),
                        input: id.to_string(),
                    });
                }
            },
            _ => {
                return Err(poise::SlashArgError::CommandStructureMismatch(
                    "Member id must be a string",
                ))
            }
        };

        let member = match Member::find_by_id(id) {
            Ok(member) => member,
            Err(why) => {
                let error_msg = format!("Failed to get member: {}", why);
                error!("{}", error_msg);
                return Err(poise::SlashArgError::Parse {
                    error: why.into(),
                    input: id.to_string(),
                });
            }
        };

        Ok(member)
    }

    fn create(builder: &mut serenity::CreateApplicationCommandOption) {
        builder.kind(serenity::command::CommandOptionType::String);
    }
}
