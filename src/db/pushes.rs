//! Queries over the ledger of pushed worklogs.

use super::*;

impl Db {
    /// The push already recorded for `organization_id` with this fingerprint.
    pub fn find_push(&self, organization_id: i64, hash: &str) -> Result<Option<Push>> {
        self.find_one(
            "WHERE organization_id = ?1 AND hash = ?2 ORDER BY id DESC",
            params![organization_id, hash],
        )
    }
}
