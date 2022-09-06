use std::fmt::Display;

use crate::database::pagination::Paginate;
use crate::database::schema::member;
use crate::diesel::ExpressionMethods;
use crate::diesel::RunQueryDsl;
use crate::discord::find_option_as_string;
use diesel::QueryDsl;
use diesel::Table;
use serenity::model::application::interaction::application_command::CommandDataOption;
use uuid::Uuid;

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Debug)]
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
        Ok(diesel::update(&self)
            .set(self)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    pub fn delete(&self) -> Result<bool, Box<dyn std::error::Error>> {
        use crate::database::schema::member::dsl::*;

        Ok(diesel::delete(member.filter(id.eq(id)))
            .execute(&mut crate::database::PG_POOL.get()?)
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

        let (vec, total_pages) =
            query.load_and_count_pages(&mut crate::database::PG_POOL.get().unwrap())?;
        Ok((vec, total_pages))
    }

    pub fn find_by_id(find_id: impl Into<Uuid>) -> Result<Self, Box<dyn std::error::Error>> {
        use crate::database::schema::member::dsl::*;

        let uuid = find_id.into();

        Ok(member
            .find(uuid)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    pub fn find_by_discord_id(
        find_id: impl Into<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        use crate::database::schema::member::dsl::*;

        let dc_id = find_id.into();

        Ok(member
            .filter(discord_id.eq(dc_id))
            .get_result(&mut crate::database::PG_POOL.get()?)?)
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

    pub(crate) fn set_name(&mut self, new_name: String) {
        self.display_name = new_name;
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
