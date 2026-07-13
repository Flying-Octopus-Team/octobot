use tracing::info;

use super::Context;
use crate::{error::Error, silent};

/// Show whether silent mode is currently enabled.
#[poise::command(
    slash_command,
    rename = "status",
    required_permissions = "ADMINISTRATOR",
    default_member_permissions = "ADMINISTRATOR"
)]
pub(crate) async fn status(ctx: Context<'_>) -> Result<(), Error> {
    let output = if silent::is_enabled() {
        "Silent mode is enabled. The bot will not act on its own (scheduled meetings are not \
         started). It still responds to commands."
    } else {
        "Silent mode is disabled. The bot acts normally (scheduled meetings are started)."
    };

    crate::discord::respond(ctx, output.to_string()).await
}

/// Enable silent mode: the bot stops acting on its own.
#[poise::command(
    slash_command,
    rename = "enable",
    required_permissions = "ADMINISTRATOR",
    default_member_permissions = "ADMINISTRATOR"
)]
pub(crate) async fn enable(ctx: Context<'_>) -> Result<(), Error> {
    silent::set_enabled(true);

    info!("Silent mode enabled by {}", ctx.author().name);

    crate::discord::respond(
        ctx,
        "Silent mode enabled. The bot will not start scheduled meetings or send anything on its \
         own. It still responds to commands."
            .to_string(),
    )
    .await
}

/// Disable silent mode: the bot resumes acting on its own.
#[poise::command(
    slash_command,
    rename = "disable",
    required_permissions = "ADMINISTRATOR",
    default_member_permissions = "ADMINISTRATOR"
)]
pub(crate) async fn disable(ctx: Context<'_>) -> Result<(), Error> {
    silent::set_enabled(false);

    info!("Silent mode disabled by {}", ctx.author().name);

    crate::discord::respond(
        ctx,
        "Silent mode disabled. Scheduled meetings will resume at the next scheduled time."
            .to_string(),
    )
    .await
}
