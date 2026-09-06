use crate::sessions::{SessionRecord, minutes_to_hhmm};

pub trait SessionLog {
    /// Inserts a new record: a freshly begun session with no `end` yet.
    fn insert(&self, record: &SessionRecord);

    /// Closes the open session matching `record.name` (and, for a
    /// storage backend that needs it to locate the row, `record.begin`),
    /// setting its `end` and `message` from `record`.
    fn update(&self, record: &SessionRecord);

    /// Selects every session currently in the log.
    fn select(&self) -> Vec<SessionRecord>;

    fn begin(&self, value: &str) {
        let record = SessionRecord {
            name: value.to_string(),
            begin: chrono::Local::now().naive_local(),
            end: None,
            message: None,
        };
        self.insert(&record);
    }

    fn end_with_message(&self, value: &str, message: Option<&str>) {
        let now = chrono::Local::now().naive_local();
        // Editing the last session means finding it first: the open record
        // this `end` closes, so `update` gets a full record (begin included)
        // rather than just the fields this call happens to know.
        if let Some(open) = self
            .select()
            .into_iter()
            .find(|s| s.is_ongoing() && s.name == value)
        {
            self.update(&SessionRecord {
                name: open.name,
                begin: open.begin,
                end: Some(now),
                message: message.map(|m| m.to_string()),
            });
        }
        // else: no open session by that name — nothing to update.
    }

    fn elapsed(&self, value: &str) {
        let now = chrono::Local::now().naive_local();
        let sessions = self.select();

        match sessions.iter().find(|s| s.is_ongoing() && s.name == value) {
            Some(session) => println!("{}", minutes_to_hhmm(session.duration_minutes(now))),
            _ => println!("No session running"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::csv_log::parse_sessions;
    // elapsed() itself only prints, so these exercise the same
    // ongoing-and-name-matches selection it runs, directly against
    // parse_sessions — mirroring how sessions.rs tests its own logic.
    #[test]
    fn does_not_match_on_substring_of_the_name() {
        // A name "A" must not match an open session named "AB".
        let sessions = parse_sessions("AB,2026-09-04 09:00,,\n");
        assert!(
            sessions
                .iter()
                .find(|s| s.is_ongoing() && s.name == "A")
                .is_none()
        );
        assert!(
            sessions
                .iter()
                .find(|s| s.is_ongoing() && s.name == "AB")
                .is_some()
        );
    }

    #[test]
    fn ignores_a_session_that_has_already_ended() {
        let sessions = parse_sessions("A,2026-09-01 09:00,2026-09-01 10:00,\n");
        assert!(
            sessions
                .iter()
                .find(|s| s.is_ongoing() && s.name == "A")
                .is_none()
        );
    }

    #[test]
    fn tracks_two_open_sessions_independently() {
        let sessions = parse_sessions("A,2026-09-04 09:00,,\nB,2026-09-04 09:05,,\n");
        let a = sessions.iter().find(|s| s.is_ongoing() && s.name == "A");
        let b = sessions.iter().find(|s| s.is_ongoing() && s.name == "B");
        assert!(a.is_some());
        assert!(b.is_some());
        assert_ne!(a.unwrap().begin, b.unwrap().begin);
    }
}
