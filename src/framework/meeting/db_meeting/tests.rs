use uuid::Uuid;

use crate::framework::member::db_member::Member;

use super::Meeting;
use super::MeetingMembers;

#[test]
fn crud_meeting() {
    // Create, Read, Update, Delete
    let now = chrono::Local::now();
    let mut meeting = Meeting::new(now, String::from("0 0 0 * * *"), String::from("channel_id"));

    assert_eq!(meeting.start_date(), now.naive_local());
    assert_eq!(meeting.end_date(), None);
    assert_eq!(meeting.channel_id, String::from("channel_id"));
    assert_eq!(meeting.scheduled_cron, String::from("0 0 0 * * *"));

    let inserted_meeting = meeting.insert().unwrap();

    let latest_meeting = Meeting::get_latest().unwrap();

    assert_eq!(latest_meeting.id, inserted_meeting.id);

    let find_by_id = Meeting::find_by_id(inserted_meeting.id).unwrap();
    let find_by_summary_id = Meeting::find_by_summary_id(inserted_meeting.summary_id).unwrap();

    assert_eq!(find_by_id.id, inserted_meeting.id);
    assert_eq!(find_by_summary_id.id, inserted_meeting.id);

    meeting.channel_id = String::from("new_channel_id");

    let updated_meeting = meeting.update().unwrap();

    assert_eq!(updated_meeting.channel_id, String::from("new_channel_id"));

    updated_meeting.delete().unwrap();

    let find_by_id = Meeting::find_by_id(inserted_meeting.id);

    assert!(find_by_id.is_err());
}

#[test]
fn meeting_members() {
    let now = chrono::Local::now();
    let meeting = Meeting::new(now, String::from("0 0 0 * * *"), String::from("channel_id"));

    let inserted_meeting = meeting.insert().unwrap();

    let member = Member {
        id: Uuid::new_v4(),
        display_name: String::from("name"),
        discord_id: Some(String::from("discord_id")),
        trello_id: Some(String::from("trello_id")),
        trello_report_card_id: Some(String::from("trello_report_card_id")),
        is_apprentice: false,
    };

    let inserted_member = member.insert().unwrap();

    let meeting_member = MeetingMembers::new(inserted_meeting.id, inserted_member.id);

    let inserted_meeting_member = meeting_member.insert().unwrap();

    assert!(MeetingMembers::is_user_in_meeting(inserted_meeting.id, inserted_member.id).unwrap());

    let find_by_meeting_id = MeetingMembers::load_members(inserted_meeting.id).unwrap();

    assert_eq!(find_by_meeting_id.len(), 1);
    assert_eq!(find_by_meeting_id[0].id, inserted_meeting_member.id);

    MeetingMembers::delete_by_meeting_and_member(inserted_meeting.id, inserted_member.id).unwrap();

    let find_by_meeting_id = MeetingMembers::load_members(inserted_meeting.id).unwrap();

    assert_eq!(find_by_meeting_id.len(), 0);

    inserted_meeting.delete().unwrap();
    inserted_member.delete().unwrap();
}
