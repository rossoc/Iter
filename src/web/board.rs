//! The board pages: the list of boards, a board's day agenda and its
//! Eisenhower matrix. Both boards are drag-and-drop: `board.js` posts the
//! dropped task and where it landed to the two `POST` routes below, and
//! reloads.

use super::edit::{EditForm, Loaded, load, submit};
use super::forms::text;
use super::layout::{Nav, Sel, cls, description, error_box, field, form_actions, shell, textarea};
use super::open_db;
use crate::db::{Db, Table};
use crate::models::{
    Board, Card, Duration, IMPORTANT_TAG, Named, Priority, Project, START_TIME_FMT, Tag, Task,
    TaskStatus, URGENT_TAG, parse_start_time,
};
use crate::reporting::fmt_date;
use crate::utils::report::parse_day;
use chrono::{Local, NaiveDate};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::{Form, Json},
        error::{RouterErrorExt, bad_request},
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

/// How the web board draws a [`Card`].
impl Card {
    /// The colour class: `ui` (both flags), `u`, `i`, or none.
    fn class(&self) -> &'static str {
        match (self.priority.urgent, self.priority.important) {
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
        if let Some(end) = self.task.end_time() {
            slot.push_str(&format!("-{}", end.format("%H:%M")));
        }
        slot
    }
}

/// The colours a board draws with: the Urgent and Important tags' colours,
/// as CSS variables. Both tags are seeded and can't be deleted.
fn colors_style(db: &Db) -> crate::error::Result<String> {
    Ok(format!(
        "--urgent:{};--important:{}",
        db.resolve::<Tag>(URGENT_TAG)?.color,
        db.resolve::<Tag>(IMPORTANT_TAG)?.color
    ))
}

/// Every unfinished task on the board's projects, urgent first, then by name.
fn cards(db: &Db, board_id: i64) -> crate::error::Result<Vec<Card>> {
    let mut out = db.cards(board_id)?;
    out.retain(|c| c.task.status != TaskStatus::Done);
    out.sort_by(|a, b| (b.priority.urgent, &a.label).cmp(&(a.priority.urgent, &b.label)));
    Ok(out)
}

fn pick(all: &[Card], keep: impl Fn(&Card) -> bool) -> Vec<Card> {
    all.iter().filter(|c| keep(c)).cloned().collect()
}

fn day_of(cx: &Cx) -> Result<NaiveDate> {
    let today = Local::now().date_naive();
    Ok(match query_params::<DayQuery>(cx)?.date.as_deref() {
        Some(text) => parse_day(text).unwrap_or(today),
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
    let start = parse_start_time(target).ok_or_else(|| format!("invalid start time '{target}'"))?;
    task.start_time = Some(start);
    task.duration.get_or_insert(Duration(60));
    Ok(())
}

/// The matrix's quadrants: the drop target `board.js` posts, the flags it
/// stands for, and its heading.
const QUADRANTS: [(&str, Priority, &str); 4] = [
    ("both", priority(true, true), "Important and urgent"),
    ("important", priority(false, true), "Important, not urgent"),
    ("urgent", priority(true, false), "Urgent, not important"),
    (
        "neither",
        priority(false, false),
        "Not urgent, not important",
    ),
];

const fn priority(urgent: bool, important: bool) -> Priority {
    Priority { urgent, important }
}

/// Applies a matrix drop. `target` is `left` (back to the side list, flags
/// untouched) or a quadrant, whose flags the task then gets.
fn place(task: &mut Task, target: &str) -> std::result::Result<Option<Priority>, String> {
    if target == "left" {
        task.matrix_placed = false;
        return Ok(None);
    }
    let &(_, flags, _) = QUADRANTS
        .iter()
        .find(|q| q.0 == target)
        .ok_or_else(|| format!("unknown quadrant '{target}'"))?;
    task.matrix_placed = true;
    Ok(Some(flags))
}

#[derive(Deserialize)]
struct Drop {
    task_id: i64,
    target: String,
}

/// Loads the dropped task, checks it is one of this board's, and stores what
/// `apply` makes of it -- including new flags, when it returns some.
fn drop_on(
    board_id: i64,
    drop: &Drop,
    apply: impl FnOnce(&mut Task, &str) -> std::result::Result<Option<Priority>, String>,
) -> Result<Json<bool>> {
    let db = open_db()?;
    let mut task = db.get::<Task>(drop.task_id)?.ok_or_not_found()?;
    let project = db.get::<Project>(task.project_id)?.ok_or_not_found()?;
    (project.board_id == Some(board_id))
        .then_some(())
        .ok_or_not_found()?;
    let flags = apply(&mut task, &drop.target).map_err(bad_request)?;
    db.update(drop.task_id, &task)?;
    if let Some(flags) = flags {
        db.set_priority(drop.task_id, flags)?;
    }
    Ok(Json(true))
}

#[route(POST "/board/{id}/schedule")]
async fn schedule_task(cx: &Cx, Form(drop): Form<Drop>) -> Result<Json<bool>> {
    drop_on(*path_param::<Id>(cx)?, &drop, |task, target| {
        schedule(task, target).map(|()| None)
    })
}

#[route(POST "/board/{id}/matrix")]
async fn place_task(cx: &Cx, Form(drop): Form<Drop>) -> Result<Json<bool>> {
    drop_on(*path_param::<Id>(cx)?, &drop, place)
}

// ---- views ------------------------------------------------------------------

#[component]
async fn card(c: &Card) -> Result<impl View> {
    Ok(view! {
        <div class=(c.class()) draggable="true" data-task=(c.task.id().to_string())>
            <a href=(format!("/task/{}", c.task.id()))>(c.label.clone())</a>
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
    Ok(view! {
        <div class="tabs">
            <a class=(cls(active == "agenda")) href=(format!("/board/{id}?date={date}"))>"Agenda"</a>
            <a class=(cls(active == "matrix")) href=(format!("/board/{id}/matrix"))>"Eisenhower matrix"</a>
            <a class=(cls(active == "info")) href=(format!("/board/{id}/info"))>"Info"</a>
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
    let important = pick(&all, |c| {
        c.task.start_time.is_none() && c.priority.important
    });
    let other = pick(&all, |c| {
        c.task.start_time.is_none() && !c.priority.important
    });
    let hours: Vec<(String, String, Vec<Card>)> = (0..24)
        .map(|h| {
            let slot = day.and_hms_opt(h, 0, 0).expect("hour is in range");
            let here = pick(&all, |c| {
                c.task
                    .start_time
                    .is_some_and(|s| s >= slot && s < slot + chrono::Duration::hours(1))
            });
            (
                format!("{h:02}:00"),
                slot.format(START_TIME_FMT).to_string(),
                here,
            )
        })
        .collect();
    let date = fmt_date(day);
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
                        <a href=(format!("/board/{id}?date={}", fmt_date(prev)))>"<"</a>
                        <strong>(day.format("%A %-d %B %Y").to_string())</strong>
                        <a href=(format!("/board/{id}?date={}", fmt_date(next)))>">"</a>
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
    let quadrants: Vec<(&str, String, &str, Vec<Card>)> = QUADRANTS
        .iter()
        .map(|&(target, flags, title)| {
            let here = pick(&all, |c| c.task.matrix_placed && c.priority == flags);
            (target, format!("q-{target}"), title, here)
        })
        .collect();
    let today = fmt_date(Local::now().date_naive());
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
                    for (target, class, title, here) in quadrants.iter() {
                        zone(target: target.to_string(), class: class, title: title, cards: here)
                    }
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
            description: text(&first("description")),
            projects: pairs
                .iter()
                .filter(|(k, _)| k == "project")
                .filter_map(|(_, v)| v.parse().ok())
                .collect(),
        }
    }
}

impl EditForm for BoardForm {
    type Row = Board;

    fn saved(id: i64) -> String {
        format!("/board/{id}/info")
    }

    fn apply(&self, mut board: Board) -> (Board, crate::error::Result<()>) {
        board.name = self.name.trim().to_string();
        board.description = self.description.clone();
        let valid = board.validate();
        (board, valid)
    }

    /// The projects on the board too.
    fn save(&self, db: &Db, id: i64, board: &Board) -> crate::error::Result<()> {
        db.update(id, board)?;
        db.set_projects::<Board>(id, &self.projects)
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
            Choice {
                project,
                on,
                elsewhere,
            }
        })
        .collect())
}

#[component]
async fn board_form(
    board: &Board,
    choices: &[Choice],
    #[default] error: Option<String>,
) -> Result<impl View> {
    Ok(view! {
        <h1>"Edit board"</h1>
        error_box(error: &error)
        <form method="post" action=(format!("/board/{}/edit", board.id()))>
            field(name: "name", label: "Name", value: &board.name)
            textarea(text: &board.description)
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
            form_actions(cancel: format!("/board/{}/info", board.id()))
        </form>
    })
}

#[page("/board/{id}/info")]
async fn info_page(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (nav, board, projects) = {
        let db = open_db()?;
        let board = db.get::<Board>(id)?.ok_or_not_found()?;
        (Nav::load(&db)?, board, db.projects_in::<Board>(id)?)
    };
    let today = fmt_date(Local::now().date_naive());
    Ok(view! {
        shell(
            nav: &nav,
            sel: Sel::Board,
            <div class="crumbs"><a href="/boards">"Boards"</a></div>
            <h1>(board.name.clone())</h1>
            board_tabs(id: id, active: "info", date: &today)
            description(text: &board.description)
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
    let page: Loaded<Board, _> = load(id, |db| choices(db, id, None))?;
    Ok(view! {
        shell(nav: &page.nav, sel: Sel::Board, board_form(board: &page.row, choices: &page.extra))
    })
}

#[page(POST "/board/{id}/edit")]
async fn save_page(cx: &Cx, Form(pairs): Form<Vec<(String, String)>>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let form = BoardForm::parse(&pairs);
    let (page, error) = submit(id, &form, |db| choices(db, id, Some(&form.projects)))?;
    Ok(view! {
        shell(nav: &page.nav, sel: Sel::Board, board_form(board: &page.row, choices: &page.extra, error: Some(error)))
    })
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
            v.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };
        let form = BoardForm::parse(&pairs(&[
            ("name", " work "),
            ("description", "a\r\nb"),
            ("project", "3"),
            ("project", "x"),
            ("project", "5"),
        ]));
        assert_eq!(form.projects, [3, 5]);
        let board = Board {
            id: Some(1),
            name: String::new(),
            description: String::new(),
        };
        let (board, valid) = form.apply(board);
        assert!(valid.is_ok());
        assert_eq!(
            (board.name.as_str(), board.description.as_str()),
            ("work", "a\nb")
        );
        let (_, blank) = BoardForm::parse(&pairs(&[("name", " ")])).apply(board);
        assert!(matches!(
            blank,
            Err(crate::error::IterError::EmptyName("board"))
        ));
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

    #[test]
    fn quadrants_set_the_flags_and_the_side_list_only_unplaces() {
        let mut t = task();
        assert_eq!(place(&mut t, "both"), Ok(Some(priority(true, true))));
        assert!(t.matrix_placed);
        assert_eq!(place(&mut t, "urgent"), Ok(Some(priority(true, false))));
        assert_eq!(place(&mut t, "left"), Ok(None));
        assert!(!t.matrix_placed);
        assert_eq!(place(&mut t, "neither"), Ok(Some(priority(false, false))));
        assert!(t.matrix_placed);
        assert!(place(&mut t, "nowhere").is_err());
    }
}
