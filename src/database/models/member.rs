use crate::database::schema::member;
use crate::diesel::query_dsl::filter_dsl::FilterDsl;
use crate::diesel::ExpressionMethods;
use crate::diesel::RunQueryDsl;
use uuid::Uuid;

#[derive(Queryable, Insertable, Debug)]
#[diesel(table_name = member)]
struct Member {
    id: Uuid,
    discord_id: Option<String>,
    trello_id: Option<String>,
    trello_report_card_id: Option<String>,
}

fn create_member(
    discord_id: Option<String>,
    trello_id: Option<String>,
    trello_report_card_id: Option<String>,
) -> Member {
    let new_member = Member {
        id: Uuid::new_v4(),
        discord_id,
        trello_id,
        trello_report_card_id,
    };

    diesel::insert_into(member::table)
        .values(&new_member)
        .get_result(&mut crate::database::PG_POOL.get().unwrap())
        .expect("Error creating new member")
}

fn get_member_by_id(id: &Uuid) -> Option<Member> {
    use crate::database::schema::member::dsl::*;

    member
        .filter(id.eq(id))
        .first(&mut crate::database::PG_POOL.get().unwrap())
        .ok()
}

fn get_member_by_discord_id(discord_id: &str) -> Option<Member> {
    use crate::database::schema::member::dsl::*;

    member
        .filter(discord_id.eq(discord_id))
        .first(&mut crate::database::PG_POOL.get().unwrap())
        .ok()
}

fn get_member_by_trello_id(trello_id: &str) -> Option<Member> {
    use crate::database::schema::member::dsl::*;

    member
        .filter(trello_id.eq(trello_id))
        .first(&mut crate::database::PG_POOL.get().unwrap())
        .ok()
}

fn get_member_by_trello_report_card_id(trello_report_card_id: &str) -> Option<Member> {
    use crate::database::schema::member::dsl::*;

    member
        .filter(trello_report_card_id.eq(trello_report_card_id))
        .first(&mut crate::database::PG_POOL.get().unwrap())
        .ok()
}

fn update_member(
    id: &Uuid,
    discord_id: Option<String>,
    trello_id: Option<String>,
    trello_report_card_id: Option<String>,
) -> Member {
    use crate::database::schema::member::dsl;

    diesel::update(dsl::member.filter(dsl::id.eq(id)))
        .set((
            dsl::discord_id.eq(discord_id),
            dsl::trello_id.eq(trello_id),
            dsl::trello_report_card_id.eq(trello_report_card_id),
        ))
        .get_result(&mut crate::database::PG_POOL.get().unwrap())
        .expect("Error updating member")
}

fn delete_member(id: &Uuid) -> bool {
    use crate::database::schema::member::dsl;

    diesel::delete(dsl::member.filter(dsl::id.eq(id)))
        .execute(&mut crate::database::PG_POOL.get().unwrap())
        .is_ok()
}
