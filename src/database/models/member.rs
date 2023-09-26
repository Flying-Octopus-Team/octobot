use std::fmt::Display;

use diesel::{
    backend::Backend,
    deserialize::FromSql,
    query_dsl::SaveChangesDsl,
    serialize::{Output, ToSql},
    sql_types::Integer,
    BoolExpressionMethods, QueryDsl, Table,
};
use poise::{serenity_prelude as serenity, SlashArgument};
use serenity::{http::CacheHttp, model::prelude::RoleId};
use tracing::{error, warn};
use uuid::Uuid;

use crate::{
    database::{pagination::Paginate, schema::member, PG_POOL},
    diesel::{ExpressionMethods, RunQueryDsl},
    error::Error,
    SETTINGS,
};

// Seconds in a day
const SECONDS_IN_DAY: i64 = 86_400;

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
    last_activity: Option<chrono::NaiveDate>,
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
    Apprentice = 1, /* if you add more roles, make sure to update the FromSql and ToSql
                     * implementation below */
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, poise::ChoiceParameter)]
pub enum Activity {
    Active,
    Inactive,
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
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
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
            last_activity: None,
        }
    }

    pub fn insert(&self) -> Result<Self, Error> {
        Ok(diesel::insert_into(member::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self, Error> {
        Ok(self.save_changes(&mut PG_POOL.get()?)?)
    }

    pub fn hard_delete(&self) -> Result<usize, Error> {
        use crate::database::schema::member::dsl::*;

        Ok(diesel::delete(member.filter(id.eq(self.id))).execute(&mut PG_POOL.get()?)?)
    }

    /// Sets users role to Ex-member and removes their discord role
    pub fn delete(&self) -> Result<usize, Error> {
        use crate::database::schema::member::dsl::*;

        Ok(diesel::update(member.filter(id.eq(self.id)))
            .set((
                role.eq(MemberRole::ExMember),
                discord_id.eq(None::<String>),
                trello_id.eq(None::<String>),
                trello_report_card_id.eq(None::<String>),
            ))
            .execute(&mut PG_POOL.get()?)?)
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

    /// Assigns member appropriate group on wiki. This should be used when
    /// assigning a member role to a user. This function will also remove
    /// the guest group if the user had one.
    pub async fn assign_wiki_group_by_role(&self) -> Result<(), Error> {
        let group_id = self.wiki_group();

        self.assign_wiki_group(group_id).await?;

        Ok(())
    }

    /// Unassigns member appropriate group on wiki. This should be used when
    /// unassigning a member role to a user. This function will also remove
    /// the guest group if the user had one.
    pub async fn unassign_wiki_group_by_role(&self) -> Result<(), Error> {
        let group_id = self.wiki_group();

        self.unassign_wiki_group(group_id).await?;

        Ok(())
    }

    /// Returns wiki group id based on member role
    pub fn wiki_group(&self) -> i64 {
        match self.role {
            MemberRole::Member => SETTINGS.wiki.member_group_id,
            MemberRole::Apprentice => SETTINGS.wiki.member_group_id,
            MemberRole::ExMember => SETTINGS.wiki.guest_group_id,
        }
    }

    pub fn list(
        page: i64,
        per_page: Option<i64>,
        role: Option<MemberRole>,
        activity: Option<Activity>,
    ) -> Result<(Vec<Self>, i64), Error> {
        use crate::database::schema::member::dsl;

        let mut query = dsl::member
            .select(dsl::member::all_columns())
            .order(dsl::display_name.asc())
            .into_boxed();

        if let Some(role) = role {
            query = query.filter(dsl::role.eq(role));
        }

        if let Some(activity) = activity {
            match activity {
                Activity::Active => {
                    query = query.filter(
                        dsl::last_activity.gt((chrono::Utc::now().naive_utc()
                            - chrono::Duration::seconds(
                                SECONDS_IN_DAY * SETTINGS.activity_threshold_days,
                            ))
                        .date()),
                    );
                }
                Activity::Inactive => {
                    query = query.filter(
                        dsl::last_activity
                            .le((chrono::Utc::now().naive_utc()
                                - chrono::Duration::seconds(
                                    SECONDS_IN_DAY * SETTINGS.activity_threshold_days,
                                ))
                            .date())
                            .or(dsl::last_activity.is_null()),
                    );
                }
            }
        }

        let mut query = query.paginate(page);

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

    /// Refreshes every member's activity by checking their last report or attended meeting
    /// and updating their last activity date
    pub fn refresh_all_activities() -> Result<(), Error> {
        use crate::database::schema::member::dsl;

        let members = dsl::member
            .select(dsl::member::all_columns())
            .load::<Member>(&mut PG_POOL.get()?)?;

        for mut member in members {
            member.refresh_activity()?;
        }

        Ok(())
    }

    pub fn refresh_activity(&mut self) -> Result<(), Error> {
        use crate::database::schema::{
            meeting::dsl as meeting_dsl, meeting_members::dsl as meeting_members_dsl,
            report::dsl as report_dsl,
        };

        let report_date = report_dsl::report
            .select(diesel::dsl::max(report_dsl::create_date))
            .filter(report_dsl::member_id.eq(self.id))
            .get_result::<Option<chrono::NaiveDate>>(&mut PG_POOL.get()?)?;

        let meeting_date = meeting_dsl::meeting
            .select(diesel::dsl::max(meeting_dsl::end_date))
            .inner_join(meeting_members_dsl::meeting_members)
            .filter(meeting_members_dsl::member_id.eq(self.id))
            .get_result::<Option<chrono::NaiveDateTime>>(&mut PG_POOL.get()?)?
            .map(|dt| dt.date());

        let last_activity = match (report_date, meeting_date) {
            (Some(report_date), Some(meeting_date)) => report_date.max(meeting_date),
            (Some(report_date), None) => report_date,
            (None, Some(meeting_date)) => meeting_date,
            (None, None) => {
                warn!("Member {} has no report or meeting", self.id);
                return Ok(());
            }
        };

        self.last_activity = Some(last_activity);

        self.update()?;

        Ok(())
    }

    /// Checks if given date is newer than last activity date. If it is, it will update
    /// the last activity date. If it is not, it will do nothing.
    ///
    /// This function returns true if the last activity date was updated, false otherwise.
    pub fn update_activity(&mut self, date: chrono::NaiveDate) -> Result<bool, Error> {
        if let Some(last_activity) = self.last_activity {
            if date > last_activity {
                self.set_last_activity(date);
                self.update()?;

                return Ok(true);
            }
        } else {
            self.set_last_activity(date);
            self.update()?;

            return Ok(true);
        }

        Ok(false)
    }

    pub fn discord_id(&self) -> Option<&String> {
        self.discord_id.as_ref()
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub(crate) fn name(&self) -> String {
        self.display_name.clone()
    }

    pub(crate) fn set_name(&mut self, new_name: String) -> Result<(), Error> {
        self.display_name = new_name;

        self.update()?;

        Ok(())
    }

    pub fn set_discord_id(&mut self, new_id: String) {
        self.discord_id = Some(new_id);
    }

    pub fn set_trello_id(&mut self, new_id: String) {
        self.trello_id = Some(new_id);
    }

    pub fn set_trello_report_card_id(&mut self, new_id: String) {
        self.trello_report_card_id = Some(new_id);
    }

    pub fn set_role(&mut self, new_role: MemberRole) {
        self.role = new_role;
    }

    pub fn set_wiki_id(&mut self, new_id: i64) {
        self.wiki_id = Some(new_id);
    }

    pub fn set_last_activity(&mut self, new_activity: chrono::NaiveDate) {
        self.last_activity = Some(new_activity);
    }

    pub fn role(&self) -> MemberRole {
        self.role
    }

    pub fn wiki_id(&self) -> Option<i64> {
        self.wiki_id
    }

    pub fn last_activity(&self) -> Option<chrono::NaiveDate> {
        self.last_activity
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

        let activity = if let Some(last_activity) = self.last_activity {
            last_activity.to_string()
        } else {
            "Never".to_string()
        };

        write!(
            f,
            "{} <@{}> ({}) Last active: {}, Trello ID: {}, Trello Report Card ID: {}, Wiki ID: {}",
            self.role,
            discord_id,
            self.id.simple(),
            activity,
            trello_id,
            trello_report_card_id,
            wiki_id
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
        ctx: &serenity::Context,
        interaction: poise::ApplicationCommandOrAutocompleteInteraction<'_>,
        value: &serenity::json::Value,
    ) -> Result<Self, poise::SlashArgError> {
        let member_id = match poise::extract_slash_argument!(
            serenity::model::guild::Member,
            ctx,
            interaction,
            value
        )
        .await
        {
            Ok(member) => member.user.id.to_string(),
            Err(why) => {
                String::from(value.as_str().ok_or_else(|| poise::SlashArgError::Parse {
                    error: why.into(),
                    input: value.to_string(),
                })?)
            }
        };

        let member = match Member::find_by_discord_id(&member_id) {
            Ok(member) => member,
            Err(why) => {
                let error_msg = format!("Could not find member in database: {}", why);
                error!("{}", error_msg);
                return Err(poise::SlashArgError::Parse {
                    error: why.into(),
                    input: member_id,
                });
            }
        };

        Ok(member)
    }

    fn create(builder: &mut serenity::CreateApplicationCommandOption) {
        builder.kind(serenity::command::CommandOptionType::User);
    }
}
