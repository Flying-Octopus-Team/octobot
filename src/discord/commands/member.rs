use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::application::interaction::application_command::CommandDataOption;
use std::fmt::Write;
use tracing::info;
use uuid::Uuid;

use crate::framework::member::Member;
use crate::framework::member::MemberBuilder;

use super::find_option_value;

pub async fn add_member(
    ctx: &Context,
    _command: &ApplicationCommandInteraction,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Adding member");

    let member = MemberBuilder::from(option);
    member.check_for_duplicates(ctx).await?;

    let mut member = member.build(ctx).await;
    member.insert()?;
    member.setup(ctx).await?;

    info!("Member added: {:?}", member);

    Ok(format!("Added {}", member))
}

pub async fn remove_member(
    ctx: &Context,
    _command: &ApplicationCommandInteraction,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Removing member");
    let id = option.options[0]
        .value
        .as_ref()
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    let id = Uuid::parse_str(&id)?;

    let mut member = Member::get(id, ctx).await?;

    member.delete(ctx).await?;

    info!("Member removed: {:?}", member);

    Ok(format!("Removed {}", member))
}

pub async fn update_member(
    ctx: &Context,
    _command: &ApplicationCommandInteraction,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Updating member");
    let updated_member = MemberBuilder::from(option);

    let id = Uuid::parse_str(
        find_option_value(&option.options[..], "id")
            .unwrap()
            .as_str()
            .unwrap(),
    )?;

    let mut old_member = Member::get(id, ctx).await?;

    old_member.edit(updated_member, ctx).await.unwrap();

    old_member = old_member.update().unwrap();

    info!("Member updated: {:?}", old_member);

    Ok(format!("Updated {}", old_member))
}

pub async fn list_members(
    ctx: &Context,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Listing members");
    let page = find_option_value(&option.options[..], "page").map_or(1, |x| x.as_i64().unwrap());
    let page_size =
        find_option_value(&option.options[..], "page-size").map(|v| v.as_i64().unwrap());

    let member_filter = MemberBuilder::from(option);

    let (members, total_pages) = Member::list(member_filter, ctx, page, page_size).await?;

    let mut output = String::new();

    for member in members {
        writeln!(&mut output, "{}\n", member)?;
    }
    write!(&mut output, "Page: {page}/{total_pages}")?;

    Ok(output)
}
