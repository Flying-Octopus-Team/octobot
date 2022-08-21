table! {
    meeting (id) {
        id -> Uuid,
        start_date -> Timestamp,
        end_date -> Nullable<Timestamp>,
        summary_id -> Nullable<Uuid>,
        scheduled_cron -> Text,
    }
}

table! {
    meeting_members (id) {
        id -> Uuid,
        member_id -> Uuid,
        meeting_id -> Uuid,
    }
}

table! {
    member (id) {
        id -> Uuid,
        discord_id -> Nullable<Text>,
        trello_id -> Nullable<Text>,
        trello_report_card_id -> Nullable<Text>,
    }
}

table! {
    report (id) {
        id -> Uuid,
        member_id -> Uuid,
        content -> Text,
        create_date -> Date,
        published -> Bool,
        summary_id -> Nullable<Uuid>,
    }
}

table! {
    summary (id) {
        id -> Uuid,
        content -> Text,
        create_date -> Date,
    }
}

joinable!(meeting -> summary (summary_id));
joinable!(meeting_members -> meeting (meeting_id));
joinable!(meeting_members -> member (member_id));
joinable!(report -> member (member_id));
joinable!(report -> summary (summary_id));

allow_tables_to_appear_in_same_query!(meeting, meeting_members, member, report, summary,);
