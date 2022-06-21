use crate::database::schema::report;
use crate::diesel::RunQueryDsl;
use chrono::NaiveDate;
use uuid::Uuid;

#[derive(Queryable, Debug)]
struct Report {
    id: Uuid,
    member_uuid: Uuid,
    content: String,
    create_date: NaiveDate,
}

#[derive(Insertable, Debug)]
#[table_name = "report"]
struct NewReport {
    member_uuid: Uuid,
    content: String,
}

fn create_report(member_uuid: Uuid, content: String) -> Report {
    let new_report = NewReport {
        member_uuid,
        content,
    };

    diesel::insert_into(report::table)
        .values(&new_report)
        .get_result(&crate::database::PG_POOL.get().unwrap())
        .expect("Error creating new report")
}
