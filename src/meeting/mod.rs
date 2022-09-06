use std::{str::FromStr, sync::Arc, time::Duration};

use chrono::Local;
use cron::Schedule;
use parking_lot::{Condvar, Mutex};
use serenity::client::Cache;
use serenity::prelude::TypeMapKey;
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
    is_ongoing: Arc<(Mutex<bool>, Condvar)>,
    meeting_data: Meeting,
    members: Vec<MeetingMembers>,
    handle: Option<JoinHandle<()>>,
    schedule: Schedule,
}

impl TypeMapKey for MeetingStatus {
    type Value = Arc<RwLock<MeetingStatus>>;
}

pub async fn create_meeting_job(
    cache: Arc<Cache>,
) -> Result<Arc<RwLock<MeetingStatus>>, Box<dyn std::error::Error>> {
    let meeting_status = MeetingStatus::load_next_meeting()?;

    let meeting_status = Arc::new(RwLock::new(meeting_status));

    MeetingStatus::await_meeting(Arc::clone(&meeting_status), cache).await;

    Ok(meeting_status)
}

async fn start_meeting(
    meeting_status: &Arc<RwLock<MeetingStatus>>,
    cache: &Arc<Cache>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut meeting_status = meeting_status.write().await;
    {
        let (is_ongoing, condvar) = meeting_status.is_ongoing.as_ref();
        let mut lock = is_ongoing.lock();
        *lock = true;
        condvar.notify_all();
    }
    let channel = cache
        .guild_channel(SETTINGS.meeting.channel_id)
        .unwrap_or_else(|| panic!("Channel not found {}", SETTINGS.meeting.channel_id));
    for member in channel.members(cache).await? {
        let member = {
            let member_result = Member::find_by_discord_id(member.user.id.0.to_string());
            match member_result {
                Ok(t) => t,
                Err(_) => continue,
            }
        };
        match meeting_status.add_member(&member) {
            Ok(_) => {}
            Err(e) => {
                error!("Error adding member to meeting: {}", e);
            }
        }
    }

    Ok(())
}

impl MeetingStatus {
    pub fn new(
        scheduled_cron: &str,
        channel_id: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let meeting_status = Self {
            is_ongoing: Arc::new((Mutex::new(false), Condvar::new())),
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
        cache: Arc<Cache>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut meeting_status = meeting_status.write().await;

            meeting_status.abort_meeting();

            meeting_status.schedule = Schedule::from_str(scheduled_cron)?;
            meeting_status.meeting_data =
                Meeting::try_from_cron(scheduled_cron, meeting_status.channel().to_string())?;
        }

        MeetingStatus::await_meeting(meeting_status, cache).await;

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
        let (is_ongoing, _) = self.is_ongoing.as_ref();
        *is_ongoing.lock()
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

    /// Ends the meeting and inserts data to the database.
    /// Clears the meeting data and members.
    pub fn end_meeting(&self, summary_note: String) -> Result<Self, Box<dyn std::error::Error>> {
        let mut meeting = MeetingStatus {
            meeting_data: self.meeting_data.clone(),
            members: self.members.clone(),
            is_ongoing: self.is_ongoing.clone(),
            handle: None,
            schedule: self.schedule()?,
        };
        {
            let mut lock = meeting.is_ongoing.0.lock();
            *lock = false;
            meeting.is_ongoing.1.notify_all();
        }
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

        MeetingStatus::new(&scheduled_cron, channel_id)
    }

    pub fn add_member(&mut self, member: &Member) -> Result<String, Box<dyn std::error::Error>> {
        let meeting = self.meeting();
        match meeting.add_member(member) {
            Ok(msg) => {
                println!("{} joined", member.name());
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
            is_ongoing: Arc::new((Mutex::new(false), Condvar::new())),
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
    async fn await_meeting(meeting_status: Arc<RwLock<Self>>, cache: Arc<Cache>) {
        let meeting_status_clone = Arc::clone(&meeting_status);
        let join_handle = tokio::spawn(async move {
            let meeting_status = meeting_status_clone;
            info!(
                "Awaiting meeting {}",
                meeting_status.read().await.meeting_id()
            );
            let schedule = match meeting_status.read().await.schedule() {
                Ok(s) => s,
                Err(e) => {
                    error!("Error while getting schedule: {}", e);
                    return;
                }
            };
            while let Some(datetime) = schedule.upcoming(Local).next() {
                let mut duration = datetime
                    .signed_duration_since(Local::now())
                    .to_std()
                    .unwrap();
                {
                    // check if the given meeting data already exists in the database
                    if meeting_status.read().await.meeting_data.exists().unwrap() {
                        // if it does, update the meeting data
                        duration = if meeting_status.read().await.meeting_data.start_date()
                            > Local::now().naive_local()
                            && !meeting_status.read().await.is_meeting_ongoing()
                        {
                            meeting_status
                                .read()
                                .await
                                .meeting_data
                                .start_date()
                                .signed_duration_since(Local::now().naive_local())
                                .to_std()
                                .unwrap()
                        } else {
                            Duration::from_secs(0)
                        };
                        meeting_status.write().await.meeting_data.update().unwrap();
                    } else {
                        // if it doesn't, insert the meeting data
                        meeting_status.write().await.meeting_data.insert().unwrap();
                    }
                }

                if duration.as_secs() > 0 {
                    info!("Waiting for meeting to start in {:?}", duration);
                    tokio::time::sleep(duration).await;
                }

                match start_meeting(&meeting_status, &cache).await {
                    Ok(_) => {}
                    Err(e) => error!("Error creating meeting job: {:?}", e),
                }
                // wait for meeting to end
                let is_ongoing;
                {
                    is_ongoing = meeting_status.read().await.is_ongoing.clone();
                }
                let mut lock = is_ongoing.0.lock();
                if *lock {
                    is_ongoing.1.wait(&mut lock);
                }
            }
        });
        meeting_status.write().await.handle = Some(join_handle);
    }

    pub fn meeting(&self) -> &Meeting {
        &self.meeting_data
    }

    pub(crate) fn summary_id(&self) -> Uuid {
        self.meeting_data.summary_id()
    }
}
