//! What a board's cards show: the mapping from [`Card`]s and the board's
//! priority colors to `ui::task_card::CardView`, shared by the agenda and
//! the matrix. `ui/` takes no models, so it happens here.

use super::ui::tag_chips::Chip;
use super::ui::task_card::{CardAction, CardView, Edge};
use super::url::task_url;
use crate::db::{Db, Table};
use crate::error::Result;
use crate::models::{Card, IMPORTANT_TAG, Task, TaskStatus, URGENT_TAG, end_text, is_hex_color};

/// The colors of the Urgent and Important tags: what the board draws with.
/// Both tags are seeded and can't be deleted.
pub struct Colors {
    urgent: String,
    important: String,
}

impl Colors {
    /// Both colors in one query.
    pub fn load(db: &Db) -> Result<Colors> {
        let (urgent, important) = db.priority_colors()?;
        Ok(Colors { urgent, important })
    }

    /// The custom properties the cards' edges use (`--urgent`,
    /// `--important`); a color that is not a `#rrggbb` is left out (the
    /// edge then falls back to a neutral one).
    pub fn style(&self) -> String {
        let mut style = String::new();
        for (name, color) in [("urgent", &self.urgent), ("important", &self.important)] {
            // the database accepts any text as a color, and it ends up in a
            // `style` attribute
            if is_hex_color(color) {
                if !style.is_empty() {
                    style.push(';');
                }
                style.push_str(&format!("--{name}:{color}"));
            }
        }
        style
    }
}

/// The clock format of the cards and the now line.
pub const CLOCK_FMT: &str = "%H:%M";

/// `1h 30m`, `45m`, `2h`; empty for no time. This is the cards' short form;
/// the forms and the CLI write a duration as `hh:mm` (`Duration`'s Display).
fn duration_label(minutes: i64) -> String {
    match (minutes / 60, minutes % 60) {
        _ if minutes <= 0 => String::new(),
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

/// When the task is: `09:00-10:00` (an end on another day is written in
/// full), `09:00` with no duration, its duration when it has no start, or
/// nothing.
pub fn time_text(task: &Task) -> String {
    match (task.start_time, task.end_time()) {
        (Some(start), Some(end)) => {
            format!(
                "{}\u{2013}{}",
                start.format(CLOCK_FMT),
                end_text(start, end)
            )
        }
        (Some(start), None) => start.format(CLOCK_FMT).to_string(),
        (None, _) => task
            .duration
            .map(|d| duration_label(d.minutes()))
            .unwrap_or_default(),
    }
}

/// Which of a card's flag chips are shown: a list that says what a flag is
/// (the agenda's Important list, a quadrant) leaves that chip off.
#[derive(Clone, Copy)]
pub enum Chips {
    All,
    /// Every chip but the one of this tag.
    Except(&'static str),
    None,
}

impl Chips {
    fn shows(self, name: &str) -> bool {
        match self {
            Chips::All => true,
            Chips::Except(hidden) => hidden != name,
            Chips::None => false,
        }
    }
}

/// What every card of a board page shares: the colors, and the task being
/// moved (the pick mode), which its card marks.
pub struct CardCtx<'a> {
    pub colors: &'a Colors,
    pub picked: Option<i64>,
}

impl<'a> CardCtx<'a> {
    /// The card with all its chips; `label` and `href` are its own link (see
    /// `CardAction`).
    pub fn view(&self, card: &'a Card, label: &'static str, href: String) -> CardView<'a> {
        self.view_chips(card, label, href, Chips::All)
    }

    /// The same, showing only the `chips` asked for (none are built for the
    /// others).
    pub fn view_chips(
        &self,
        card: &'a Card,
        label: &'static str,
        href: String,
        chips: Chips,
    ) -> CardView<'a> {
        let action = CardAction { label, href };
        card_view(
            card,
            self.colors,
            Some(action),
            self.picked == Some(card.task.id()),
            chips,
        )
    }
}

/// How `card` is drawn. `action` is the card's own link, `picked` marks the
/// card being moved.
pub fn card_view<'a>(
    card: &'a Card,
    colors: &'a Colors,
    action: Option<CardAction<'a>>,
    picked: bool,
    chips: Chips,
) -> CardView<'a> {
    let Card {
        label,
        task,
        priority,
    } = card;
    let mut flags = Vec::with_capacity(2);
    if priority.urgent && chips.shows(URGENT_TAG) {
        flags.push(Chip {
            name: URGENT_TAG,
            color: &colors.urgent,
        });
    }
    if priority.important && chips.shows(IMPORTANT_TAG) {
        flags.push(Chip {
            name: IMPORTANT_TAG,
            color: &colors.important,
        });
    }
    CardView {
        id: task.id(),
        href: task_url(task.id()),
        title: label,
        time: time_text(task),
        edge: Edge::of(*priority),
        flags,
        status: (task.status == TaskStatus::Wip).then_some(TaskStatus::Wip),
        action,
        picked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Duration, Priority, parse_start_time};

    fn task(start: Option<&str>, minutes: Option<i64>) -> Task {
        let mut t = Task::template(1, String::new(), TaskStatus::Queue);
        t.id = Some(1);
        t.start_time = start.and_then(parse_start_time);
        t.duration = minutes.map(Duration);
        t
    }

    #[test]
    fn a_scheduled_task_shows_its_slot() {
        let t = task(Some("2026-09-29 09:00"), Some(90));
        assert_eq!(time_text(&t), "09:00\u{2013}10:30");
        let over = task(Some("2026-09-29 23:30"), Some(60));
        assert_eq!(time_text(&over), "23:30\u{2013}2026-09-30 00:30");
        assert_eq!(time_text(&task(Some("2026-09-29 09:00"), None)), "09:00");
    }

    #[test]
    fn an_unscheduled_task_shows_its_duration() {
        assert_eq!(time_text(&task(None, Some(90))), "1h 30m");
        assert_eq!(time_text(&task(None, Some(45))), "45m");
        assert_eq!(time_text(&task(None, Some(120))), "2h");
        assert_eq!(time_text(&task(None, Some(0))), "");
        assert_eq!(time_text(&task(None, None)), "");
    }

    #[test]
    fn only_a_hex_color_reaches_the_style() {
        let colors = |urgent: &str, important: &str| Colors {
            urgent: urgent.into(),
            important: important.into(),
        };
        assert_eq!(
            colors("#facc15", "#3b82f6").style(),
            "--urgent:#facc15;--important:#3b82f6"
        );
        assert_eq!(colors("red;x:y", "#3b82f6").style(), "--important:#3b82f6");
        assert_eq!(colors("", "url(x)").style(), "");
    }

    #[test]
    fn flags_become_chips_and_an_edge() {
        let colors = Colors {
            urgent: "#facc15".into(),
            important: "#3b82f6".into(),
        };
        let card = Card {
            label: "p/t".into(),
            task: task(None, None),
            priority: Priority {
                urgent: true,
                important: true,
            },
        };
        let view = card_view(&card, &colors, None, false, Chips::All);
        assert_eq!(view.edge, Edge::Both);
        assert_eq!(
            view.flags.iter().map(|c| c.name).collect::<Vec<_>>(),
            ["Urgent", "Important"]
        );
        assert_eq!(view.status, None);
        let names = |chips| {
            card_view(&card, &colors, None, false, chips)
                .flags
                .iter()
                .map(|c| c.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(Chips::Except(IMPORTANT_TAG)), ["Urgent"]);
        assert!(names(Chips::None).is_empty());
    }
}
