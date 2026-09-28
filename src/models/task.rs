use chrono::NaiveDateTime;
use iter_macros::{MarkdownBody, Table};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    #[default]
    Queue,
    Wip,
    Done,
}

impl TaskStatus {
    /// Every status, in workflow order.
    #[cfg(any(test, feature = "web"))]
    pub const ALL: [TaskStatus; 3] = [TaskStatus::Queue, TaskStatus::Wip, TaskStatus::Done];

    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Queue => "queue",
            TaskStatus::Wip => "wip",
            TaskStatus::Done => "done",
        }
    }

    /// The status spelled out, for a report heading where the bare
    /// `wip`/`queue` shorthand next to a task name reads as jargon.
    pub fn label(self) -> &'static str {
        match self {
            TaskStatus::Queue => "queued",
            TaskStatus::Wip => "work in progress",
            TaskStatus::Done => "done",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "queue" => Some(TaskStatus::Queue),
            "wip" => Some(TaskStatus::Wip),
            "done" => Some(TaskStatus::Done),
            _ => None,
        }
    }
}

/// A unit of work under a project. `project_id` is not part of the YAML
/// editor's view -- it's set by the caller from the `--project` flag (on
/// creation) or looked up from the existing row (on edit), never typed by
/// hand -- so it's skipped on both serialize and deserialize.
#[derive(Debug, Clone, Serialize, Deserialize, Table, MarkdownBody)]
#[table(name = "tasks", order_by = "name")]
pub struct Task {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<i64>,

    #[serde(skip)]
    pub project_id: i64,

    pub name: String,

    /// Free-form markdown notes. Edited as the markdown body below the
    /// YAML front matter rather than as a field among the others -- see
    /// `MarkdownBody`.
    #[serde(skip)]
    pub description: String,

    /// Issue number in the project's repo, if this task tracks one.
    #[serde(default)]
    pub github_issue: Option<i64>,

    #[serde(default)]
    pub status: TaskStatus,

    /// What `iter session new` puts in front of this task's slugified name
    /// to get its branch, e.g. `"feat/"`. Pre-filled from the project's
    /// `branch_template` when the task is created, and editable from then
    /// on -- which is the point of it living on the task rather than being
    /// read off the project at session time. Empty means "whatever the
    /// project's template says now", so a task created before this field
    /// existed still branches the project's way.
    #[serde(default)]
    pub branch_prefix: String,

    /// Whether the task has been dropped onto the board's Eisenhower
    /// matrix. Flags alone can't say: a task with neither flag is either
    /// waiting in the matrix's side list or sitting in the gray quadrant.
    #[serde(default)]
    pub matrix_placed: bool,

    /// When the task is scheduled to begin, as `yyyy-mm-dd hh:mm`. Together
    /// with `duration` this places the task on the board's agenda.
    #[serde(default, with = "datetime_opt")]
    pub start_time: Option<NaiveDateTime>,

    /// How long the task is expected to take, as `hh:mm`. The end of the
    /// slot is `start_time + duration` rather than a stored field, so the
    /// two can never disagree.
    #[serde(default)]
    pub duration: Option<Duration>,
}

/// The format `start_time` is typed and shown in. chrono's own serde form is
/// ISO 8601 with a `T`, which is not what anyone edits by hand.
pub const START_TIME_FMT: &str = "%Y-%m-%d %H:%M";

/// Reads a [`START_TIME_FMT`] timestamp -- the one parser for every place a
/// start time is typed (the editor, the web form, a board drop).
pub fn parse_start_time(text: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(text.trim(), START_TIME_FMT).ok()
}

mod datetime_opt {
    use super::{START_TIME_FMT, parse_start_time};
    use chrono::NaiveDateTime;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        value: &Option<NaiveDateTime>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(dt) => serializer.serialize_str(&dt.format(START_TIME_FMT).to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<NaiveDateTime>, D::Error> {
        let raw = Option::<String>::deserialize(deserializer)?;
        match raw.as_deref().map(str::trim) {
            None | Some("") => Ok(None),
            Some(text) => parse_start_time(text).map(Some).ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "invalid start_time `{text}`, expected yyyy-mm-dd hh:mm"
                ))
            }),
        }
    }
}

/// A span of time in whole minutes, written `hh:mm` (hours may exceed 24).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duration(pub i64);

impl Duration {
    pub fn minutes(self) -> i64 {
        self.0
    }

    pub fn parse(text: &str) -> Option<Self> {
        let (hours, minutes) = text.trim().split_once(':')?;
        let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        if !all_digits(hours) || !all_digits(minutes) {
            return None;
        }
        let (hours, minutes): (i64, i64) = (hours.parse().ok()?, minutes.parse().ok()?);
        if minutes >= 60 {
            return None;
        }
        // Checked: `hours` is whatever was typed, and a huge one must be a
        // parse failure rather than an overflow.
        hours.checked_mul(60)?.checked_add(minutes).map(Duration)
    }
}

impl std::fmt::Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}:{:02}", self.0 / 60, self.0 % 60)
    }
}

impl Serialize for Duration {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Duration {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Duration::parse(&text).ok_or_else(|| {
            serde::de::Error::custom(format!("invalid duration `{text}`, expected hh:mm"))
        })
    }
}

/// What `iter task new`/`edit` open in the editor: the task plus the names
/// of its tags -- which is also how a task is marked urgent or important
/// (see [`crate::models::Priority`]). Tags are rows of their own, so they
/// ride alongside the task rather than on it.
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskEdit {
    #[serde(flatten)]
    pub task: Task,

    /// Tag names; each must already exist (`iter tag new`).
    #[serde(default)]
    pub tags: Vec<String>,
}

impl crate::models::MarkdownBody for TaskEdit {
    fn description(&self) -> &str {
        &self.task.description
    }

    fn set_description(&mut self, description: String) {
        self.task.description = description;
    }

    fn carry_over(&mut self, original: &Self) {
        self.task.carry_over(&original.task);
    }

    fn from_front_matter(front: &str) -> serde_yaml::Result<Self> {
        let (task, tags) = crate::models::read_wrapped(front, "tags")?;
        Ok(TaskEdit { task, tags })
    }
}

impl Task {
    /// When the scheduled slot ends: `start_time + duration`, if both are set.
    pub fn end_time(&self) -> Option<NaiveDateTime> {
        Some(self.start_time? + chrono::Duration::minutes(self.duration?.minutes()))
    }

    /// `start_time` as typed and shown, or empty when unscheduled.
    #[cfg(feature = "web")]
    pub fn start_text(&self) -> String {
        self.start_time
            .map(|t| t.format(START_TIME_FMT).to_string())
            .unwrap_or_default()
    }

    /// `duration` as `hh:mm`, or empty when there is none.
    #[cfg(feature = "web")]
    pub fn duration_text(&self) -> String {
        self.duration.map(|d| d.to_string()).unwrap_or_default()
    }

    /// A blank (or issue-prefilled) template for `iter task new` to open in
    /// the YAML editor. `branch_prefix` comes from the owning project's
    /// `branch_template`, so the default is the project's rule and the
    /// editor is where it gets overridden; `status` comes from the config
    /// file, for anyone who tracks work that starts already in progress.
    pub fn template(project_id: i64, branch_prefix: String, status: TaskStatus) -> Self {
        Task {
            id: None,
            project_id,
            name: String::new(),
            description: String::new(),
            github_issue: None,
            status,
            branch_prefix,
            matrix_placed: false,
            start_time: None,
            duration: None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_slot_ends_after_its_duration_and_only_when_both_are_set() {
        let mut t = Task::template(1, String::new(), TaskStatus::Queue);
        assert_eq!(t.end_time(), None);
        t.start_time = parse_start_time(" 2026-09-28 23:30 ");
        assert_eq!(t.end_time(), None);
        t.duration = Some(Duration(90));
        assert_eq!(t.end_time(), parse_start_time("2026-09-29 01:00"));
        #[cfg(feature = "web")]
        assert_eq!(
            (t.start_text(), t.duration_text()),
            ("2026-09-28 23:30".to_string(), "01:30".to_string())
        );
        assert_eq!(parse_start_time("tomorrow"), None);
    }

    use super::*;

    /// `branch_prefix` has to reach the YAML editor as an ordinary field --
    /// that's the whole point of it living on the task -- unlike
    /// `project_id`/`description`, which are deliberately kept out of it.
    #[test]
    fn a_new_task_carries_its_branch_prefix_into_the_editor() {
        let template = Task::template(3, "hotfix/".to_string(), TaskStatus::Queue);
        let yaml = serde_yaml::to_string(&template).expect("a task serializes");
        assert!(
            yaml.contains("branch_prefix: hotfix/"),
            "branch_prefix missing from the editor template:\n{yaml}"
        );
        assert!(!yaml.contains("project_id"));
    }

    #[test]
    fn an_edited_prefix_parses_back_out() {
        let task: Task = serde_yaml::from_str("name: t\nbranch_prefix: chore/\n")
            .expect("front matter parses back into a task");
        assert_eq!(task.branch_prefix, "chore/");
    }

    /// Front matter written before the field existed still loads, blank --
    /// which `iter session new` reads as "use the project's template".
    #[test]
    fn a_missing_prefix_defaults_to_blank() {
        let task: Task = serde_yaml::from_str("name: t\n").expect("front matter parses");
        assert_eq!(task.branch_prefix, "");
    }

    #[test]
    fn scheduling_fields_default_when_absent() {
        let task: Task = serde_yaml::from_str("name: t\n").expect("front matter parses");
        assert_eq!(task.start_time, None);
        assert_eq!(task.duration, None);
    }

    #[test]
    fn scheduling_fields_round_trip_in_the_editor_format() {
        let task: Task =
            serde_yaml::from_str("name: t\nstart_time: 2026-09-28 09:30\nduration: 01:45\n")
                .expect("front matter parses");
        assert_eq!(task.duration, Some(Duration(105)));
        let yaml = serde_yaml::to_string(&task).expect("a task serializes");
        assert!(yaml.contains("2026-09-28 09:30"), "{yaml}");
        assert!(yaml.contains("01:45"), "{yaml}");
        let back: Task = serde_yaml::from_str(&yaml).expect("round trip parses");
        assert_eq!(back.start_time, task.start_time);
        assert_eq!(back.duration, task.duration);
    }

    #[test]
    fn a_blank_schedule_serializes_as_null_and_parses_back() {
        let template = Task::template(1, String::new(), TaskStatus::Queue);
        let yaml = serde_yaml::to_string(&template).expect("a task serializes");
        assert!(yaml.contains("start_time: null"), "{yaml}");
        let back: Task = serde_yaml::from_str(&yaml).expect("parses");
        assert_eq!((back.start_time, back.duration), (None, None));
    }

    /// Hours are whatever was typed: one too big for the minutes to fit
    /// is a parse failure, not an overflow.
    #[test]
    fn a_duration_too_long_to_count_is_rejected() {
        assert_eq!(Duration::parse("999999999999999999:00"), None);
        assert_eq!(Duration::parse("1000:30"), Some(Duration(60_030)));
    }

    #[test]
    fn malformed_schedule_values_are_rejected() {
        assert!(serde_yaml::from_str::<Task>("name: t\nstart_time: tomorrow\n").is_err());
        assert!(serde_yaml::from_str::<Task>("name: t\nduration: 1:75\n").is_err());
        assert!(serde_yaml::from_str::<Task>("name: t\nduration: soon\n").is_err());
    }
}
