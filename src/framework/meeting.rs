use std::fmt::Display;
use std::fmt::Formatter;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

use chrono::NaiveDateTime;
use cron::Schedule;
use crony::Job;
use serenity::http::CacheHttp;
use serenity::model::prelude::GuildChannel;
use serenity::prelude::Context;
use serenity::prelude::TypeMap;
use serenity::prelude::TypeMapKey;
use tokio::sync::RwLock;
use tracing::log::error;
use tracing::log::info;
use uuid::Uuid;

use super::member::Member;
use super::summary::Summary;
use crate::database::models::meeting::Meeting as DbMeeting;
use crate::database::models::meeting::MeetingFilter;
use crate::database::models::meeting::MeetingMembers;
use crate::SETTINGS;

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
        let start_date = schedule
            .upcoming(chrono::Local)
            .next()
            .unwrap()
            .naive_local();

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

    pub fn insert(&self) -> Result<(), Box<dyn std::error::Error>> {
        let db_meeting = DbMeeting::from(self.clone());
        db_meeting.insert()?;
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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

    pub async fn get(
        cache_http: &impl CacheHttp,
        id: Uuid,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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

    pub async fn from_db_meeting(
        cache_http: &impl CacheHttp,
        db_meeting: DbMeeting,
    ) -> Result<Meeting, Box<dyn std::error::Error>> {
        let summary = Summary::get(cache_http, db_meeting.summary_id()).await?;

        let channel_id = db_meeting.channel_id().parse::<u64>().unwrap();

        let channel = match cache_http.cache().and_then(|c| c.guild_channel(channel_id)) {
            Some(channel) => channel,
            None => {
                error!("Channel not found: {}", channel_id);
                return Err("Channel not found".into());
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
        filter: impl Into<MeetingFilter>,
        cache_http: &impl CacheHttp,
        page: i64,
        per_page: Option<i64>,
    ) -> Result<(Vec<Self>, i64), Box<dyn std::error::Error>> {
        let (db_meetings, total_pages) = DbMeeting::list(filter, page, per_page)?;

        let mut meetings = Vec::new();

        for db_meeting in db_meetings {
            let meeting = Self::from_db_meeting(cache_http, db_meeting).await?;
            meetings.push(meeting);
        }

        Ok((meetings, total_pages))
    }

    pub async fn await_meeting(data: Arc<RwLock<TypeMap>> ,client: impl CacheHttp + 'static) {
        let meeting = Self::next_meeting(&client).await;
        let schedule = crony::Schedule::from_str(&meeting.schedule.to_string()).unwrap();

        let job = MeetingJob {
            data: data.clone(),
            cache_and_http: Arc::new(client),
            schedule,
        };

        let runner = crony::Runner::new().add(Box::new(job)).run();

        let meeting_status = MeetingStatus {
            meeting,
            is_running: false,
            skip: false,
            runner: Mutex::new(runner),
        };

        data
            .write()
            .await
            .insert::<MeetingStatus>(Arc::new(RwLock::new(meeting_status)));
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
            Self::new(cache_http)
        } else {
            info!("Latest meeting has not ended. Await next meeting");
            Self::from_db_meeting(cache_http, latest).await.unwrap()
        }
    }

    pub async fn skip(ctx: &Context) {
        let mut data = ctx.data.write().await;
        let meeting_status = data.get_mut::<MeetingStatus>().unwrap();

        let mut meeting_status = meeting_status.write().await;
        meeting_status.skip = true;
    }

    pub async fn end(note: String, ctx: &Context) -> Result<String, Box<dyn std::error::Error>> {
        let mut data = ctx.data.write().await;
        let meeting_status = data.get_mut::<MeetingStatus>().unwrap();

        let mut meeting_status = meeting_status.write().await;

        if meeting_status.is_running {
            meeting_status.meeting._end()?;
            meeting_status.is_running = false;
            meeting_status.meeting.summary.note = note;
            meeting_status.meeting.summary.update()?;

            let mut meeting = Self::new(ctx);
            meeting.schedule = meeting_status.meeting.schedule.clone();

            meeting.insert()?;
            meeting.summary.insert()?;
            meeting_status.meeting = meeting;

            Ok("Meeting ended successfully".to_string())
        } else {
            Err("Meeting is not running".into())
        }
    }

    fn _end(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.end_date = Some(chrono::Local::now().naive_local());
        self.update()
    }

    pub async fn get_current_meeting(ctx: &Context) -> Self {
        let data = ctx.data.read().await;
        let meeting_status = data.get::<MeetingStatus>().unwrap();
        let meeting_status = meeting_status.read().await;

        meeting_status.meeting.clone()
    }

    pub async fn add_member(
        &mut self,
        member: Member,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let meeting_member = MeetingMembers::new(self.id, member.id);
        meeting_member.insert()?;

        let output = format!("Member {} added to meeting {}", member.name(), self.id);
        self.members.push((meeting_member.id(), member));
        Ok(output)
    }

    pub async fn remove_member(
        &mut self,
        member: Member,
    ) -> Result<String, Box<dyn std::error::Error>> {
        self.members.retain(|(_, m)| m.id != member.id);
        MeetingMembers::delete_by_meeting_and_member(self.id, member.id)?;
        let output = format!("Member {} removed", member.name());
        Ok(output)
    }

    pub async fn change_future_schedule(ctx: Context, schedule: String) -> Result<String, Box<dyn std::error::Error>> {
        let mut data = ctx.data.write().await;
        let meeting_status = data.remove::<MeetingStatus>().unwrap();

        let mut meeting_status = Arc::try_unwrap(meeting_status).unwrap().into_inner();

        let schedule = cron::Schedule::from_str(&schedule)?;
        meeting_status.meeting.schedule = schedule;
        meeting_status.meeting.update()?;

        meeting_status.abort();
        Self::await_meeting(ctx.data.clone(), ctx.clone()).await;

        Ok("Schedule changed successfully".to_string())
    }

    pub(crate) async fn resend_summary(
        &self,
        cache_http: &impl CacheHttp,
    ) -> Result<String, Box<dyn std::error::Error>> {
        self.summary.resend_summary(cache_http).await
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

pub(self) struct MeetingStatus {
    skip: bool,
    is_running: bool,
    meeting: Meeting,
    runner: Mutex<crony::Runner>,
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
    pub(self) fn meeting(&self) -> &Meeting {
        &self.meeting
    }

    pub(self) fn schedule(&self) -> &Schedule {
        &self.meeting.schedule
    }

    pub(self) fn is_running(&self) -> bool {
        self.is_running
    }

    pub(self) fn members(&self) -> &Vec<(Uuid, Member)> {
        &self.meeting.members
    }

    pub(self) fn channel(&self) -> &GuildChannel {
        &self.meeting.channel
    }

    // stop the runner
    pub(self) fn abort(self) {
        let runner = self.runner.into_inner().unwrap();
        runner.stop();
    }
}

impl TypeMapKey for MeetingStatus {
    type Value = Arc<RwLock<Self>>;
}

pub(self) struct MeetingJob<C: CacheHttp> {
    data: Arc<RwLock<TypeMap>>,
    cache_and_http: Arc<C>,
    schedule: crony::Schedule,
}

impl<C: CacheHttp + 'static> Job for MeetingJob<C> {
    fn schedule(&self) -> crony::Schedule {
        self.schedule.clone()
    }

    fn handle(&self) {
        let data = self.data.clone();
        let cache_and_http = self.cache_and_http.clone();
        tokio::spawn(async move {
            let data = data.read().await;
            let meeting_status = data.get::<MeetingStatus>().unwrap().clone();
            let mut meeting_status = meeting_status.write().await;

            if meeting_status.skip {
                meeting_status.skip = false;
                return;
            }

            if meeting_status.is_running {
                return;
            }

            // load members from the channel
            let cache = cache_and_http.cache().unwrap();
            let members = match meeting_status.meeting.channel.members(cache).await {
                Ok(members) => members,
                Err(e) => {
                    error!("Error while getting members from channel: {}", e);
                    return;
                }
            };

            for dc_member in members {
                let member =
                    match Member::get_by_discord_id(dc_member.user.id.0, &cache_and_http).await {
                        Ok(member) => match member {
                            Some(member) => member,
                            None => {
                                error!("Member not found: {}", dc_member.user.id.0);
                                continue;
                            }
                        },
                        Err(e) => {
                            error!("Error while getting member: {}", e);
                            return;
                        }
                    };
                let info = meeting_status.meeting.add_member(member).await.unwrap();
                info!("{}", info);
            }

            meeting_status.is_running = true;
        });
    }
}

pub async fn status(ctx: &Context) -> String {
    let data = ctx.data.read().await;
    let meeting_status = data.get::<MeetingStatus>().unwrap();
    let meeting_status = meeting_status.read().await;

    let mut output = String::new();

    if meeting_status.is_running() {
        output.push_str("Meeting is ongoing. ");
        output.push_str(&meeting_status.meeting().to_string());
    } else {
        output.push_str("Planned meeting on ");
        output.push_str(
            &meeting_status
                .schedule()
                .upcoming(chrono::Local)
                .next()
                .unwrap()
                .to_string(),
        );
        output.push_str(" with id ");
        output.push_str(&meeting_status.meeting().to_string());
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
