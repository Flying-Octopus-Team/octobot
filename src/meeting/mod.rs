use std::{
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc,
    },
};

use chrono::Local;
use cron::Schedule;
use serenity::{client::Cache, prelude::TypeMapKey};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    database::models::{
        meeting::{Meeting, MeetingMembers},
        member::Member,
    },
    SETTINGS,
};

#[derive(Default, Debug)]
pub struct MeetingStatus {
    pub is_meeting_ongoing: Arc<AtomicBool>,
    pub meeting_data: Option<Meeting>,
    pub members: Vec<MeetingMembers>,
}

impl TypeMapKey for MeetingStatus {
    type Value = Arc<RwLock<MeetingStatus>>;
}

pub async fn create_meeting_job(
    cache: Arc<Cache>,
) -> Result<Arc<RwLock<MeetingStatus>>, Box<dyn std::error::Error>> {
    let meeting_status = Arc::new(RwLock::new(MeetingStatus::new()));
    let meeting_status_clone = meeting_status.clone();
    let schedule = Schedule::from_str(SETTINGS.meeting.cron.as_str()).unwrap();
    
    tokio::spawn(async move {
        let meeting_status = meeting_status_clone;
        while let Some(datetime) = schedule.upcoming(Local).next() {
            let duration = datetime
                .signed_duration_since(Local::now())
                .to_std()
                .unwrap();
            tokio::time::sleep(duration).await;
            run_job(&meeting_status, datetime, &cache).await;
        }
    });

    Ok(meeting_status)
}

async fn run_job(
    meeting_status: &Arc<RwLock<MeetingStatus>>,
    datetime: chrono::DateTime<Local>,
    cache: &Arc<Cache>,
) {
    let mut meeting_status = meeting_status.write().await;
    meeting_status.is_meeting_ongoing.store(true, SeqCst);
    meeting_status.meeting_data = Some(Meeting::new(datetime, SETTINGS.meeting.cron.clone()));
    let channel = cache.guild_channel(SETTINGS.meeting.channel_id).unwrap();
    meeting_status.members = channel
        .members(cache)
        .await
        .unwrap()
        .into_iter()
        .map(|member| {
            let member_id = Member::find_by_discord_id(member.user.id.0.to_string())
                .unwrap()
                .id();
            MeetingMembers::new(member_id, meeting_status.meeting_data.as_ref().unwrap().id)
        })
        .collect();
}

impl MeetingStatus {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn is_meeting_ongoing(&self) -> bool {
        self.is_meeting_ongoing.load(SeqCst)
    }

    pub fn meeting_data(&self) -> Option<&Meeting> {
        self.meeting_data.as_ref()
    }

    /// Ends the meeting and inserts data to the database.
    /// Clears the meeting data and members.
    pub fn end_meeting(&self) -> Result<Self, Box<dyn std::error::Error>> {
        let mut meeting = MeetingStatus {
            meeting_data: self.meeting_data.clone(),
            members: self.members.clone(),
            ..Default::default()
        };

        meeting.meeting_data = meeting.meeting_data.map(|mut meeting| {
            meeting.end_meeting(chrono::offset::Local::now());
            meeting
        });

        meeting.insert()?;

        Ok(MeetingStatus::new())
    }

    pub fn insert(self) -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(meeting) = self.meeting_data() {
            meeting.insert()?;
        }

        for member in &self.members {
            member.insert()?;
        }

        Ok(self)
    }

    pub fn add_member(
        &mut self,
        member_id: impl Into<Uuid>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.members.push(MeetingMembers::new(
            member_id,
            self.meeting_data().unwrap().id,
        ));

        Ok(())
    }
}
