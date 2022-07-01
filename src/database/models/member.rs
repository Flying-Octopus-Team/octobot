use crate::database::schema::member;
use crate::diesel::ExpressionMethods;
use crate::diesel::RunQueryDsl;
use diesel::QueryDsl;
use uuid::Uuid;

#[derive(Queryable, Insertable, AsChangeset, Debug)]
#[diesel(table_name = member)]
pub struct Member {
    id: Uuid,
    discord_id: Option<String>,
    trello_id: Option<String>,
    trello_report_card_id: Option<String>,
}

impl Member {
    pub fn new(
        discord_id: Option<String>,
        trello_id: Option<String>,
        trello_report_card_id: Option<String>,
    ) -> Member {
        Member {
            id: Uuid::new_v4(),
            discord_id,
            trello_id,
            trello_report_card_id,
        }
    }

    pub fn insert(&self) -> Member {
        diesel::insert_into(member::table)
            .values(self)
            .get_result(&mut crate::database::PG_POOL.get().unwrap())
            .expect("Error creating new member")
    }

    pub fn update(&self) -> Self {
        diesel::update(member::table)
            .set(self)
            .get_result(&mut crate::database::PG_POOL.get().unwrap())
            .expect("Error updating member")
    }

    pub fn delete(&self) -> bool {
        use crate::database::schema::member::dsl::*;

        diesel::delete(member.filter(id.eq(id)))
            .execute(&mut crate::database::PG_POOL.get().unwrap())
            .is_ok()
    }
}
