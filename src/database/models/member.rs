use anyhow::Result;
use diesel::pg::Pg;
use diesel::query_dsl::SaveChangesDsl;
use diesel::result::OptionalExtension;
use diesel::QueryDsl;
use serenity::model::application::interaction::application_command::CommandDataOption;
use std::fmt::Display;
use uuid::Uuid;

use crate::database::pagination::Paginate;
use crate::database::pagination::Paginated;
use crate::database::schema::member;
use crate::database::schema::member::BoxedQuery;
use crate::database::PG_POOL;
use crate::diesel::ExpressionMethods;
use crate::diesel::RunQueryDsl;
use crate::discord::find_option_as_string;
use crate::discord::find_option_value;

type AllColumns = (
    member::id,
    member::display_name,
    member::discord_id,
    member::trello_id,
    member::trello_report_card_id,
    member::is_apprentice,
);

const ALL_COLUMNS: AllColumns = (
    member::id,
    member::display_name,
    member::discord_id,
    member::trello_id,
    member::trello_report_card_id,
    member::is_apprentice,
);

type All = diesel::dsl::Select<crate::database::schema::member::table, AllColumns>;

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Selectable, Debug, Eq)]
#[diesel(table_name = member)]
pub struct Member {
    id: Uuid,
    pub display_name: String,
    pub discord_id: Option<String>,
    pub trello_id: Option<String>,
    pub trello_report_card_id: Option<String>,
    pub is_apprentice: bool,
}

impl Member {
    pub fn new(
        display_name: String,
        discord_id: Option<String>,
        trello_id: Option<String>,
        trello_report_card_id: Option<String>,
        is_apprentice: bool,
    ) -> Member {
        Member {
            id: Uuid::new_v4(),
            display_name,
            discord_id,
            trello_id,
            trello_report_card_id,
            is_apprentice,
        }
    }

    pub fn all() -> All {
        member::table.select(ALL_COLUMNS)
    }

    pub fn insert(&self) -> Result<Self> {
        Ok(diesel::insert_into(member::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self> {
        Ok(self.save_changes(&mut PG_POOL.get()?)?)
    }

    pub fn delete(&self) -> Result<bool> {
        use crate::database::schema::member::dsl::*;

        Ok(diesel::delete(member.filter(id.eq(self.id)))
            .execute(&mut PG_POOL.get()?)
            .map(|rows| rows != 0)?)
    }

    /// Paginates the query.
    ///
    /// Returns wrapped query in a `Paginated` struct.
    pub fn paginate(
        query: BoxedQuery<'_, Pg>,
        page: i64,
        per_page: Option<i64>,
    ) -> Paginated<BoxedQuery<'_, Pg>> {
        let mut query = query.paginate(page);

        if let Some(per_page) = per_page {
            query = query.per_page(per_page);
        };

        query
    }

    pub fn find_by_id(find_id: impl Into<Uuid>) -> Result<Self> {
        use crate::database::schema::member::dsl::*;

        let uuid = find_id.into();

        Ok(member.find(uuid).get_result(&mut PG_POOL.get()?)?)
    }

    pub fn find_by_discord_id(find_id: impl Into<String>) -> Result<Option<Self>> {
        use crate::database::schema::member::dsl::*;

        let dc_id = find_id.into();

        Ok(member
            .filter(discord_id.eq(dc_id))
            .get_result(&mut PG_POOL.get()?)
            .optional()?)
    }

    pub(crate) fn find_by_trello_id(find_id: impl Into<String>) -> Result<Option<Self>> {
        use crate::database::schema::member::dsl::*;

        let find_id = find_id.into();

        Ok(member
            .filter(trello_id.eq(find_id))
            .get_result(&mut PG_POOL.get()?)
            .optional()?)
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
            Ok(result) => match result {
                Some(member) => member,
                None => {
                    let error_msg = format!("Member not found in database: {}", member_id);
                    error!("{}", error_msg);
                    return Err(error_msg.into());
                }
            },
            Err(why) => {
                let error_msg = format!(
                    "Error while finding member in database: {}\nReason: {}",
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
}

impl Display for Member {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let discord_id = match &self.discord_id {
            Some(discord_id) => discord_id,
            None => "None",
        };

        let trello_id = match &self.trello_id {
            Some(trello_id) => trello_id,
            _ => "None",
        };

        let trello_report_card_id = match &self.trello_report_card_id {
            Some(trello_report_card_id) => trello_report_card_id,
            _ => "None",
        };

        write!(
            f,
            "Member: {} ({}) Discord ID: {}, Trello ID: {}, Trello Report Card ID: {}, Apprentice: {}",
            self.display_name, self.id, discord_id, trello_id, trello_report_card_id, self.is_apprentice
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
        let discord_id = find_option_as_string(options, "discord-id");
        let trello_id = find_option_as_string(options, "trello-id");
        let trello_report_card_id = find_option_as_string(options, "trello-report-card");
        let display_name = match find_option_as_string(options, "display-name") {
            Some(display_name) => display_name,
            None => "None".to_string(),
        };
        let is_apprentice = match find_option_value(options, "is-apprentice") {
            Some(is_apprentice) => is_apprentice.as_bool().unwrap(),
            None => false,
        };

        Member {
            id,
            display_name,
            discord_id,
            trello_id,
            trello_report_card_id,
            is_apprentice,
        }
    }
}

impl From<crate::framework::member::Member> for Member {
    fn from(mem: crate::framework::member::Member) -> Self {
        Self {
            id: mem.id,
            display_name: mem.display_name,
            discord_id: mem.discord_user.map(|user| user.id.to_string()),
            trello_id: mem.trello_id,
            trello_report_card_id: mem.trello_report_card_id,
            is_apprentice: mem.member_role.is_apprentice(),
        }
    }
}
