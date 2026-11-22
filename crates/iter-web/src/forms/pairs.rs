//! The pairs a form posts, read by name: the first value, whether a box is ticked, the ids of a checklist.

/// The submitted `(name, value)` pairs of a form, read by name.
pub(crate) struct Pairs<'a>(pub(super) &'a [(String, String)]);

impl Pairs<'_> {
    pub(crate) fn first(&self, key: &str) -> String {
        self.get(key).unwrap_or_default().to_string()
    }

    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub(crate) fn ids(&self, key: &str) -> Vec<i64> {
        let mut ids: Vec<i64> = self
            .0
            .iter()
            .filter(|(k, _)| k == key)
            .filter_map(|(_, v)| v.parse().ok())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}
