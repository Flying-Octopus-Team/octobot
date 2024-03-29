use poise::serenity_prelude as serenity;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Error with serenity: {0}")]
    SerenityError(#[from] serenity::Error),
    #[error("Error operating with settings: {0}")]
    ConfigError(#[from] config::ConfigError),
    #[error("Error formatting: {0}")]
    FmtError(#[from] std::fmt::Error),
    #[error("Error with reqwest: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Error from wiki: {source}")]
    WikiError {
        #[from]
        source: crate::wiki::WikiError,
    },
    #[error("Error parsing schedule: {source}")]
    CronError {
        #[from]
        source: cron::error::Error,
    },
    #[error("Error parsing: {source}")]
    ParseIntError {
        #[from]
        source: std::num::ParseIntError,
    },
    #[error("Error with diesel: {source}")]
    DieselError {
        #[from]
        source: diesel::result::Error,
    },
    #[error("Error with r2d2: {source}")]
    R2d2Error {
        #[from]
        source: r2d2::Error,
    },
    #[error("User {} already in meeting {}", .user_id, .meeting_id)]
    UserAlreadyInMeeting {
        user_id: uuid::Uuid,
        meeting_id: uuid::Uuid,
    },
    #[error("User {} not in meeting {}", .user_id, .meeting_id)]
    UserNotInMeeting {
        user_id: uuid::Uuid,
        meeting_id: uuid::Uuid,
    },
    #[error("Summary too long to fit in messages")]
    SummaryTooLong,
    #[error("No messages to send summary to")]
    NoSummaryMessages,
    #[error("No meeting is ongoing")]
    NoMeetingOngoing,
    #[error("No meeting is planned")]
    NoMeetingPlanned,
    #[error("Guild channel not found")]
    GuildChannelNotFound,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
