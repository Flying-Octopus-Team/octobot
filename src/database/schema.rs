// @generated automatically by Diesel CLI.

diesel::table! {
    meeting (id) {
        id -> Uuid,
        start_date -> Timestamp,
        end_date -> Nullable<Timestamp>,
        summary_id -> Uuid,
        channel_id -> Text,
        scheduled_cron -> Text,
    }
}

diesel::table! {
    meeting_members (id) {
        id -> Uuid,
        member_id -> Uuid,
        meeting_id -> Uuid,
    }
}

diesel::table! {
    member (id) {
        id -> Uuid,
        display_name -> Text,
        discord_id -> Nullable<Text>,
        trello_id -> Nullable<Text>,
        trello_report_card_id -> Nullable<Text>,
        role -> Int4,
        wiki_id -> Nullable<Int8>,
    }
}

diesel::table! {
    report (id) {
        id -> Uuid,
        member_id -> Uuid,
        content -> Text,
        create_date -> Date,
        published -> Bool,
        summary_id -> Nullable<Uuid>,
    }
}

diesel::table! {
    summary (id) {
        id -> Uuid,
        note -> Text,
        create_date -> Date,
        messages_id -> Nullable<Array<Text>>,
    }
}

diesel::joinable!(meeting -> summary (summary_id));
diesel::joinable!(meeting_members -> meeting (meeting_id));
diesel::joinable!(meeting_members -> member (member_id));
diesel::joinable!(report -> member (member_id));
diesel::joinable!(report -> summary (summary_id));

diesel::allow_tables_to_appear_in_same_query!(meeting, meeting_members, member, report, summary,);
