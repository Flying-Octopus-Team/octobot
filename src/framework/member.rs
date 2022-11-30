use std::cmp::Ordering;
use std::fmt::Display;
use std::fmt::Formatter;

use anyhow::Ok;
use anyhow::Result;
use diesel::pg::Pg;
use diesel::QueryDsl;
use serenity::http::CacheHttp;
use serenity::model::prelude::interaction::application_command::CommandDataOption;
use serenity::model::prelude::RoleId;
use serenity::model::prelude::UserId;
use serenity::model::user::User;
use tracing::error;
use tracing::info;
use uuid::Uuid;

use crate::database::models::member::Member as DbMember;
use crate::database::schema::member::BoxedQuery;
use crate::database::PG_POOL;
use crate::diesel::ExpressionMethods;
use crate::discord::find_option_as_string;
use crate::discord::find_option_value;
use crate::SETTINGS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberRole {
    Normal,
    Apprentice,
}

impl MemberRole {
    pub fn discord_role(&self) -> RoleId {
        match self {
            MemberRole::Normal => SETTINGS.member_role_id,
            MemberRole::Apprentice => SETTINGS.apprentice_role_id,
        }
    }

    pub(crate) async fn remove_role(
        &self,
        member: &mut Member,
        cache_http: &impl CacheHttp,
    ) -> Result<()> {
        let role = self.discord_role();
        let guild = cache_http
            .cache()
            .expect("Failed to get cache")
            .guild(SETTINGS.server_id)
            .unwrap();
        let mut member = guild
            .member(cache_http.http(), member.discord_user.as_ref().unwrap().id)
            .await
            .unwrap();
        member.remove_role(cache_http.http(), role).await?;
        Ok(())
    }

    pub(crate) async fn add_role(
        &self,
        member: &mut Member,
        cache_http: &impl CacheHttp,
    ) -> Result<()> {
        let role = self.discord_role();
        let guild = cache_http
            .cache()
            .expect("Failed to get cache")
            .guild(SETTINGS.server_id)
            .unwrap();
        let mut member = guild
            .member(cache_http.http(), member.discord_user.as_ref().unwrap().id)
            .await
            .unwrap();
        member.add_role(cache_http.http(), role).await?;
        Ok(())
    }

    pub(crate) async fn swap_roles(
        &self,
        member: &mut Member,
        cache_http: &impl CacheHttp,
    ) -> Result<()> {
        let old_role = member.member_role;
        old_role.remove_role(member, cache_http).await?;
        self.add_role(member, cache_http).await?;
        Ok(())
    }

    pub(crate) fn is_apprentice(&self) -> bool {
        match self {
            MemberRole::Normal => false,
            MemberRole::Apprentice => true,
        }
    }
}

#[derive(Debug, Clone, Eq)]
pub struct Member {
    pub id: Uuid,
    pub display_name: String,
    pub discord_user: Option<User>,
    pub trello_id: Option<String>,
    pub trello_report_card_id: Option<String>,
    pub member_role: MemberRole,
}

impl Member {
    // Adds member to the services if they don't exist.
    pub fn add_member(
        display_name: String,
        discord_user: Option<User>,
        trello_id: Option<String>,
        trello_report_card_id: Option<String>,
        is_apprentice: bool,
    ) -> Result<Member> {
        let member = Self {
            id: Uuid::new_v4(),
            display_name,
            discord_user,
            trello_id,
            trello_report_card_id,
            member_role: if is_apprentice {
                MemberRole::Apprentice
            } else {
                MemberRole::Normal
            },
        };
        member.insert()?;
        Ok(member)
    }

    pub(crate) fn insert(&self) -> Result<()> {
        let db_member = DbMember::from(self.clone());
        db_member.insert()?;
        Ok(())
    }

    // Updates the member's information in the services
    pub fn update(self) -> Result<Self> {
        let member = DbMember::from(self.clone());

        if let Err(why) = member.update() {
            error!("Failed to update member: {}", why);
            Err(why)
        } else {
            Ok(self)
        }
    }

    pub async fn delete(&mut self, cache_http: &impl CacheHttp) -> Result<()> {
        let member = DbMember::from(self.clone());

        if let Err(why) = member.delete() {
            error!("Failed to delete member: {}", why);
            return Err(why);
        } else {
            info!("Deleted member: {}", self.display_name)
        }

        let member_role = self.member_role;
        member_role.remove_role(self, cache_http).await?;

        Ok(())
    }

    /// Edits member's information. Does not update the database nor any of the services.
    /// In order to update the database, call `update()`.
    pub async fn edit(
        &mut self,
        builder: MemberBuilder,
        cache_http: &impl CacheHttp,
    ) -> Result<()> {
        if let Some(display_name) = builder.display_name {
            self.display_name = display_name;
        }
        if let Some(discord_id) = builder.discord_id {
            self.discord_user = Some(
                cache_http
                    .cache()
                    .expect("Failed to get cache")
                    .user(discord_id.parse::<u64>().unwrap())
                    .unwrap(),
            );
        }
        if let Some(trello_id) = builder.trello_id {
            self.trello_id = Some(trello_id);
        }
        if let Some(trello_report_card_id) = builder.trello_report_card_id {
            self.trello_report_card_id = Some(trello_report_card_id);
        }
        if let Some(member_role) = builder.member_role {
            self.swap_roles(member_role, cache_http).await.unwrap();
            self.member_role = member_role;
        }

        Ok(())
    }

    // Lists all members from the database, applying the given filters
    pub async fn list(
        filter: MemberBuilder,
        cache_http: impl CacheHttp,
        page: i64,
        per_page: Option<i64>,
    ) -> Result<(Vec<Self>, i64)> {
        let query = filter.apply_filter(DbMember::all().into_boxed());
        let query = DbMember::paginate(query, page, per_page);
        let (db_members, total) =
            query.load_and_count_pages::<DbMember>(&mut PG_POOL.get().unwrap())?;
        let mut members = Vec::new();
        for db_member in db_members.into_iter() {
            let member = Self::from_db_member(&cache_http, db_member).await?;
            members.push(member);
        }
        Ok((members, total))
    }

    pub async fn from_db_member(cache_http: impl CacheHttp, db_member: DbMember) -> Result<Self> {
        let discord_user = match db_member.discord_id {
            Some(ref discord_id) => Some(
                UserId::from(discord_id.parse::<u64>().unwrap())
                    .to_user(cache_http)
                    .await?,
            ),
            None => None,
        };

        Ok(Self {
            id: db_member.id(),
            display_name: db_member.display_name,
            discord_user,
            trello_id: db_member.trello_id,
            trello_report_card_id: db_member.trello_report_card_id,
            member_role: if db_member.is_apprentice {
                MemberRole::Apprentice
            } else {
                MemberRole::Normal
            },
        })
    }

    pub(crate) async fn get(id: Uuid, cache_http: &impl CacheHttp) -> Result<Self> {
        let db_member = DbMember::find_by_id(id)?;
        Ok(Self {
            id: db_member.id(),
            display_name: db_member.display_name,
            discord_user: match db_member.discord_id {
                Some(ref discord_id) => Some(
                    UserId::from(discord_id.parse::<u64>().unwrap())
                        .to_user(cache_http)
                        .await?,
                ),
                None => None,
            },
            trello_id: db_member.trello_id,
            trello_report_card_id: db_member.trello_report_card_id,
            member_role: if db_member.is_apprentice {
                MemberRole::Apprentice
            } else {
                MemberRole::Normal
            },
        })
    }

    // Get member from the database by their discord id
    pub(crate) async fn get_by_discord_id(
        discord_id: u64,
        cache_http: &impl CacheHttp,
    ) -> Result<Option<Self>> {
        let db_member = DbMember::find_by_discord_id(format!("{}", discord_id))?;

        match db_member {
            Some(db_member) => {
                let member = Self::from_db_member(cache_http, db_member).await?;
                Ok(Some(member))
            }
            None => Ok(None),
        }
    }

    async fn get_by_trello_id(trello_id: &str, cache_http: impl CacheHttp) -> Result<Option<Self>> {
        let db_member = DbMember::find_by_trello_id(trello_id)?;

        match db_member {
            Some(db_member) => {
                let member = Self::from_db_member(cache_http, db_member).await?;
                Ok(Some(member))
            }
            None => Ok(None),
        }
    }

    async fn swap_roles(&mut self, role: MemberRole, cache_http: &impl CacheHttp) -> Result<()> {
        role.swap_roles(self, cache_http).await
    }

    /// Set users Discord roles to match their member role
    /// This is should be called when a member is created
    pub(crate) async fn setup(&mut self, cache_http: &impl CacheHttp) -> Result<()> {
        let role = self.member_role;
        role.add_role(self, cache_http).await
    }

    pub(crate) fn name(&self) -> String {
        self.display_name.clone()
    }
}

impl Display for Member {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let discord_user = match &self.discord_user {
            Some(discord_user) => discord_user.tag(),
            None => String::from("None"),
        };

        let trello_id = match &self.trello_id {
            Some(trello_id) => trello_id,
            None => "None",
        };

        let trello_report_card_id = match &self.trello_report_card_id {
            Some(trello_report_card_id) => trello_report_card_id,
            None => "None",
        };

        write!(
            f,
            "Member: {} ({}) Discord User: {}, Trello ID: {}, Trello Report Card ID: {}, Member Role: {}",
            self.display_name, self.id.simple(), discord_user, trello_id, trello_report_card_id, self.member_role
        )
    }
}

impl PartialEq for Member {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialOrd for Member {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

impl Ord for Member {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

pub struct MemberBuilder {
    pub display_name: Option<String>,
    pub discord_id: Option<String>,
    pub trello_id: Option<String>,
    pub trello_report_card_id: Option<String>,
    pub member_role: Option<MemberRole>,
}

impl MemberBuilder {
    pub fn set_display_name(&mut self, display_name: Option<String>) {
        self.display_name = display_name;
    }

    pub fn set_discord_id(&mut self, discord_id: Option<String>) {
        self.discord_id = discord_id;
    }

    pub fn set_trello_id(&mut self, trello_id: Option<String>) {
        self.trello_id = trello_id;
    }

    pub fn set_trello_report_card_id(&mut self, trello_report_card_id: Option<String>) {
        self.trello_report_card_id = trello_report_card_id;
    }

    pub fn set_member_role(&mut self, member_role: Option<MemberRole>) {
        self.member_role = member_role;
    }

    pub fn new() -> Self {
        Self {
            display_name: None,
            discord_id: None,
            trello_id: None,
            trello_report_card_id: None,
            member_role: None,
        }
    }

    pub(crate) async fn build(self, cache_http: &impl CacheHttp) -> Member {
        let discord_user = match &self.discord_id {
            Some(discord_id) => Some(
                UserId::from(discord_id.parse::<u64>().unwrap())
                    .to_user(cache_http)
                    .await
                    .unwrap(),
            ),
            None => None,
        };

        let display_name = match self.display_name {
            Some(display_name) => display_name,
            None => match &discord_user {
                Some(discord_user) => discord_user
                    .nick_in(cache_http, SETTINGS.server_id)
                    .await
                    .unwrap_or_else(|| discord_user.name.clone()),
                None => String::from("None"),
            },
        };

        Member {
            id: Uuid::new_v4(),
            display_name,
            discord_user,
            trello_id: self.trello_id,
            trello_report_card_id: self.trello_report_card_id,
            member_role: match self.member_role {
                Some(member_role) => member_role,
                None => MemberRole::Normal,
            },
        }
    }

    pub(crate) async fn check_for_duplicates(&self, cache_http: &impl CacheHttp) -> Result<bool> {
        let mut duplicate = false;
        let mut duplicate_message = String::from("Duplicate members found: ");

        if let Some(discord_id) = &self.discord_id {
            match Member::get_by_discord_id(discord_id.parse::<u64>().unwrap(), cache_http).await {
                Ok(member) => {
                    if let Some(member) = member {
                        duplicate = true;
                        duplicate_message.push_str(&format!("{} ", member));
                    }
                }
                Err(err) => return Err(err),
            }
        }

        if let Some(trello_id) = &self.trello_id {
            match Member::get_by_trello_id(trello_id, cache_http).await {
                Ok(member) => {
                    if let Some(member) = member {
                        duplicate = true;
                        duplicate_message.push_str(&format!("{} ", member));
                    }
                }
                Err(err) => return Err(err),
            }
        }

        if duplicate {
            Err(duplicate_message.into())
        } else {
            Ok(())
        }
    }

    pub fn apply_filter<'a>(&'a self, mut query: BoxedQuery<'a, Pg>) -> BoxedQuery<'a, Pg> {
        use crate::database::schema::member::dsl;

        if let Some(ref display_name) = self.display_name {
            query = query.filter(dsl::display_name.eq(display_name));
        }

        if let Some(ref discord_id) = self.discord_id {
            query = query.filter(dsl::discord_id.eq(discord_id));
        }

        if let Some(ref trello_id) = self.trello_id {
            query = query.filter(dsl::trello_id.eq(trello_id));
        }

        if let Some(ref trello_report_card_id) = self.trello_report_card_id {
            query = query.filter(dsl::trello_report_card_id.eq(trello_report_card_id));
        }

        if let Some(ref member_role) = self.member_role {
            query = query.filter(dsl::is_apprentice.eq(member_role.is_apprentice()));
        }

        query
    }
}

impl Default for MemberBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl From<&CommandDataOption> for MemberBuilder {
    fn from(option: &CommandDataOption) -> Self {
        let options = &option.options[..];
        let mut builder = Self::new();

        if let Some(ref display_name) = find_option_as_string(options, "display-name") {
            builder.set_display_name(Some(display_name.clone()));
        }

        if let Some(ref discord_id) = find_option_as_string(options, "discord-id") {
            builder.set_discord_id(Some(discord_id.clone()));
        }

        if let Some(ref trello_id) = find_option_as_string(options, "trello-id") {
            builder.set_trello_id(Some(trello_id.clone()));
        }

        if let Some(ref trello_report_card_id) =
            find_option_as_string(options, "trello-report-card-id")
        {
            builder.set_trello_report_card_id(Some(trello_report_card_id.clone()));
        }

        if let Some(is_apprentice) = find_option_value(options, "is-apprentice") {
            if is_apprentice.as_bool().unwrap() {
                builder.set_member_role(Some(MemberRole::Apprentice));
            } else {
                builder.set_member_role(Some(MemberRole::Normal));
            }
        }

        builder
    }
}

impl Display for MemberRole {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MemberRole::Normal => write!(f, "Normal"),
            MemberRole::Apprentice => write!(f, "Apprentice"),
        }
    }
}
