use std::{
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc,
    },
    time::Duration,
};

use chrono::Local;
use cron::Schedule;
use poise::{
    serenity_prelude as serenity,
    serenity_prelude::{prelude::TypeMapKey, CacheHttp},
};
use serenity::{Cache, ChannelId, GuildId};
use tokio::{sync::RwLock, task::JoinHandle};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{
    database::models::{
        meeting::{Meeting, MeetingMembers},
        member::Member,
    },
    error::Error,
    SETTINGS,
};

/// Struct that holds the current meeting status.
/// It is used to keep track of the meeting's members and to check if the
/// meeting is ongoing. Creates and manages task that is responsible for
/// starting the meeting.
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
    ctx: &serenity::Context,
) -> Result<Arc<RwLock<MeetingStatus>>, Error> {
    let meeting_status = MeetingStatus::load_next_meeting()?;

    let meeting_status = Arc::new(RwLock::new(meeting_status));

    MeetingStatus::await_meeting(Arc::clone(&meeting_status), ctx).await;

    Ok(meeting_status)
}

impl MeetingStatus {
    pub fn new(scheduled_cron: &str, channel_id: String) -> Result<Self, Error> {
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
        scheduled_cron: Schedule,
        ctx: &serenity::Context,
    ) -> Result<(), Error> {
        {
            let mut meeting_status = meeting_status.write().await;

            meeting_status.abort_meeting();
            meeting_status.set_is_ongoing(false);

            meeting_status.schedule = scheduled_cron.clone();
            meeting_status
                .meeting_data
                .set_schedule(scheduled_cron.to_owned())?;
        }

        MeetingStatus::await_meeting(meeting_status, ctx).await;

        Ok(())
    }

    pub fn change_channel(&mut self, channel_id: String) -> Result<(), Error> {
        match self.meeting_data.set_channel_id(channel_id) {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Error while changing channel: {}", e);
                Err(e)
            }
        }
    }

    fn abort_meeting(&self) {
        info!("Aborting meeting {:?}", self.meeting_data);
        self.handle.as_ref().unwrap().abort();
    }

    pub fn is_meeting_ongoing(&self) -> bool {
        self.is_ongoing.load(SeqCst)
    }

    pub fn meeting_id(&self) -> Uuid {
        self.meeting_data.id()
    }

    pub fn schedule(&self) -> Result<Schedule, Error> {
        self.meeting_data.schedule()
    }

    pub fn members(&self) -> Vec<MeetingMembers> {
        self.members.clone()
    }

    pub fn channel(&self) -> &str {
        self.meeting_data.channel_id()
    }

    /// Ends the meeting and inserts data to the database. Updates given meeting
    /// status. Clears the meeting data and members.
    pub async fn end_meeting(
        ctx: &serenity::Context,
        meeting_status: Arc<RwLock<MeetingStatus>>,
    ) -> Result<(), Error> {
        {
            let mut meeting = meeting_status.write().await;

            meeting.set_is_ongoing(false);

            meeting.meeting_data.end_meeting(Local::now())?;

            let scheduled_cron = String::from(meeting.meeting_data.scheduled_cron());

            let channel_id = meeting.channel().to_string();

            let end_time = meeting.meeting_data.end_date.unwrap();

            let members = meeting.meeting_data.members()?;

            for mut member in members {
                member.set_last_activity(end_time.into());

                member.update()?;
            }

            *meeting = MeetingStatus::new(&scheduled_cron, channel_id)?;
        }

        MeetingStatus::await_meeting(meeting_status.clone(), ctx).await;

        Ok(())
    }

    pub fn add_member(&mut self, member: &mut Member) -> Result<String, Error> {
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

    pub fn remove_member(&mut self, member: &mut Member) -> Result<String, Error> {
        self.members.retain(|m| m.member_id() != member.id());

        let meeting = self.meeting();

        meeting.remove_member(member)
    }

    /// Loads the next meeting from the database, or defaults to a new meeting.
    fn load_next_meeting() -> Result<Self, Error> {
        let meeting_data = Meeting::load_next_meeting()?;

        let meeting_status = MeetingStatus::try_from(meeting_data)?;

        Ok(meeting_status)
    }

    /// Saves the meeting to the database and creates a task that will start the
    /// meeting.
    ///
    /// The task will be cancelled if the schedule is changed.
    ///
    /// This is the central guard for silent mode: while silent mode is
    /// enabled, the task never starts a meeting. Instead it re-checks at
    /// every scheduled occurrence, so disabling silent mode makes meetings
    /// resume at their next scheduled time without a restart.
    ///
    /// It is also the guard for the presence gate (`require_presence`): a
    /// meeting never starts into an empty voice channel, independent of
    /// silent mode. This applies even when silent mode is disabled.
    async fn await_meeting(meeting_status: Arc<RwLock<Self>>, ctx: &serenity::Context) {
        let meeting_status_clone = Arc::clone(&meeting_status);
        let cache = ctx.cache().unwrap().clone();
        let join_handle = tokio::spawn(async move {
            let meeting_status = meeting_status_clone;
            info!(
                "Waiting for the meeting {}",
                meeting_status.read().await.meeting_id()
            );

            loop {
                let duration = meeting_status.read().await.load_duration().unwrap();

                if duration.as_secs() > 0 {
                    info!("Sleeping for {:?}", duration);
                    tokio::time::sleep(duration).await;
                }

                if crate::silent::is_enabled() {
                    info!("Silent mode is enabled; not starting the scheduled meeting");

                    // Sleep until the next scheduled occurrence and check
                    // again, instead of starting the meeting.
                    if Self::wait_for_next_occurrence(&meeting_status).await {
                        continue;
                    } else {
                        break;
                    }
                }

                if SETTINGS.require_presence {
                    let has_human = {
                        let meeting_status = meeting_status.read().await;
                        meeting_status.voice_channel_has_human(&cache)
                    };

                    if !has_human {
                        info!(
                            "No human present in the meeting voice channel; not starting the scheduled meeting"
                        );

                        // Sleep until the next scheduled occurrence and check
                        // again, instead of starting the meeting.
                        if Self::wait_for_next_occurrence(&meeting_status).await {
                            continue;
                        } else {
                            break;
                        }
                    }
                }

                let mut meeting_status = meeting_status.write().await;

                meeting_status.set_is_ongoing(true);

                match meeting_status.start_meeting(&cache).await {
                    Ok(_) => {
                        info!("Meeting started");
                    }
                    Err(e) => error!("Error creating meeting job: {:?}", e),
                }

                break;
            }
        });
        meeting_status.write().await.handle = Some(join_handle);
    }

    /// Sleeps until the next scheduled occurrence of the meeting's cron
    /// schedule.
    ///
    /// Returns `true` if the caller should re-check its gates (silent mode /
    /// presence) and try again, or `false` if there is no upcoming
    /// occurrence and the polling job should stop entirely.
    async fn wait_for_next_occurrence(meeting_status: &Arc<RwLock<Self>>) -> bool {
        let next_check = {
            let meeting_status = meeting_status.read().await;
            meeting_status.schedule.upcoming(Local).next().map(|next| {
                next.signed_duration_since(Local::now())
                    .to_std()
                    .unwrap_or_default()
            })
        };

        match next_check {
            Some(duration) => {
                info!("Re-checking in {:?}", duration);
                tokio::time::sleep(duration).await;
                true
            }
            None => {
                error!("No upcoming occurrence in the schedule; stopping meeting job");
                false
            }
        }
    }

    /// Looks up the meeting's voice channel in the given cache.
    fn voice_channel(&self, cache: &Arc<Cache>) -> Result<serenity::GuildChannel, Error> {
        let guild_id = GuildId::new(SETTINGS.discord.server_id.get());
        let channel_id = ChannelId::new(self.channel().parse::<u64>()?);

        let guild = match cache.guild(guild_id) {
            Some(g) => g,
            None => {
                error!("Guild not found in cache");
                return Err(Error::GuildChannelNotFound);
            }
        };

        match guild.channels.get(&channel_id) {
            Some(c) => Ok(c.clone()),
            None => {
                error!("Channel not found in guild");
                Err(Error::GuildChannelNotFound)
            }
        }
    }

    /// Checks whether the meeting's voice channel currently has at least one
    /// human (non-bot) member connected.
    ///
    /// This is the presence gate: an additional, independent safety check on
    /// top of silent mode. If the channel/guild state cannot be determined
    /// (cache miss, lookup error, etc.) this conservatively returns `false`
    /// (treats the channel as empty) so a meeting is never started without
    /// positive confirmation that a human is present.
    fn voice_channel_has_human(&self, cache: &Arc<Cache>) -> bool {
        let members = self
            .voice_channel(cache)
            .and_then(|channel| Ok(channel.members(cache)?));

        match members {
            Ok(members) => has_human_presence(members.iter().map(|member| member.user.bot)),
            Err(e) => {
                warn!(
                    "Could not determine voice channel presence, treating as empty: {}",
                    e
                );
                false
            }
        }
    }

    fn load_duration(&self) -> Result<Duration, Error> {
        let schedule = self.schedule()?;

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
            Err(Error::NoMeetingPlanned)
        }
    }

    /// Starts the meeting and saves current users in the meeting channel
    async fn start_meeting(&mut self, cache: &Arc<Cache>) -> Result<(), Error> {
        let channel = self.voice_channel(cache)?;

        for member in channel.members(cache)? {
            let mut member = {
                let member_result = Member::find_by_discord_id(member.user.id.get().to_string());
                match member_result {
                    Ok(m) => m,
                    Err(e) => {
                        error!("Error getting member: {}", e);
                        continue;
                    }
                }
            };
            match self.add_member(&mut member) {
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
}

impl TryFrom<Meeting> for MeetingStatus {
    type Error = Error;

    fn try_from(meeting: Meeting) -> Result<Self, Self::Error> {
        let members = MeetingMembers::load_members(meeting.id())?;

        let s = meeting.scheduled_cron().to_string();

        Ok(Self {
            is_ongoing: AtomicBool::new(false),
            meeting_data: meeting,
            members,
            handle: None,
            schedule: Schedule::from_str(&s).unwrap(),
        })
    }
}

/// Returns `true` if at least one of the given members is not a bot.
///
/// Each item is a member's `user.bot` flag (`true` for bots, `false` for
/// humans). This is a pure helper, deliberately kept free of any Discord
/// cache/HTTP access, so the presence gate's core decision can be unit
/// tested without standing up a live cache or constructing `serenity`
/// model types.
fn has_human_presence<I>(bot_flags: I) -> bool
where
    I: IntoIterator<Item = bool>,
{
    bot_flags.into_iter().any(|is_bot| !is_bot)
}

#[cfg(test)]
mod tests {
    use super::has_human_presence;

    #[test]
    fn empty_channel_has_no_human() {
        assert!(!has_human_presence(vec![]));
    }

    #[test]
    fn only_bots_has_no_human() {
        assert!(!has_human_presence(vec![true, true, true]));
    }

    #[test]
    fn single_human_is_detected() {
        assert!(has_human_presence(vec![false]));
    }

    #[test]
    fn human_among_bots_is_detected() {
        assert!(has_human_presence(vec![true, false, true]));
    }
}
