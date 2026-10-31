use chrono::NaiveDateTime;
use iter_macros::Table;

/// One worklog pushed to an organization's time platform: the ledger that
/// lets an identical second push be refused. Not edited by hand, so it
/// carries no serde derives.
#[derive(Debug, Clone, PartialEq, Table)]
#[table(name = "pushes")]
pub struct Push {
    pub id: Option<i64>,
    pub organization_id: i64,
    /// The period covered, `yyyy-mm-dd` (empty `from_day` for an open start).
    pub from_day: String,
    pub to_day: String,
    /// A fingerprint of what was sent: [`crate::push::fingerprint`].
    pub hash: String,
    #[table(with = "datetime")]
    pub at: NaiveDateTime,
    /// What succeeded and what did not, one line per item.
    pub results: String,
}

impl Push {
    /// The items of `results` that succeeded, without their `ok: ` marker.
    pub fn ok_items(&self) -> impl Iterator<Item = &str> {
        self.results.lines().filter_map(|l| l.strip_prefix("ok: "))
    }

    /// Whether every step of this push went through. A push that is not
    /// complete (a day the platform refused, a GitHub issue that could not
    /// be closed) is not "already pushed": running it again finishes it.
    pub fn complete(&self) -> bool {
        !self.results.lines().any(|l| l.starts_with("FAILED: "))
    }
}
