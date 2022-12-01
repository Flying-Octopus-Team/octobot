use std::fmt::Display;
use std::fmt::Formatter;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::Local;
use chrono::NaiveDateTime;
use cron::Schedule;
use diesel::pg::Pg;
use diesel::ExpressionMethods;
use diesel::QueryDsl;
use serenity::client::Cache;
use serenity::http::CacheHttp;
use serenity::http::Http;
use serenity::model::channel::ChannelType;
use serenity::model::prelude::GuildChannel;
use serenity::prelude::Context;
use serenity::prelude::Mentionable;
use serenity::prelude::TypeMapKey;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::log::error;
use tracing::log::info;
use uuid::Uuid;

use self::db_meeting::Meeting as DbMeeting;
use self::db_meeting::MeetingMembers;
use super::member::Member;
use super::summary::Summary;
use crate::database::schema::meeting::BoxedQuery;
use crate::SETTINGS;

mod db_meeting;

#[derive(Debug, Clone)]
pub struct Meeting {
    pub id: Uuid,
    pub start_date: NaiveDateTime,
    pub end_date: Option<NaiveDateTime>,
    pub summary: Summary,
    pub channel: GuildChannel,
    pub schedule: Schedule,
    pub members: Vec<(Uuid, Member)>,
}

impl Meeting {
    fn new(cache_http: &impl CacheHttp) -> Self {
        let channel = cache_http
            .cache()
            .unwrap()
            .guild_channel(SETTINGS.meeting.channel_id)
            .unwrap();
        let schedule = Schedule::from_str(&SETTINGS.meeting.cron).unwrap();
        let start_date = schedule.upcoming(Local).next().unwrap().naive_local();

        Self {
            id: Uuid::new_v4(),
            start_date,
            end_date: None,
            summary: Summary::new(start_date.date()),
            channel,
            schedule,
            members: Vec::new(),
        }
    }

    fn new_from_previous(previous: Self, cache_http: &impl CacheHttp) -> Self {
        let new_meeting = Self::new(cache_http);

        Self {
            id: new_meeting.id,
            start_date: new_meeting.start_date,
            end_date: new_meeting.end_date,
            summary: new_meeting.summary,
            channel: previous.channel,
            schedule: previous.schedule,
            members: new_meeting.members,
        }
    }

    fn insert(&self) -> Result<()> {
        let db_meeting = DbMeeting::from(self.clone());
        db_meeting.insert()?;
        Ok(())
    }

    fn update(&mut self) -> Result<()> {
        let db_meeting = DbMeeting::from(self.clone());

        match db_meeting.update() {
            Ok(_) => {
                info!("Meeting updated successfully");
                Ok(())
            }
            Err(e) => {
                error!("Error while updating meeting: {}", e);
                Err(e)
            }
        }
    }

    pub async fn get(cache_http: &impl CacheHttp, id: Uuid) -> Result<Self> {
        let db_meeting = match DbMeeting::find_by_id(id) {
            Ok(meeting) => meeting,
            Err(e) => {
                error!("Error while getting meeting from database: {}", e);
                return Err(e);
            }
        };

        let meeting = Self::from_db_meeting(cache_http, db_meeting).await?;

        Ok(meeting)
    }

    pub(super) async fn get_by_summary_id(
        cache_http: &impl CacheHttp,
        summary_id: Uuid,
    ) -> Result<Self> {
        let db_meeting = match DbMeeting::find_by_summary_id(summary_id) {
            Ok(meeting) => meeting,
            Err(e) => {
                error!("Error while getting meeting from database: {}", e);
                return Err(e);
            }
        };

        let meeting = Self::from_db_meeting(cache_http, db_meeting).await?;

        Ok(meeting)
    }

    async fn from_db_meeting(
        cache_http: &impl CacheHttp,
        db_meeting: DbMeeting,
    ) -> Result<Meeting> {
        let summary = Summary::get(cache_http, db_meeting.summary_id()).await?;

        let channel_id = db_meeting.channel_id().parse::<u64>().unwrap();

        let channel = match cache_http.cache().and_then(|c| c.guild_channel(channel_id)) {
            Some(channel) => channel,
            None => {
                error!("Channel not found: {}", channel_id);
                return Err(anyhow::anyhow!("Channel not found"));
            }
        };

        let schedule = match db_meeting.schedule() {
            Ok(schedule) => schedule,
            Err(e) => {
                error!("Error while parsing schedule: {}", e);
                return Err(e);
            }
        };

        let mut members = Vec::new();
        let load_members = MeetingMembers::load_members(db_meeting.id())?;

        for m_member in load_members {
            let member = Member::get(m_member.member_id(), cache_http).await?;
            members.push((m_member.id(), member));
        }

        let meeting = Meeting {
            id: db_meeting.id(),
            start_date: db_meeting.start_date(),
            end_date: db_meeting.end_date(),
            summary,
            channel,
            schedule,
            members,
        };

        Ok(meeting)
    }

    pub async fn list(
        filter: impl Into<Filter>,
        cache_http: &impl CacheHttp,
        page: i64,
        page_size: Option<i64>,
    ) -> Result<(Vec<Self>, i64)> {
        let (db_meetings, total_pages) = DbMeeting::list(filter, page, page_size)?;

        let mut meetings = Vec::new();

        for db_meeting in db_meetings {
            let meeting = Self::from_db_meeting(cache_http, db_meeting).await?;
            meetings.push(meeting);
        }

        Ok((meetings, total_pages))
    }

    pub async fn await_meeting(ctx: &Context) {
        if let Some(meeting_status) = ctx.data.read().await.get::<MeetingStatus>() {
            let meeting_status = meeting_status.read().await;

            if meeting_status.is_running {
                return;
            }
        }

        let meeting = Self::next_meeting(&ctx).await;

        let meeting_status = MeetingStatus {
            meeting,
            is_running: false,
            skip: false,
            cache: ctx.cache.clone(),
            http: ctx.http.clone(),
            join_handle: None,
        };

        let meeting_status = Arc::new(RwLock::new(meeting_status));

        MeetingStatus::await_meeting(meeting_status.clone()).await;

        {
            let mut data = ctx.data.write().await;
            data.insert::<MeetingStatus>(meeting_status);
        }
    }

    async fn next_meeting(cache_http: &impl CacheHttp) -> Self {
        let latest = match DbMeeting::get_latest_meeting() {
            Ok(meeting) => meeting,
            Err(e) => {
                info!(
                    "Error while getting latest meeting: {}. Falling back to default",
                    e
                );
                return Self::new(cache_http);
            }
        };

        if latest.end_date().is_some() {
            info!("Latest meeting has ended. Falling back to default");
            let latest = Self::from_db_meeting(cache_http, latest).await.unwrap();
            let new_meeting = Self::new_from_previous(latest, cache_http);
            new_meeting.insert().unwrap();
            new_meeting
        } else {
            info!("Latest meeting has not ended. Await next meeting");
            Self::from_db_meeting(cache_http, latest).await.unwrap()
        }
    }

    async fn skip(ctx: &Context) {
        let mut data = ctx.data.write().await;
        let meeting_status = data.get_mut::<MeetingStatus>().unwrap();

        let mut meeting_status = meeting_status.write().await;
        meeting_status.skip = true;
    }

    pub async fn end(note: String, ctx: &Context) -> Result<String> {
        let mut data = ctx.data.write().await;
        let meeting_status = data.get_mut::<MeetingStatus>().unwrap();

        let mut meeting_status = meeting_status.write().await;

        if meeting_status.is_running {
            meeting_status.meeting._end(ctx).await?;
            meeting_status.is_running = false;
            meeting_status.meeting.summary.note = note;
            meeting_status.meeting.summary.update()?;

            let meeting = Self::new_from_previous(meeting_status.meeting.clone(), ctx);

            meeting.insert()?;
            meeting.summary.insert()?;
            meeting_status.meeting = meeting;

            Ok("Meeting ended successfully".to_string())
        } else {
            Err(anyhow::anyhow!("Meeting is not running"))
        }
    }

    async fn _end(&mut self, cache_http: &impl CacheHttp) -> Result<()> {
        self.end_date = Some(Local::now().naive_local());
        self.summary.send_summary(cache_http).await?;
        self.update()
    }

    pub async fn get_current_meeting(ctx: &Context) -> Self {
        let data = ctx.data.read().await;
        let meeting_status = data.get::<MeetingStatus>().unwrap();
        let meeting_status = meeting_status.read().await;

        meeting_status.meeting.clone()
    }

    pub async fn is_meeting_ongoing(ctx: &Context) -> bool {
        let data = ctx.data.read().await;
        let meeting_status = data.get::<MeetingStatus>().unwrap();
        let meeting_status = meeting_status.read().await;

        meeting_status.is_running
    }

    pub async fn add_member(&mut self, member: Member) -> Result<String> {
        if MeetingMembers::is_user_in_meeting(self.id, member.id)? {
            return Err(anyhow::anyhow!("User is already in meeting"));
        }

        let meeting_member = MeetingMembers::new(self.id, member.id);
        meeting_member.insert()?;

        let output = format!("Member {} added to meeting {}", member.name(), self.id);
        self.members.push((meeting_member.id(), member));
        Ok(output)
    }

    pub async fn remove_member(&mut self, member: Member) -> Result<String> {
        self.members.retain(|(_, m)| m.id != member.id);
        let edited_rows = MeetingMembers::delete_by_meeting_and_member(self.id, member.id)?;
        if edited_rows > 0 {
            Ok(format!("Member {} removed", member.name()))
        } else {
            Err(anyhow::anyhow!("Member not found"))
        }
    }

    pub async fn change_future_schedule<T: TryInto<Schedule>>(
        ctx: &Context,
        schedule: T,
    ) -> Result<String>
    where
        <T as TryInto<Schedule>>::Error: std::error::Error,
    {
        let schedule = schedule.try_into().unwrap();
        let mut data = ctx.data.write().await;
        let meeting_status = data.remove::<MeetingStatus>().unwrap();

        if meeting_status.read().await.is_running {
            return Err(anyhow::anyhow!(
                "Meeting is running. Cannot change schedule"
            ));
        }

        let mut meeting_status = Arc::try_unwrap(meeting_status).unwrap().into_inner();

        meeting_status.meeting.schedule = schedule;
        meeting_status.meeting.update()?;

        meeting_status.abort();
        Self::await_meeting(ctx).await;

        Ok("Schedule changed successfully".to_string())
    }

    pub async fn change_future_channel(ctx: &Context, channel: GuildChannel) -> Result<()> {
        let data = ctx.data.read().await;
        let meeting_status = data.get::<MeetingStatus>().unwrap();
        let mut meeting_status = meeting_status.write().await;

        if channel.kind != ChannelType::Voice {
            error!("Channel is not a voice channel: {}", channel.mention());
            return Err(anyhow::anyhow!("Channel is not a voice channel"));
        }

        meeting_status.meeting.channel = channel;
        meeting_status.meeting.update()?;

        Ok(())
    }

    pub async fn display_status(ctx: &Context) -> String {
        MeetingStatus::display_status(ctx).await
    }

    pub async fn resend_summary(&self, cache_http: &impl CacheHttp) -> Result<String> {
        self.summary.resend_summary(cache_http).await
    }

    pub fn find() -> Filter {
        Filter::default()
    }
}

impl Display for Meeting {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Meeting ID: {} Summary: {} Start Date: {} End Date: {:?} Members: {}",
            self.id,
            self.summary.id,
            self.start_date,
            self.end_date,
            self.members.len()
        )
    }
}

struct MeetingStatus {
    skip: bool,
    is_running: bool,
    meeting: Meeting,
    cache: Arc<Cache>,
    http: Arc<Http>,
    join_handle: Option<JoinHandle<()>>,
}

impl CacheHttp for MeetingStatus {
    fn cache(&self) -> Option<&Arc<Cache>> {
        Some(&self.cache)
    }

    fn http(&self) -> &Http {
        &self.http
    }
}

impl std::fmt::Debug for MeetingStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Meeting Status: {{ skip: {}, is_running: {}, meeting: {} }}",
            self.skip, self.is_running, self.meeting
        )
    }
}

impl MeetingStatus {
    fn schedule(&self) -> &Schedule {
        &self.meeting.schedule
    }

    fn members(&self) -> &Vec<(Uuid, Member)> {
        &self.meeting.members
    }

    fn channel(&self) -> &GuildChannel {
        &self.meeting.channel
    }

    // stop the runner
    fn abort(self) {
        self.join_handle.unwrap().abort();
    }

    async fn await_meeting(meeting_status: Arc<RwLock<Self>>) {
        let meeting_status_clone = meeting_status.clone();
        let handle = tokio::spawn(async move {
            loop {
                {
                    let mut meeting_status = meeting_status.write().await;
                    if meeting_status.should_start() {
                        match meeting_status.start().await {
                            Ok(_) => {}
                            Err(e) => {
                                error!("Error starting meeting: {}", e);
                            }
                        }
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        let mut meeting_status = meeting_status_clone.write().await;
        meeting_status.join_handle = Some(handle);
    }

    fn should_start(&self) -> bool {
        // run if the meeting start date is in the past
        let now = Local::now();
        if self.meeting.start_date < now.naive_local() {
            return true;
        }

        false
    }

    async fn start(&mut self) -> Result<()> {
        if self.skip {
            self.skip = false;
            return Ok(());
        }

        if self.is_running {
            return Ok(());
        }

        self.is_running = true;

        // load members from the channel
        let members = self.meeting.channel.members(self.cache().unwrap()).await?;

        for dc_member in members {
            let member = Member::get_by_discord_id(dc_member.user.id.0, self).await?;

            if let Some(member) = member {
                let info = self.meeting.add_member(member).await;
                if let Err(e) = info {
                    error!("Error adding member: {}", e);
                } else {
                    info!("Member added: {}", info.unwrap());
                }
            }
        }

        Ok(())
    }

    async fn display_status(ctx: &Context) -> String {
        let data = ctx.data.read().await;
        let meeting_status = data.get::<MeetingStatus>().unwrap();
        let meeting_status = meeting_status.read().await;

        let mut output = String::new();

        if meeting_status.is_running {
            output.push_str("Meeting is ongoing. ");
            output.push_str(&meeting_status.meeting.to_string());
        } else {
            output.push_str("Planned meeting on ");
            output.push_str(
                &meeting_status
                    .schedule()
                    .upcoming(Local)
                    .next()
                    .unwrap()
                    .to_string(),
            );
            output.push_str(" with id ");
            output.push_str(&meeting_status.meeting.to_string());
        }

        output.push_str("\nMembers:");
        for (_, member) in meeting_status.members() {
            output.push_str(&member.name());
        }

        output.push_str("\nMonitoring channel: <#");
        output.push_str(&meeting_status.channel().id.0.to_string());
        output.push('>');

        output
    }
}

impl TypeMapKey for MeetingStatus {
    type Value = Arc<RwLock<Self>>;
}

pub struct Filter {
    start_date: Option<chrono::NaiveDateTime>,
    end_date: Option<chrono::NaiveDateTime>,
    summary_id: Option<Uuid>,
    channel_id: Option<String>,
}

impl Filter {
    pub fn new() -> Self {
        Self {
            start_date: None,
            end_date: None,
            summary_id: None,
            channel_id: None,
        }
    }

    pub fn apply(self, query: BoxedQuery<'_, Pg>) -> BoxedQuery<'_, Pg> {
        use crate::database::schema::meeting;

        let mut query = query;

        if let Some(start_date) = self.start_date {
            query = query.filter(meeting::start_date.gt(start_date));
        }

        if let Some(end_date) = self.end_date {
            query = query.filter(meeting::end_date.lt(end_date));
        }

        if let Some(summary_id) = self.summary_id {
            query = query.filter(meeting::summary_id.eq(summary_id));
        }

        if let Some(channel_id) = self.channel_id {
            query = query.filter(meeting::channel_id.eq(channel_id));
        }

        query
    }

    pub async fn list(
        self,
        cache_http: &impl CacheHttp,
        page: i64,
        page_size: Option<i64>,
    ) -> Result<(Vec<Meeting>, i64)> {
        Meeting::list(self, cache_http, page, page_size).await
    }

    pub fn start_date(mut self, start_date: Option<chrono::NaiveDateTime>) -> Self {
        self.start_date = start_date;
        self
    }

    pub fn end_date(mut self, end_date: Option<chrono::NaiveDateTime>) -> Self {
        self.end_date = end_date;
        self
    }

    pub fn summary_id(mut self, summary_id: Option<Uuid>) -> Self {
        self.summary_id = summary_id;
        self
    }

    pub fn channel_id(mut self, channel_id: Option<String>) -> Self {
        self.channel_id = channel_id;
        self
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self::new()
    }
}
