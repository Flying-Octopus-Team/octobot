use std::fmt::Display;
use std::fmt::Formatter;

use chrono::NaiveDateTime;
use cron::Schedule;
use serenity::model::prelude::Channel;
use serenity::http::CacheHttp;
use tracing::log::error;
use uuid::Uuid;

use super::summary::Summary;
use super::member::Member;
use crate::database::models::meeting::MeetingMembers;
use crate::database::models::meeting::MeetingFilter;
use crate::database::models::meeting::Meeting as DbMeeting;

pub struct Meeting {
    pub id: Uuid,
    pub start_date: NaiveDateTime,
    pub end_date: Option<NaiveDateTime>,
    pub summary: Summary,
    pub channel: Channel,
    pub schedule: Schedule,
    pub members: Vec<(Uuid, Member)>,
}

impl Meeting {
    pub async fn get(
        cache_http: &impl CacheHttp,
        id: Uuid,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let db_meeting = match DbMeeting::find_by_id(id) {
            Ok(meeting) => meeting,
            Err(e) => {
                error!("Error while getting meeting from database: {}", e);
                return Err(e.into());
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

        let channel = match cache_http.cache().map(|c| c.channel(channel_id)).flatten() {
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
                return Err(e.into());
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

    pub fn summary(&self) -> &Summary {
        &self.summary
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
