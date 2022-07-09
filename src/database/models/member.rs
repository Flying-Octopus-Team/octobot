use std::fmt::Display;

use crate::database::schema::member;
use crate::diesel::ExpressionMethods;
use crate::diesel::RunQueryDsl;
use crate::discord::find_option_as_string;
use diesel::QueryDsl;
use serenity::model::interactions::application_command::ApplicationCommandInteractionDataOption;
use uuid::Uuid;

#[derive(Queryable, Insertable, AsChangeset, Debug)]
#[diesel(table_name = member)]
pub struct Member {
    id: Uuid,
    discord_id: Option<String>,
    trello_id: Option<String>,
    trello_report_card_id: Option<String>,
}

impl Member {
    pub fn new(
        discord_id: Option<String>,
        trello_id: Option<String>,
        trello_report_card_id: Option<String>,
    ) -> Member {
        Member {
            id: Uuid::new_v4(),
            discord_id,
            trello_id,
            trello_report_card_id,
        }
    }

    pub fn insert(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::insert_into(member::table)
            .values(self)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::update(member::table)
            .set(self)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    pub fn delete(&self) -> Result<bool, Box<dyn std::error::Error>> {
        use crate::database::schema::member::dsl::*;

        Ok(diesel::delete(member.filter(id.eq(id)))
            .execute(&mut crate::database::PG_POOL.get().unwrap())
            .map(|_| true)?)
    }

    pub fn find_by_id(find_id: impl Into<Uuid>) -> Result<Self, Box<dyn std::error::Error>> {
        use crate::database::schema::member::dsl::*;

        let uuid = find_id.into();

        Ok(member
            .filter(id.eq(uuid))
            .get_result(&mut crate::database::PG_POOL.get().unwrap())?)
    }

    pub fn discord_id(&self) -> Option<&String> {
        self.discord_id.as_ref()
    }

    pub fn id(&self) -> Uuid {
        self.id
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
            "Member: {}, discord_id: {}, trello_id: {}, trello_report_card: {}",
            self.id, discord_id, trello_id, trello_report_card_id
        )
    }
}

impl From<&[ApplicationCommandInteractionDataOption]> for Member {
    fn from(options: &[ApplicationCommandInteractionDataOption]) -> Self {
        let id = match find_option_as_string(options, "id") {
            Some(id) => Uuid::parse_str(&id).unwrap(),
            None => Uuid::new_v4(),
        };
        let discord_id = find_option_as_string(options, "discord_id");
        let trello_id = find_option_as_string(options, "trello_id");
        let trello_report_card_id = find_option_as_string(options, "trello_report_card");

        Member {
            id,
            discord_id,
            trello_id,
            trello_report_card_id,
        }
    }
}
