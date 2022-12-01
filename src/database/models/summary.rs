use std::fmt::Display;
use std::fmt::Formatter;

use anyhow::Result;
use chrono::NaiveDate;
use diesel::pg::Pg;
use diesel::query_dsl::SaveChangesDsl;
use uuid::Uuid;

use crate::database::pagination::Paginate;
use crate::database::pagination::Paginated;
use crate::database::schema::summary;
use crate::database::schema::summary::BoxedQuery;
use crate::database::PG_POOL;
use crate::diesel::ExpressionMethods;
use crate::diesel::QueryDsl;
use crate::diesel::RunQueryDsl;
use crate::framework::summary::Filter;

type AllColumns = (
    summary::id,
    summary::note,
    summary::create_date,
    summary::messages_id,
);

const ALL_COLUMNS: AllColumns = (
    summary::id,
    summary::note,
    summary::create_date,
    summary::messages_id,
);

type All = diesel::dsl::Select<crate::database::schema::summary::table, AllColumns>;

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Debug)]
#[diesel(table_name = summary)]
pub(crate) struct Summary {
    id: Uuid,
    note: String,
    create_date: NaiveDate,
    pub(crate) messages_id: Option<Vec<String>>,
}

impl Summary {
    pub fn new(content: String, create_date: NaiveDate) -> Summary {
        Summary {
            id: Uuid::new_v4(),
            note: content,
            create_date,
            messages_id: None,
        }
    }

    pub fn all() -> All {
        summary::table.select(ALL_COLUMNS)
    }

    pub fn insert(&self) -> Result<Self> {
        Ok(diesel::insert_into(summary::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self> {
        Ok(self.save_changes(&mut PG_POOL.get()?)?)
    }

    pub fn delete(&self) -> Result<bool> {
        use crate::database::schema::summary::dsl::*;

        Ok(diesel::delete(summary.filter(id.eq(self.id)))
            .execute(&mut PG_POOL.get()?)
            .map(|rows| rows != 0)?)
    }

    pub fn list(filter: Filter, page: i64, page_size: Option<i64>) -> Result<(Vec<Self>, i64)> {
        let query = filter.apply(Summary::all().into_boxed());

        let query = Self::paginate(query, page, page_size);

        let (summaries, total) = query.load_and_count_pages(&mut PG_POOL.get().unwrap()).unwrap();

        Ok((summaries, total))
    }

    pub fn paginate(
        query: BoxedQuery<'_, Pg>,
        page: i64,
        per_page: Option<i64>,
    ) -> Paginated<BoxedQuery<'_, Pg>> {
        let mut query = query.paginate(page);

        if let Some(per_page) = per_page {
            query = query.per_page(per_page);
        }

        query
    }

    pub(crate) fn id(&self) -> Uuid {
        self.id
    }

    pub(crate) fn find_by_id(summary_id: Uuid) -> Result<Self> {
        use crate::database::schema::summary::dsl::*;

        Ok(summary
            .filter(id.eq(summary_id))
            .first(&mut PG_POOL.get()?)?)
    }

    /// Set content. Returns the updated summary.
    pub(crate) fn set_note(&mut self, note: String) -> Result<Self> {
        self.note = note;

        self.update()
    }

    pub(crate) fn note(&self) -> &str {
        &self.note
    }

    pub(crate) fn create_date(&self) -> NaiveDate {
        self.create_date
    }
}

impl Display for Summary {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Summary {{ id: {}, create_date: {}, messages_id: {:?} }}",
            self.id, self.create_date, self.messages_id
        )
    }
}

impl From<crate::framework::summary::Summary> for Summary {
    fn from(summary: crate::framework::summary::Summary) -> Self {
        let messages = summary.messages_id;
        let messages = if messages.is_empty() {
            None
        } else {
            Some(messages.into_iter().map(|msg| msg.to_string()).collect())
        };
        Summary {
            id: summary.id,
            note: summary.note,
            create_date: summary.create_date,
            messages_id: messages,
        }
    }
}
