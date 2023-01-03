use super::*;

#[test]
fn crud_member() {
    // Create, Read, Update, Delete
    let mut member = Member::new(
        String::from("name"),
        Some(String::from("discord_id")),
        Some(String::from("trello_id")),
        Some(String::from("trello_report_card_id")),
        false,
    );

    let inserted_member = member.insert().unwrap();

    assert_eq!(inserted_member.display_name, String::from("name"));
    assert_eq!(inserted_member.discord_id, Some(String::from("discord_id")));
    assert_eq!(inserted_member.trello_id, Some(String::from("trello_id")));
    assert_eq!(
        inserted_member.trello_report_card_id,
        Some(String::from("trello_report_card_id"))
    );
    assert_eq!(inserted_member.is_apprentice, false);

    let find_by_id = Member::find_by_id(inserted_member.id).unwrap();

    assert_eq!(find_by_id.id, inserted_member.id);

    let find_by_discord_id = Member::find_by_discord_id(inserted_member.discord_id.unwrap())
        .unwrap()
        .unwrap();

    assert_eq!(find_by_discord_id.id, inserted_member.id);

    let find_by_trello_id = Member::find_by_trello_id(inserted_member.trello_id.unwrap())
        .unwrap()
        .unwrap();

    assert_eq!(find_by_trello_id.id, inserted_member.id);

    member.display_name = String::from("new_name");

    let updated_member = member.update().unwrap();

    assert_eq!(updated_member.display_name, String::from("new_name"));

    updated_member.delete().unwrap();

    let find_by_id = Member::find_by_id(inserted_member.id);

    assert!(find_by_id.is_err());
}
