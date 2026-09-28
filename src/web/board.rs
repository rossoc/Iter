//! The board pages: the list of boards, a board's day agenda and its
//! Eisenhower matrix. Both boards are drag-and-drop: `board.js` posts the
//! dropped task and where it landed to the two `POST` routes below, and
//! reloads.

use super::layout::{Nav, Sel, shell};
use super::open_db;
use crate::db::{Db, Table};
use crate::error::IterError;
use crate::models::{
    Board, Duration, IMPORTANT_TAG, Project, START_TIME_FMT, Tag, Task, TaskStatus, URGENT_TAG,
};
use chrono::{Local, NaiveDate, NaiveDateTime};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::{Form, Json},
        error::{RouterErrorExt, bad_request, see_other},
        page, path_param, query_params, route,
    },
    view::{View, component, view},
};

path_param!(id: i64, error = bad_request);

#[query_params(error = bad_request)]
struct DayQuery {
    date: Option<String>,
}

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder
        .page(boards_page)
        .page(agenda_page)
        .page(matrix_page)
        .page(info_page)
        .page(edit_page)
        .page(save_page)
        .route(schedule_task)
        .route(place_task)
}

// ---- data -------------------------------------------------------------------

/// A task as a board shows it: a card carrying what it needs to be drawn,
/// sorted and dropped.
#[derive(Clone)]
struct Card {
    id: i64,
    /// `<project>/<task>`, the same way the CLI names it.
    label: String,
    task: Task,
}

impl Card {
    /// The colour class: `ui` (both flags), `u`, `i`, or none.
    fn class(&self) -> &'static str {
        match (self.task.urgency, self.task.importance) {
            (true, true) => "card ui",
            (true, false) => "card u",
            (false, true) => "card i",
            (false, false) => "card",
        }
    }

    fn slot(&self) -> String {
        let Some(start) = self.task.start_time else {
            return String::new();
        };
        let mut slot = start.format("%H:%M").to_string();
        if let Some(d) = self.task.duration {
            let end = start + chrono::Duration::minutes(d.minutes());
            slot.push_str(&format!("-{}", end.format("%H:%M")));
        }
        slot
    }
}

/// The colours a board draws with: the Urgent and Important tags' colours,
/// as CSS variables.
fn colors_style(db: &Db) -> crate::error::Result<String> {
    let color = |name: &str, fallback: &str| -> crate::error::Result<String> {
        Ok(db
            .find_by_name::<Tag>(name)?
            .map_or_else(|| fallback.to_string(), |t| t.color))
    };
    Ok(format!(
        "--urgent:{};--important:{}",
        color(URGENT_TAG, "#facc15")?,
        color(IMPORTANT_TAG, "#3b82f6")?
    ))
}

/// Every unfinished task on the board's projects, urgent first, then by name.
fn cards(db: &Db, board_id: i64) -> crate::error::Result<Vec<Card>> {
    let mut out = Vec::new();
    for project in db.projects_for_board(board_id)? {
        for task in db.tasks_for_project(project.id())? {
            if task.status == TaskStatus::Done {
                continue;
            }
            out.push(Card {
                id: task.id(),
                label: crate::models::task_ref(&project.name, &task.name),
                task,
            });
        }
    }
    out.sort_by(|a, b| {
        b.task
            .urgency
            .cmp(&a.task.urgency)
            .then_with(|| a.label.cmp(&b.label))
    });
    Ok(out)
}

fn pick(all: &[Card], keep: impl Fn(&Card) -> bool) -> Vec<Card> {
    all.iter().filter(|c| keep(c)).cloned().collect()
}

fn day_of(cx: &Cx) -> Result<NaiveDate> {
    let today = Local::now().date_naive();
    Ok(match query_params::<DayQuery>(cx)?.date.as_deref() {
        Some(text) => NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap_or(today),
        None => today,
    })
}

// ---- moves ------------------------------------------------------------------

/// Applies an agenda drop. `target` is empty to unschedule, or the
/// `yyyy-mm-dd hh:mm` an hour slot starts at. A task with no duration gets an
/// hour, so it has a length on the agenda.
fn schedule(task: &mut Task, target: &str) -> std::result::Result<(), String> {
    let target = target.trim();
    if target.is_empty() {
        task.start_time = None;
        return Ok(());
    }
    let start = NaiveDateTime::parse_from_str(target, START_TIME_FMT)
        .map_err(|_| format!("invalid start time '{target}'"))?;
    task.start_time = Some(start);
    task.duration.get_or_insert(Duration(60));
    Ok(())
}

/// Applies a matrix drop. `target` is `left` (back to the side list, flags
/// untouched) or a quadrant, which sets the flags to match.
fn place(task: &mut Task, target: &str) -> std::result::Result<(), String> {
    let (urgent, important) = match target {
        "left" => {
            task.matrix_placed = false;
            return Ok(());
        }
        "both" => (true, true),
        "important" => (false, true),
        "urgent" => (true, false),
        "neither" => (false, false),
        other => return Err(format!("unknown quadrant '{other}'")),
    };
    task.urgency = urgent;
    task.importance = important;
    task.matrix_placed = true;
    Ok(())
}

#[derive(Deserialize)]
struct Drop {
    task_id: i64,
    target: String,
}

/// Loads the dropped task, checks it is one of this board's, and stores what
/// `apply` makes of it.
fn drop_on(
    board_id: i64,
    drop: &Drop,
    apply: fn(&mut Task, &str) -> std::result::Result<(), String>,
) -> Result<Json<bool>> {
    let db = open_db()?;
    let mut found = cards(&db, board_id)?
        .into_iter()
        .find(|c| c.id == drop.task_id)
        .ok_or_not_found()?;
    apply(&mut found.task, &drop.target).map_err(bad_request)?;
    db.update(found.id, &found.task)?;
    Ok(Json(true))
}

#[route(POST "/board/{id}/schedule")]
async fn schedule_task(cx: &Cx, Form(drop): Form<Drop>) -> Result<Json<bool>> {
    drop_on(*path_param::<Id>(cx)?, &drop, schedule)
}

#[route(POST "/board/{id}/matrix")]
async fn place_task(cx: &Cx, Form(drop): Form<Drop>) -> Result<Json<bool>> {
    drop_on(*path_param::<Id>(cx)?, &drop, place)
}

// ---- views ------------------------------------------------------------------

#[component]
async fn card(c: &Card) -> Result<impl View> {
    Ok(view! {
        <div class=(c.class()) draggable="true" data-task=(c.id.to_string())>
            <a href=(format!("/task/{}", c.id))>(c.label.clone())</a>
            <span class="slot">(c.slot())</span>
        </div>
    })
}

/// A place cards can be dropped: `target` is what `board.js` posts.
#[component]
async fn zone(target: String, class: &str, title: &str, cards: &[Card]) -> Result<impl View> {
    Ok(view! {
        <section class=(format!("zone {class}")) data-target=(target.clone())>
            <h3>(title.to_string())</h3>
            for c in cards.iter() {
                card(c: c)
            }
        </section>
    })
}

#[component]
async fn board_tabs(id: i64, active: &str, date: &str) -> Result<impl View> {
    let agenda = if active == "agenda" { "sel" } else { "" };
    let matrix = if active == "matrix" { "sel" } else { "" };
    let info = if active == "info" { "sel" } else { "" };
    Ok(view! {
        <div class="tabs">
            <a class=(agenda) href=(format!("/board/{id}?date={date}"))>"Agenda"</a>
            <a class=(matrix) href=(format!("/board/{id}/matrix"))>"Eisenhower matrix"</a>
            <a class=(info) href=(format!("/board/{id}/info"))>"Info"</a>
            <a class="edit" href=(format!("/board/{id}/edit"))>"Edit"</a>
        </div>
    })
}

#[page("/boards")]
async fn boards_page() -> Result<impl View> {
    let (nav, boards) = {
        let db = open_db()?;
        (Nav::load(&db)?, db.list::<Board>()?)
    };
    Ok(view! {
        shell(
            nav: &nav,
            sel: Sel::Board,
            <h1>"Boards"</h1>
            if boards.is_empty() {
                <p class="empty">"No boards. Create one with `iter board new`."</p>
            } else {
                <ul class="boards">
                    for b in boards.iter() {
                        <li><a href=(format!("/board/{}", b.id()))>(b.name.clone())</a></li>
                    }
                </ul>
            }
        )
    })
}

#[page("/board/{id}")]
async fn agenda_page(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let day = day_of(cx)?;
    let (nav, board, style, all) = {
        let db = open_db()?;
        let board = db.get::<Board>(id)?.ok_or_not_found()?;
        (Nav::load(&db)?, board, colors_style(&db)?, cards(&db, id)?)
    };
    let important = pick(&all, |c| c.task.start_time.is_none() && c.task.importance);
    let other = pick(&all, |c| c.task.start_time.is_none() && !c.task.importance);
    let hours: Vec<(String, String, Vec<Card>)> = (0..24)
        .map(|h| {
            let slot = day.and_hms_opt(h, 0, 0).expect("hour is in range");
            let here = all
                .iter()
                .filter(|c| c.task.start_time.is_some_and(|s| s >= slot && s < slot + chrono::Duration::hours(1)))
                .cloned()
                .collect();
            (format!("{h:02}:00"), slot.format(START_TIME_FMT).to_string(), here)
        })
        .collect();
    let date = day.format("%Y-%m-%d").to_string();
    let (prev, next) = (day.pred_opt().unwrap_or(day), day.succ_opt().unwrap_or(day));
    Ok(view! {
        shell(
            nav: &nav,
            sel: Sel::Board,
            <div class="crumbs"><a href="/boards">"Boards"</a></div>
            <h1>(board.name.clone())</h1>
            board_tabs(id: id, active: "agenda", date: &date)
            <div class="board" style=(style.clone()) data-post=(format!("/board/{id}/schedule"))>
                <div class="lists">
                    zone(target: String::new(), class: "", title: "Important, not scheduled", cards: &important)
                    zone(target: String::new(), class: "", title: "Not important, not scheduled", cards: &other)
                </div>
                <div class="agenda">
                    <div class="days">
                        <a href=(format!("/board/{id}?date={}", prev.format("%Y-%m-%d")))>"<"</a>
                        <strong>(day.format("%A %-d %B %Y").to_string())</strong>
                        <a href=(format!("/board/{id}?date={}", next.format("%Y-%m-%d")))>">"</a>
                    </div>
                    for (label, target, here) in hours.iter() {
                        <div class="hour">
                            <span class="label">(label.clone())</span>
                            zone(target: target.clone(), class: "slotzone", title: "", cards: here)
                        </div>
                    }
                </div>
            </div>
            <script src="/board.js"></script>
        )
    })
}

#[page("/board/{id}/matrix")]
async fn matrix_page(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (nav, board, style, all) = {
        let db = open_db()?;
        let board = db.get::<Board>(id)?.ok_or_not_found()?;
        (Nav::load(&db)?, board, colors_style(&db)?, cards(&db, id)?)
    };
    let waiting = pick(&all, |c| !c.task.matrix_placed);
    let both = pick(&all, |c| c.task.matrix_placed && c.task.urgency && c.task.importance);
    let important = pick(&all, |c| c.task.matrix_placed && !c.task.urgency && c.task.importance);
    let urgent = pick(&all, |c| c.task.matrix_placed && c.task.urgency && !c.task.importance);
    let neither = pick(&all, |c| c.task.matrix_placed && !c.task.urgency && !c.task.importance);
    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
    Ok(view! {
        shell(
            nav: &nav,
            sel: Sel::Board,
            <div class="crumbs"><a href="/boards">"Boards"</a></div>
            <h1>(board.name.clone())</h1>
            board_tabs(id: id, active: "matrix", date: &today)
            <div class="board" style=(style.clone()) data-post=(format!("/board/{id}/matrix"))>
                <div class="lists">
                    zone(target: "left".to_string(), class: "", title: "Not placed", cards: &waiting)
                </div>
                <div class="matrix">
                    <div class="axis">"Urgent"</div>
                    <div class="axis">"Not urgent"</div>
                    zone(target: "both".to_string(), class: "q-both", title: "Important and urgent", cards: &both)
                    zone(target: "important".to_string(), class: "q-important", title: "Important, not urgent", cards: &important)
                    zone(target: "urgent".to_string(), class: "q-urgent", title: "Urgent, not important", cards: &urgent)
                    zone(target: "neither".to_string(), class: "q-neither", title: "Not urgent, not important", cards: &neither)
                </div>
            </div>
            <script src="/board.js"></script>
        )
    })
}

// ---- info and edit ----------------------------------------------------------

/// What the edit form submits. The projects come as repeated `project`
/// fields, one per ticked box, which a struct can't hold, so the raw pairs
/// are read instead.
struct BoardForm {
    name: String,
    description: String,
    projects: Vec<i64>,
}

impl BoardForm {
    fn parse(pairs: &[(String, String)]) -> BoardForm {
        let first = |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        BoardForm {
            name: first("name"),
            description: first("description").replace("\r\n", "\n"),
            projects: pairs
                .iter()
                .filter(|(k, _)| k == "project")
                .filter_map(|(_, v)| v.parse().ok())
                .collect(),
        }
    }

    /// `board` with the name and description applied, and whether the name
    /// is acceptable.
    fn apply(&self, mut board: Board) -> (Board, crate::error::Result<()>) {
        let valid = if self.name.trim().is_empty() {
            Err(IterError::EmptyBoardName)
        } else {
            Ok(())
        };
        board.name = self.name.trim().to_string();
        board.description = self.description.clone();
        (board, valid)
    }
}

/// A project as the edit form lists it.
struct Choice {
    project: Project,
    /// Bound to the board being edited.
    on: bool,
    /// The name of the other board it is on, if any -- ticking it moves it.
    elsewhere: String,
}

fn choices(db: &Db, board_id: i64, ticked: Option<&[i64]>) -> crate::error::Result<Vec<Choice>> {
    let boards = db.list::<Board>()?;
    Ok(db
        .list::<Project>()?
        .into_iter()
        .map(|project| {
            let on = match ticked {
                Some(ids) => ids.contains(&project.id()),
                None => project.board_id == Some(board_id),
            };
            let elsewhere = boards
                .iter()
                .find(|b| Some(b.id()) == project.board_id && b.id() != board_id)
                .map(|b| b.name.clone())
                .unwrap_or_default();
            Choice { project, on, elsewhere }
        })
        .collect())
}

#[component]
async fn board_form(board: &Board, choices: &[Choice], #[default] error: Option<String>) -> Result<impl View> {
    Ok(view! {
        <h1>"Edit board"</h1>
        if let Some(message) = &error {
            <div class="error">(message.clone())</div>
        }
        <form method="post" action=(format!("/board/{}/edit", board.id()))>
            <label>"Name"</label>
            <input type="text" name="name" value=(board.name.clone())>
            <label>"Description (markdown)"</label>
            <textarea name="description">(board.description.clone())</textarea>
            <label>"Projects"</label>
            for c in choices.iter() {
                <label class="check">
                    <input type="checkbox" name="project" value=(c.project.id().to_string()) if c.on { checked="" }>
                    (c.project.name.clone())
                    if !c.elsewhere.is_empty() {
                        <span class="empty">(format!(" (now on {})", c.elsewhere))</span>
                    }
                </label>
            }
            <button type="submit">"Save"</button>
            " "
            <a href=(format!("/board/{}/info", board.id()))>"Cancel"</a>
        </form>
    })
}

#[page("/board/{id}/info")]
async fn info_page(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (nav, board, projects) = {
        let db = open_db()?;
        let board = db.get::<Board>(id)?.ok_or_not_found()?;
        (Nav::load(&db)?, board, db.projects_for_board(id)?)
    };
    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
    Ok(view! {
        shell(
            nav: &nav,
            sel: Sel::Board,
            <div class="crumbs"><a href="/boards">"Boards"</a></div>
            <h1>(board.name.clone())</h1>
            board_tabs(id: id, active: "info", date: &today)
            if !board.description.is_empty() {
                <pre class="desc">(board.description.clone())</pre>
            }
            <h3>"Projects"</h3>
            if projects.is_empty() {
                <p class="empty">"No projects on this board."</p>
            } else {
                <ul>
                    for p in projects.iter() {
                        <li><a href=(format!("/project/{}", p.id()))>(p.name.clone())</a></li>
                    }
                </ul>
            }
        )
    })
}

#[page("/board/{id}/edit")]
async fn edit_page(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (nav, board, choices) = {
        let db = open_db()?;
        let board = db.get::<Board>(id)?.ok_or_not_found()?;
        (Nav::load(&db)?, board, choices(&db, id, None)?)
    };
    Ok(view! { shell(nav: &nav, sel: Sel::Board, board_form(board: &board, choices: &choices)) })
}

#[page(POST "/board/{id}/edit")]
async fn save_page(cx: &Cx, Form(pairs): Form<Vec<(String, String)>>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let form = BoardForm::parse(&pairs);
    let (nav, board, choices, error) = {
        let db = open_db()?;
        let existing = db.get::<Board>(id)?.ok_or_not_found()?;
        let (board, valid) = form.apply(existing);
        let saved = valid.and_then(|()| {
            db.update(id, &board)?;
            bind_projects(&db, id, &form.projects)
        });
        match saved {
            Ok(()) => return Err(see_other(format!("/board/{id}/info")).into()),
            Err(e) => (Nav::load(&db)?, board, choices(&db, id, Some(&form.projects))?, e.to_string()),
        }
    };
    Ok(view! {
        shell(nav: &nav, sel: Sel::Board, board_form(board: &board, choices: &choices, error: Some(error)))
    })
}

/// Makes `ids` exactly the projects on the board: the others that were on
/// it are unbound, the rest are bound (moving them off any other board).
fn bind_projects(db: &Db, board_id: i64, ids: &[i64]) -> crate::error::Result<()> {
    for mut project in db.list::<Project>()? {
        let wanted = ids.contains(&project.id());
        let target = match (wanted, project.board_id) {
            (true, _) => Some(board_id),
            (false, Some(b)) if b == board_id => None,
            (false, current) => current,
        };
        if target != project.board_id {
            project.board_id = target;
            db.update(project.id(), &project)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task() -> Task {
        Task::template(1, String::new(), TaskStatus::Queue)
    }

    #[test]
    fn board_form_reads_repeated_project_fields() {
        let pairs = |v: &[(&str, &str)]| -> Vec<(String, String)> {
            v.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
        };
        let form = BoardForm::parse(&pairs(&[
            ("name", " work "),
            ("description", "a\r\nb"),
            ("project", "3"),
            ("project", "x"),
            ("project", "5"),
        ]));
        assert_eq!(form.projects, [3, 5]);
        let board = Board { id: Some(1), name: String::new(), description: String::new() };
        let (board, valid) = form.apply(board);
        assert!(valid.is_ok());
        assert_eq!((board.name.as_str(), board.description.as_str()), ("work", "a\nb"));
        let (_, blank) = BoardForm::parse(&pairs(&[("name", " ")])).apply(board);
        assert!(matches!(blank, Err(IterError::EmptyBoardName)));
    }

    #[test]
    fn dropping_on_an_hour_schedules_with_a_default_hour() {
        let mut t = task();
        schedule(&mut t, "2026-09-28 09:00").expect("valid slot");
        assert_eq!(
            t.start_time,
            NaiveDateTime::parse_from_str("2026-09-28 09:00", START_TIME_FMT).ok()
        );
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

    #[test]
    fn quadrants_set_the_flags_and_the_side_list_only_unplaces() {
        let mut t = task();
        place(&mut t, "both").expect("known quadrant");
        assert!((t.urgency, t.importance, t.matrix_placed) == (true, true, true));
        place(&mut t, "urgent").expect("known quadrant");
        assert!((t.urgency, t.importance) == (true, false));
        place(&mut t, "left").expect("side list");
        assert!(!t.matrix_placed && t.urgency);
        place(&mut t, "neither").expect("known quadrant");
        assert!((t.urgency, t.importance, t.matrix_placed) == (false, false, true));
        assert!(place(&mut t, "nowhere").is_err());
    }
}
