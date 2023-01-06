use std::sync::Arc;

use serenity::prelude::TypeMapKey;
use uuid::Uuid;
use moka::dash::Cache as DashCache;

use super::{meeting::Meeting, member::Member, report::Report, summary::Summary};

impl TypeMapKey for Cache {
    type Value = Arc<Cache>;
}

pub struct Cache {
    pub meetings: DashCache<Uuid, Meeting>,
    pub members: DashCache<Uuid, Member>,
    pub reports: DashCache<Uuid, Report>,
    pub summaries: DashCache<Uuid, Summary>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            meetings: DashCache::new(100),
            members: DashCache::new(100),
            reports: DashCache::new(100),
            summaries: DashCache::new(100),
        }
    }
}
