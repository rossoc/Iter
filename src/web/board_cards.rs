//! Maps [`Card`]s and the board's priority colors to `ui::task_card::CardView` (`ui/` takes no models).

use super::ui::task_card::{CardAction, CardView, Edge};
use super::url::task_url;
use crate::db::{Db, Table};
use crate::error::Result;
use crate::models::{Card, TaskStatus, is_hex_color};

pub const BOARD_EMPTY: &str =
    "No unfinished tasks yet. Add some to the board's projects, and they show up here.";

pub struct Colors {
    urgent: String,
    important: String,
}

impl Colors {
    pub fn load(db: &Db) -> Result<Colors> {
        let (urgent, important) = db.priority_colors()?;
        Ok(Colors { urgent, important })
    }

    /// Custom properties for the cards' edges; a color that is not `#rrggbb` is left out.
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

pub const CLOCK_FMT: &str = "%H:%M";

#[derive(Clone, Copy)]
pub struct CardCtx {
    pub picked: Option<i64>,
}

impl CardCtx {
    pub fn view<'a>(&self, card: &'a Card, label: &'static str, href: String) -> CardView<'a> {
        let Card {
            label: title,
            task,
            priority,
        } = card;
        CardView {
            id: task.id(),
            href: task_url(task.id()),
            title,
            time: String::new(),
            edge: Edge::of(*priority),
            status: (task.status == TaskStatus::Wip).then_some(TaskStatus::Wip),
            action: Some(CardAction { label, href }),
            picked: self.picked == Some(task.id()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Duration, Priority, Task, parse_start_time};

    fn task(start: Option<&str>, minutes: Option<i64>) -> Task {
        Task {
            id: Some(1),
            start_time: start.and_then(parse_start_time),
            duration: minutes.map(Duration),
            ..crate::fixtures::task(1, "")
        }
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
    fn a_card_has_its_edge_shows_no_time_and_only_wip() {
        let ctx = CardCtx { picked: Some(1) };
        let card = Card {
            label: "p/t".into(),
            task: task(Some("2026-09-29 09:00"), Some(30)),
            priority: Priority {
                urgent: true,
                important: true,
            },
        };
        let view = ctx.view(&card, "Move", "/x".into());
        assert_eq!(view.edge, Edge::Both);
        assert_eq!(view.time, "", "a card says no time: its hour row does");
        assert_eq!(view.status, None);
        assert!(view.picked);
    }
}
