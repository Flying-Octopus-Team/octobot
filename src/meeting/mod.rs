use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::{str::FromStr, sync::Arc, time::Duration};

use chrono::Local;
use cron::Schedule;
use serenity::client::Cache;
use serenity::prelude::{Context, TypeMapKey};
use tokio::{sync::RwLock, task::JoinHandle};
use tracing::{error, info};
use uuid::Uuid;

use crate::database::models::summary::Summary;
use crate::{
    database::models::{
        meeting::{Meeting, MeetingMembers},
        member::Member,
    },
    SETTINGS,
};

/// Struct that holds the current meeting status.
/// It is used to keep track of the meeting's members and to check if the meeting is ongoing.
/// Creates and manages task that is responsible for starting the meeting.
///
/// Every call editing schedule will cancel the task and create a new one.
#[derive(Debug)]
pub struct MeetingStatus {
    is_ongoing: AtomicBool,
    meeting_data: Meeting,
    members: Vec<MeetingMembers>,
    handle: Option<JoinHandle<()>>,
    schedule: Schedule,
}

impl TypeMapKey for MeetingStatus {
    type Value = Arc<RwLock<MeetingStatus>>;
}

pub async fn create_meeting_job(
    ctx: &Context,
) -> Result<Arc<RwLock<MeetingStatus>>, Box<dyn std::error::Error>> {
    let meeting_status = MeetingStatus::load_next_meeting()?;

    let meeting_status = Arc::new(RwLock::new(meeting_status));

    MeetingStatus::await_meeting(Arc::clone(&meeting_status), ctx).await;

    Ok(meeting_status)
}

impl MeetingStatus {
    pub fn new(
        scheduled_cron: &str,
        channel_id: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let meeting_status = Self {
            is_ongoing: AtomicBool::new(false),
            meeting_data: Meeting::try_from_cron(scheduled_cron, channel_id)?,
            members: vec![],
            handle: None,
            schedule: Schedule::from_str(scheduled_cron)?,
        };
        Ok(meeting_status)
    }

    /// Change the meeting's schedule.
    ///
    /// This will cancel the current task and create a new one.
    pub async fn change_schedule(
        meeting_status: Arc<RwLock<Self>>,
        scheduled_cron: &str,
        ctx: &Context,
    ) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut meeting_status = meeting_status.write().await;

            meeting_status.abort_meeting();

            meeting_status.schedule = Schedule::from_str(scheduled_cron)?;
            meeting_status.meeting_data =
                Meeting::try_from_cron(scheduled_cron, meeting_status.channel().to_string())?;
        }

        MeetingStatus::await_meeting(meeting_status, ctx).await;

        Ok(())
    }

    pub fn change_channel(&mut self, channel_id: String) -> Result<(), Box<dyn std::error::Error>> {
        match self.meeting_data.set_channel_id(channel_id) {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Error while changing channel: {}", e);
                Err(e)
            }
        }
    }

    pub fn change_summary_note(
        &mut self,
        summary: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self.meeting_data.set_summary_note(summary) {
            Ok(_) => Ok(()),
            Err(e) => {
                let error = format!("Error while changing summary: {}", e);
                error!("{}", error);
                Err(error.into())
            }
        }
    }

    fn abort_meeting(&self) {
        info!("Removing meeting from the database {:?}", self.meeting_data);
        self.meeting_data.delete().unwrap();
        self.handle.as_ref().unwrap().abort();
    }

    pub fn is_meeting_ongoing(&self) -> bool {
        self.is_ongoing.load(SeqCst)
    }

    pub fn meeting_id(&self) -> Uuid {
        self.meeting_data.id()
    }

    pub fn schedule(&self) -> Result<Schedule, Box<dyn std::error::Error>> {
        self.meeting_data.schedule()
    }

    pub fn members(&self) -> Vec<MeetingMembers> {
        self.members.clone()
    }

    pub fn channel(&self) -> &str {
        self.meeting_data.channel_id()
    }

    pub fn start_date(&self) -> chrono::NaiveDateTime {
        self.meeting_data.start_date()
    }

    /// Ends the meeting and inserts data to the database. Updates given meeting status.
    /// Clears the meeting data and members.
    pub async fn end_meeting(
        ctx: &Context,
        meeting_status: Arc<RwLock<MeetingStatus>>,
        summary_note: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut meeting = meeting_status.write().await;

        meeting.set_is_ongoing(false);

        match meeting.change_summary_note(summary_note) {
            Ok(_) => {}
            Err(e) => {
                let error = format!("Error inserting summary: {}", e);
                error!("{}", error);
                return Err(error.into());
            }
        }

        match meeting.meeting_data.end_meeting(Local::now()) {
            Ok(_) => {}
            Err(e) => {
                let error = format!("Error inserting meeting: {}", e);
                error!("{}", error);
                return Err(error.into());
            }
        };

        let scheduled_cron = String::from(meeting.meeting_data.scheduled_cron());

        let channel_id = meeting.channel().to_string();

        *meeting = MeetingStatus::new(&scheduled_cron, channel_id)?;

        drop(meeting);

        MeetingStatus::await_meeting(meeting_status.clone(), ctx).await;

        Ok(())
    }

    pub fn add_member(&mut self, member: &Member) -> Result<String, Box<dyn std::error::Error>> {
        let meeting = self.meeting();
        match meeting.add_member(member) {
            Ok(msg) => {
                info!("{} joined", member.name());
                self.members
                    .push(MeetingMembers::new(member.id(), self.meeting_id()));
                Ok(msg)
            }
            Err(e) => {
                error!("Error adding member to meeting: {}", e);
                Err(e)
            }
        }
    }

    pub fn remove_member(&mut self, member: &Member) -> Result<String, Box<dyn std::error::Error>> {
        self.members.retain(|m| m.member_id() != member.id());

        let meeting = self.meeting();

        meeting.remove_member(member)
    }

    /// Generate summary for the given meeting
    pub async fn generate_summary(
        &self,
        note: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let summary = Summary::find_by_id(self.summary_id()).unwrap();

        summary.generate_summary(self, note).await
    }

    /// Loads the next meeting from the database, or defaults to a new meeting.
    fn load_next_meeting() -> Result<Self, Box<dyn std::error::Error>> {
        let meeting_data = Meeting::load_next_meeting()?;
        let s = String::from(meeting_data.scheduled_cron());

        let members = MeetingMembers::load_members(meeting_data.id())?;

        let meeting_status = Self {
            is_ongoing: AtomicBool::new(false),
            meeting_data,
            members,
            handle: None,
            schedule: Schedule::from_str(&s)?,
        };
        Ok(meeting_status)
    }

    /// Saves the meeting to the database and creates a task that will start the meeting.
    ///
    /// The task will be cancelled if the schedule is changed.
    async fn await_meeting(meeting_status: Arc<RwLock<Self>>, ctx: &Context) {
        let meeting_status_clone = Arc::clone(&meeting_status);
        let cache = ctx.cache.clone();
        let join_handle = tokio::spawn(async move {
            let meeting_status = meeting_status_clone;
            info!(
                "Waiting for the meeting {}",
                meeting_status.read().await.meeting_id()
            );

            let duration = meeting_status.read().await.load_duration().unwrap();

            if duration.as_secs() > 0 {
                info!("Sleeping for {:?}", duration);
                tokio::time::sleep(duration).await;
            }

            let mut meeting_status = meeting_status.write().await;

            meeting_status.set_is_ongoing(true);

            match meeting_status.start_meeting(&cache).await {
                Ok(_) => {
                    info!("Meeting started");
                }
                Err(e) => error!("Error creating meeting job: {:?}", e),
            }
        });
        meeting_status.write().await.handle = Some(join_handle);
    }

    fn load_duration(&self) -> Result<Duration, Box<dyn std::error::Error>> {
        let schedule = match self.schedule() {
            Ok(s) => s,
            Err(e) => {
                let error = format!("Error while getting schedule: {}", e);
                error!("{}", error);
                return Err(error.into());
            }
        };

        if let Some(datetime) = schedule.upcoming(Local).next() {
            let mut duration = datetime
                .signed_duration_since(Local::now())
                .to_std()
                .unwrap();

            // check if the given meeting data already exists in the database
            if self.meeting_data.exists().unwrap() {
                // if it does, update the meeting data
                duration = if self.meeting_data.start_date() > Local::now().naive_local()
                    && !self.is_meeting_ongoing()
                {
                    self.meeting_data
                        .start_date()
                        .signed_duration_since(Local::now().naive_local())
                        .to_std()
                        .unwrap()
                } else {
                    Duration::from_secs(0)
                };
                self.meeting_data.update().unwrap();
            } else {
                // if it doesn't, insert the meeting data
                self.meeting_data.insert().unwrap();
            }

            Ok(duration)
        } else {
            Err("No upcoming meeting".into())
        }
    }

    /// Starts the meeting and saves current users in the meeting channel
    async fn start_meeting(
        &mut self,
        cache: &Arc<Cache>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let channel = match cache.guild_channel(SETTINGS.meeting.channel_id) {
            Some(c) => c,
            None => {
                error!("Error getting channel");
                return Err("Error getting channel".into());
            }
        };

        for member in channel.members(&cache).await? {
            let member = {
                let member_result = Member::find_by_discord_id(member.user.id.0.to_string());
                match member_result {
                    Ok(m) => m,
                    Err(e) => {
                        error!("Error getting member: {}", e);
                        continue;
                    }
                }
            };
            match self.add_member(&member) {
                Ok(_) => {}
                Err(e) => error!("Error adding member to meeting: {}", e),
            }
        }

        Ok(())
    }

    fn set_is_ongoing(&mut self, new_value: bool) {
        self.is_ongoing.store(new_value, SeqCst);
    }

    pub fn meeting(&self) -> &Meeting {
        &self.meeting_data
    }

    pub(crate) fn summary_id(&self) -> Uuid {
        self.meeting_data.summary_id()
    }

    pub(crate) fn summary_messages_id(&self) -> Option<Vec<String>> {
        let summary = Summary::find_by_id(self.summary_id()).unwrap();
        summary.messages_id()
    }

    pub(crate) fn set_summary_messages_id(
        &self,
        messages_id: Vec<String>,
    ) -> Result<Summary, Box<dyn std::error::Error>> {
        let summary = Summary::find_by_id(self.summary_id()).unwrap();
        summary.set_messages_id(messages_id)
    }
}

impl From<Meeting> for MeetingStatus {
    fn from(meeting: Meeting) -> Self {
        let members = MeetingMembers::load_members(meeting.id()).unwrap();

        let s = String::from(meeting.scheduled_cron());
        Self {
            is_ongoing: AtomicBool::new(false),
            meeting_data: meeting,
            members,
            handle: None,
            schedule: Schedule::from_str(&s).unwrap(),
        }
    }
}
