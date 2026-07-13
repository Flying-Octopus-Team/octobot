# Octobot

Discord bot, that allows its users to integrate different services used in Flying Octopus together.

# Features

Add organization's member and manage their profiles and statuses on multiple services using Discord's commands.
Manage and sync reports written by users across services.

# Motivation

- Minimize the amount of work non-tech savvy member's of the team have to do while adding new members of the team.
- Reduce amount of manual copy-pasting required to keep all reports synchronized
- Track member's activity to comply with internal rules

# Requirements

- [Rust](https://www.rust-lang.org/learn/get-started)
- [PostgreSQL](https://www.postgresql.org/download/)

# Installation

Clone repository and create config file in `config/config.json` following this template

```json
{
  "database_url": "postgresql://[user[:password]@][netloc][:port][/dbname][?param1=value1&...]",
  "activity_threshold_days": 123,
  "silent_mode": true,
  "require_presence": true,
  "meeting": {
    "cron": "",
    "channel_id": 123456789012345678
  },
  "discord": {
    "token": "",
    "member_role_id": 123456789012345678,
    "apprentice_role_id": 123456789012345678,
    "server_id": 123456789012345678,
    "summary_channel": 123456789012345678
  },
  "wiki": {
    "token": "Bearer very_long_token",
    "url": "https://wiki.example.com",
    "graphql": "https://wiki.example.com/graphql",
    "provider_key": "long-key-for-discord-provider",
    "member_group_id": 123456789012345678,
    "guest_group_id": 123456789012345678
  }
}
```

`silent_mode` controls whether the bot may act on its own (start scheduled meetings, send unprompted messages). It defaults to `true` (silent) when omitted, so the bot only responds to commands. Server administrators can toggle it at runtime with the `/silent-mode enable`, `/silent-mode disable` and `/silent-mode status` commands.

`require_presence` controls an independent safety gate: when enabled (the default), a scheduled meeting will not start unless at least one human (non-bot) member is already connected to the meeting's voice channel. This check applies even when `silent_mode` is disabled, and there is no Discord command to toggle it at runtime — change it in the config file. If the channel's presence cannot be determined (e.g. cache miss or API error), the bot conservatively treats the channel as empty and does not start the meeting.

Add your bot to the Discord server you've specified in the config, and make sure it has all required permissions to access the channels.
Run the bot using cargo

    cargo run --release

# Roadmap

- [x] Member management
  - [x] Create
  - [x] Read
  - [x] Update
  - [x] Delete
- [x] Report management
  - [x] Create
  - [x] Read
  - [x] Update
  - [x] Delete
  - [x] Check member's minimum activity
- [x] Weekly meetings
  - [x] Check attendance
  - [x] Write reports after the meeting
  - [x] Include attendees in the report
- [x] Discord commands
  - [x] User
  - [x] Report
  - [x] Weekly
- [ ] Trello integration
  - [ ] Auto-invite members
  - [ ] Update and sync member information
  - [ ] Report synchronization
- [ ] Wiki.js integration
  - [x] Auto-invite members
  - [x] Add and remove member from groups
  - [ ] Update and sync member information

# Contributors

Join our [Discord server](https://discord.gg/Q2DuSNY) to learn more about our plans and help us develop this tool!

# License

Licensed under either of these:

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)
