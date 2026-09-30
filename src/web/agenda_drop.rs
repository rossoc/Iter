//! The agenda's POST route (`POST /board/{id}/schedule`): what a drop on an
//! hour, or a pick form, does to a task. The form and the transaction are
//! `drop.rs`, shared with the matrix.

use super::Id;
use super::agenda_pick::links;
use super::drop::{Drop, back_or, drop_on};
use crate::models::{Duration, Task, parse_start_time};
use crate::utils::report::parse_day;
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::{Form, Json},
        path_param, route,
    },
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.route(schedule_task)
}

/// Applies an agenda drop. `target` is empty to unschedule, or the
/// `yyyy-mm-dd hh:mm` an hour slot starts at. A task with no duration gets an
/// hour, so it has a length on the agenda.
fn schedule(task: &mut Task, target: &str) -> std::result::Result<(), String> {
    let target = target.trim();
    if target.is_empty() {
        task.start_time = None;
        return Ok(());
    }
    let start =
        parse_start_time(target).ok_or_else(|| format!("Start time '{target}' is not valid."))?;
    task.start_time = Some(start);
    task.duration.get_or_insert(Duration(60));
    Ok(())
}

/// Where a pick form lands after its move: the agenda of the day it named,
/// on the card that moved (the keyboard is back on it), saying so. The day is
/// parsed and the URL rebuilt from it, never taken from the request text.
/// `None` for a post with no (or a bad) day: `board.js`, which reloads itself.
fn schedule_back(id: i64, date: Option<&str>, task: i64) -> Option<String> {
    let day = parse_day(date?).ok()?;
    Some(links(id, day).moved(task))
}

/// `POST /board/{id}/schedule`: a [`Drop`] with the task and the hour (or
/// empty, to unschedule); a pick form also sends the day to land back on.
#[route(POST "/board/{id}/schedule")]
async fn schedule_task(cx: &Cx, Form(drop): Form<Drop>) -> Result<Json<bool>> {
    let id = *path_param::<Id>(cx)?;
    let done = drop_on(id, &drop, |task, target| {
        schedule(task, target).map(|()| None)
    })?;
    back_or(done, schedule_back(id, drop.date.as_deref(), drop.task_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task() -> Task {
        Task::template(1, String::new(), crate::models::TaskStatus::Queue)
    }

    #[test]
    fn a_pick_form_lands_back_on_the_day_and_the_card() {
        assert_eq!(
            schedule_back(2, Some("2026-09-29"), 7).as_deref(),
            Some("/board/2?date=2026-09-29&moved=7#card-7")
        );
        // no day, or a bad one: no redirect (and no text of the request in a URL)
        assert_eq!(schedule_back(2, None, 7), None);
        assert_eq!(schedule_back(2, Some("//evil.example"), 7), None);
    }

    #[test]
    fn dropping_on_an_hour_schedules_with_a_default_hour() {
        let mut t = task();
        schedule(&mut t, "2026-09-28 09:00").expect("valid slot");
        assert_eq!(t.start_time, parse_start_time("2026-09-28 09:00"));
        assert_eq!(t.duration, Some(Duration(60)));
    }

    #[test]
    fn scheduling_keeps_an_existing_duration() {
        let mut t = task();
        t.duration = Some(Duration(90));
        schedule(&mut t, "2026-09-28 09:00").expect("valid slot");
        assert_eq!(t.duration, Some(Duration(90)));
    }

    #[test]
    fn dropping_on_a_list_unschedules() {
        let mut t = task();
        schedule(&mut t, "2026-09-28 09:00").expect("valid slot");
        schedule(&mut t, "").expect("empty target unschedules");
        assert_eq!(t.start_time, None);
        assert!(schedule(&mut t, "later").is_err());
    }
}
