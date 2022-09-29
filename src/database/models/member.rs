use std::fmt::Display;

use crate::database::pagination::Paginate;
use crate::database::schema::member;
use crate::database::PG_POOL;
use crate::diesel::ExpressionMethods;
use crate::diesel::RunQueryDsl;
use crate::discord::find_option_as_string;
use crate::SETTINGS;
use diesel::QueryDsl;
use diesel::Table;
use serenity::model::application::interaction::application_command::CommandDataOption;
use serenity::prelude::Context;
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
}

impl Member {
    pub fn new(
        display_name: String,
        discord_id: Option<String>,
        trello_id: Option<String>,
        trello_report_card_id: Option<String>,
    ) -> Member {
        Member {
            id: Uuid::new_v4(),
            display_name,
            discord_id,
            trello_id,
            trello_report_card_id,
        }
    }

    pub fn insert(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::insert_into(member::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::update(self)
            .set(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn delete(&self) -> Result<bool, Box<dyn std::error::Error>> {
        use crate::database::schema::member::dsl::*;

        Ok(diesel::delete(member.filter(id.eq(self.id)))
            .execute(&mut PG_POOL.get()?)
            .map(|rows| rows != 0)?)
    }

    pub fn list(
        page: i64,
        per_page: Option<i64>,
    ) -> Result<(Vec<Self>, i64), Box<dyn std::error::Error>> {
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

    pub fn find_by_id(find_id: impl Into<Uuid>) -> Result<Self, Box<dyn std::error::Error>> {
        use crate::database::schema::member::dsl::*;

        let uuid = find_id.into();

        Ok(member.find(uuid).get_result(&mut PG_POOL.get()?)?)
    }

    pub fn find_by_discord_id(
        find_id: impl Into<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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

    pub(crate) fn from_discord_id(
        user_id: String,
        ctx: &Context,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let member_id = match user_id.parse::<u64>() {
            Ok(id) => id,
            Err(_) => {
                let error_msg = format!("Invalid member id: {}", user_id);
                error!("{}", error_msg);
                return Err(error_msg.into());
            }
        };
        let guild_member = match ctx.cache.member(SETTINGS.server_id, member_id) {
            Some(guild_member) => guild_member,
            None => {
                let error_msg = format!("Member not found in the guild: {}", member_id);
                error!("{}", error_msg);
                return Err(error_msg.into());
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
                return Err(error_msg.into());
            }
        };

        Ok(result)
    }

    pub(crate) fn name(&self) -> String {
        self.display_name.clone()
    }

    pub(crate) fn set_name(&mut self, new_name: String) -> Result<(), Box<dyn std::error::Error>> {
        self.display_name = new_name;

        match self.update() {
            Ok(_) => Ok(()),
            Err(why) => {
                let error_msg = format!("Failed to update member name: {}", why);
                error!("{}", error_msg);
                Err(error_msg.into())
            }
        }
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
        write!(
            f,
            "Member {}: {}, Discord ID: {}, Trello ID: {}, Trello Report Card ID: {}",
            self.display_name, self.id, discord_id, trello_id, trello_report_card_id
        )
    }
}

impl PartialEq for Member {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl From<&[CommandDataOption]> for Member {
    fn from(options: &[CommandDataOption]) -> Self {
        let id = match find_option_as_string(options, "id") {
            Some(id) => Uuid::parse_str(&id).unwrap(),
            None => Uuid::new_v4(),
        };
        let discord_id = find_option_as_string(options, "discord_id");
        let trello_id = find_option_as_string(options, "trello_id");
        let trello_report_card_id = find_option_as_string(options, "trello_report_card");
        let display_name = match find_option_as_string(options, "display_name") {
            Some(display_name) => display_name,
            None => "None".to_string(),
        };

        Member {
            id,
            display_name,
            discord_id,
            trello_id,
            trello_report_card_id,
        }
    }
}
