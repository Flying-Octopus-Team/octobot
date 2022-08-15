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
    }
}

joinable!(report -> member (member_id));

allow_tables_to_appear_in_same_query!(member, report,);
