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
    "discord_token": "",
    "database_url": "postgresql://[user[:password]@][netloc][:port][/dbname][?param1=value1&...]",
    "member_role_id": 123456789012345678,
    "apprentice_role_id": 123456789012345678,
    "server_id": 123456789012345678,
    "meeting": {
        "cron": "",
        "channel_id": 123456789012345678
    },
    "summary_channel": 123456789012345678
}
```

Add your bot to the Discord server you've specified in the config, and make sure it has all required permissions to access the channels.
Run the bot using cargo

    cargo run --release

# Roadmap

- [X] Member management
  - [X] Create
  - [X] Read
  - [X] Update
  - [X] Delete
- [ ] Report management
  - [X] Create
  - [X] Read
  - [X] Update
  - [X] Delete
  - [ ] Check member's minimum activity
- [X] Weekly meetings
  - [X] Check attendance
  - [X] Write reports after the meeting
  - [X] Include attendees in the report
- [X] Discord commands
  - [X] User
  - [X] Report
  - [X] Weekly
- [ ] Trello integration
  - [ ] Auto-invite members
  - [ ] Update and sync member information
  - [ ] Report synchronization
- [ ] Wiki.js integration
  - [ ] Auto-invite members
  - [ ] Update and sync member information


# Contributors

Join our [Discord server](https://discord.gg/Q2DuSNY) to learn more about our plans and help us develop this tool!

# License 

Licensed under either of these:

  - Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
  - MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)
