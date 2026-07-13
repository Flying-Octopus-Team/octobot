//! Global "silent mode" switch.
//!
//! When silent mode is enabled the bot must not act on its own: no starting
//! scheduled meetings, no unprompted channel messages, no announcements.
//! Responding to explicit commands and interactions is still allowed.
//!
//! Every proactive code path checks [`is_enabled`] at its source before
//! acting. The initial state comes from the `silent_mode` config option
//! (default: enabled) and can be toggled at runtime by admins with the
//! `/silent-mode` command.

use std::sync::atomic::{AtomicBool, Ordering::SeqCst};

/// Defaults to `true` so the bot stays quiet even if [`init`] is never
/// called.
static SILENT_MODE: AtomicBool = AtomicBool::new(true);

/// Sets the initial silent mode state from the settings. Called once on
/// startup.
pub fn init(enabled: bool) {
    SILENT_MODE.store(enabled, SeqCst);
}

/// Returns `true` when silent mode is enabled and proactive actions must be
/// skipped.
pub fn is_enabled() -> bool {
    SILENT_MODE.load(SeqCst)
}

/// Enables or disables silent mode at runtime.
pub fn set_enabled(enabled: bool) {
    SILENT_MODE.store(enabled, SeqCst);
}
