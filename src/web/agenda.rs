//! A board's day agenda (`/board/{id}?date=`). The page is the day (the
//! hero) beside the lists of tasks not yet scheduled; a card is dragged into
//! an hour with `board.js`, or moved with the pick mode (`?pick=`, `pick.rs`),
//! plain links and forms. The POST route is `agenda_drop.rs`, the cards and
//! the side lists `agenda_cards.rs`. Nothing here has rules of its own: it is
//! all shared components (`ui/`). The loading is here too.

use super::agenda_cards::{Cards, Fold, NOT_SCHEDULED, side};
use super::agenda_day::{Day, hour_label, hour_target, neighbours, split};
use super::agenda_pick::{day_url, links};
use super::board_cards::{CLOCK_FMT, CardCtx};
use super::board_header::{BoardSection, board_header, title_parts};
use super::board_page::{BoardPage, load_board};
use super::pick::{Picked, moved_note, pick_fields};
use super::ui::board_root::board_root;
use super::ui::button::post_button;
use super::ui::columns::info_columns;
use super::ui::day_nav::{DayLink, day_nav};
use super::ui::frame::frame;
use super::ui::hour_grid::{Now, hour_grid, hour_slot};
use super::ui::jump_link::jump_link;
use super::ui::pick_bar::pick_bar;
use super::ui::pick_here::pick_here;
use super::ui::sticky_head::sticky_head;
use super::ui::task_card::task_card;
use super::url::{BOARDS, board_day_url, board_schedule_url, board_url, with_side};
use super::{Id, now};
use crate::db::Table;
use crate::reporting::fmt_date;
use crate::utils::report::parse_day;
use chrono::{NaiveDate, NaiveDateTime, Timelike};
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, page, path_param, query_params},
    view::{View, component, view},
};

/// The id of the day's heading: the hours are labelled by it.
const DAY_TITLE: &str = "day-title";

#[query_params(error = bad_request)]
struct AgendaQuery {
    date: Option<String>,
    /// The task being moved (the pick mode).
    pick: Option<String>,
    /// The task that was just moved (the note says so).
    moved: Option<String>,
    /// `off`: the Unscheduled column is folded into its rail.
    side: Option<String>,
}

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(show)
}

/// The day asked for: `date` when it is a valid `yyyy-mm-dd`, else `today`.
fn day_from(date: Option<&str>, today: NaiveDate) -> NaiveDate {
    date.and_then(|d| parse_day(d).ok()).unwrap_or(today)
}

#[page("/board/{id}")]
async fn show(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let query = query_params::<AgendaQuery>(cx)?;
    let now = now();
    let day = day_from(query.date.as_deref(), now.date());
    let page = load_board(id, query.pick.as_deref(), query.moved.as_deref(), |cards| {
        split(cards, day, now.date())
    })?;
    let folded = query.side.as_deref() == Some("off");
    let model = Model::of(&page, day, now, folded);
    Ok(view! {
        screen(page: &page, model: &model, day: day, now: now)
    })
}

/// The now line of the hour `h`, when the agenda shows today and `h` is the
/// current hour: how far into the hour it is.
fn now_line(now: NaiveDateTime, day: NaiveDate, h: usize) -> Option<Now> {
    (now.date() == day && now.hour() as usize == h).then(|| Now {
        at: (now.minute() as f32 * 60.0 + now.second() as f32) / 3600.0,
        text: format!("Now, {}", now.format(CLOCK_FMT)),
    })
}

/// The Today link: the current hour when nothing is picked (`#now`), else
/// the banner (the pick mode goes along).
fn today_url(id: i64, today: NaiveDate, picked: Option<i64>, folded: bool) -> String {
    with_side(today_link(id, today, picked), folded)
}

fn today_link(id: i64, today: NaiveDate, picked: Option<i64>) -> String {
    match picked {
        Some(_) => day_url(id, today, picked),
        None => format!("{}#now", board_day_url(id, today)),
    }
}

/// Where a moved task is, in words: its slot ("Wed 30 Sep, 09:00"), or the
/// lists.
fn moved_to(task: &Picked) -> String {
    match task.state() {
        state if state.is_empty() => "Unscheduled".to_string(),
        state => state,
    }
}

/// What the screen shows beside the cards, worked out from the page: the
/// screen only lays it out.
struct Model {
    /// The parts of the document title.
    title: Vec<String>,
    /// The custom properties of the priority colors.
    style: String,
    /// What a move says, once done.
    note: String,
    /// Where drops and the pick forms post.
    schedule: String,
    /// The picked task's slot, in words (empty when it has none).
    state: String,
    /// The day as `yyyy-mm-dd`, and in words.
    date: String,
    heading: String,
    /// The label of each hour.
    labels: Vec<String>,
    previous: DayLink,
    next: DayLink,
    today: String,
    is_today: bool,
    /// The task being moved, if any.
    pick: Option<i64>,
    /// The Unscheduled column is folded into its rail.
    folded: bool,
    /// The links that fold and unfold it.
    fold: Fold,
}

impl Model {
    fn of(page: &BoardPage<Day>, day: NaiveDate, now: NaiveDateTime, folded: bool) -> Model {
        let BoardPage {
            board,
            colors,
            picked,
            moved,
            ..
        } = page;
        let id = board.id();
        let pick = picked.as_ref().map(|p| p.id);
        let lead = picked
            .as_ref()
            .map(|p| p.words().title(&p.title))
            .unwrap_or_default();
        let short = day.format("%a %-d %b").to_string();
        let (before, after) = neighbours(day);
        let step = |to: NaiveDate, what: &str| DayLink {
            href: with_side(day_url(id, to, pick), folded),
            label: format!("{what} day, {}", to.format("%A %-d %B")),
        };
        Model {
            title: title_parts(
                &lead,
                &[&short, BoardSection::Agenda.label(), board.name.as_str()],
            )
            .into_iter()
            .map(String::from)
            .collect(),
            style: colors.style(),
            note: moved
                .as_ref()
                .map(|m| moved_note(&m.title, &moved_to(m)))
                .unwrap_or_default(),
            schedule: board_schedule_url(id),
            state: picked.as_ref().map(Picked::state).unwrap_or_default(),
            date: fmt_date(day),
            heading: day.format("%A %-d %B %Y").to_string(),
            labels: (0..24).map(hour_label).collect(),
            previous: step(before, "Previous"),
            next: step(after, "Next"),
            today: today_url(id, now.date(), pick, folded),
            is_today: day == now.date(),
            pick,
            folded,
            fold: Fold {
                fold: with_side(day_url(id, day, pick), true),
                unfold: day_url(id, day, pick),
            },
        }
    }
}

#[component]
async fn screen(
    page: &BoardPage<Day>,
    model: &Model,
    day: NaiveDate,
    now: NaiveDateTime,
) -> Result<impl View> {
    let BoardPage {
        board,
        picked,
        data: split,
        ..
    } = page;
    let id = board.id();
    let Model {
        title,
        style,
        note,
        schedule,
        state,
        date,
        heading,
        labels,
        previous,
        next,
        today,
        is_today,
        pick,
        folded,
        fold,
    } = model;
    let ctx = Cards {
        board: id,
        day,
        folded: *folded,
        ctx: CardCtx { picked: *pick },
    };
    let title: Vec<&str> = title.iter().map(String::as_str).collect();
    Ok(view! {
        frame(
            title: &title,
            current: BOARDS,
            board_header(board: board, section: BoardSection::Agenda)
            board_root(post: schedule.as_str(), style: style, note: note,
                info_columns(class: if *folded { "side-folded" } else { "" },
                    <div>
                        sticky_head(
                            if let Some(p) = picked {
                                pick_bar(verb: p.words().verb, title: &p.title, state: state, prompt: "Choose an hour.", cancel: links(id, day).cancel(p.id),
                                    if p.scheduled() {
                                        post_button(action: schedule.as_str(), fields: pick_fields(p.id, "", Some(("date", date))), small: true, "Unschedule")
                                    }
                                )
                            }
                            day_nav(
                                id: DAY_TITLE,
                                title: heading,
                                iso: date,
                                action: board_url(id),
                                pick: *pick,
                                folded: *folded,
                                prev: previous.clone(),
                                next: next.clone(),
                                today: today.clone(),
                                is_today: *is_today,
                            )
                        )
                        if !split.is_empty() && !*folded {
                            jump_link(target: format!("#{NOT_SCHEDULED}"), "Skip to unscheduled tasks")
                        }
                        hour_grid(label_id: DAY_TITLE,
                            for (h, cards) in split.hours.iter().enumerate() {
                                hour_slot(label: &labels[h], target: hour_target(day, h), now: now_line(now, day, h),
                                    if let Some(p) = picked {
                                        pick_here(action: schedule.as_str(), fields: pick_fields(p.id, &hour_target(day, h), Some(("date", date))), verb: p.words().action, at: &labels[h], slot: true)
                                    }
                                    for card in cards.iter() {
                                        task_card(card: ctx.view(card), draggable: true)
                                    }
                                )
                            }
                        )
                    </div>
                    side(split: split, ctx: &ctx, to: fold)
                )
            )
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::parse_start_time;

    fn at(s: &str) -> NaiveDateTime {
        crate::models::parse_start_time(s).expect("valid time")
    }

    #[test]
    fn a_bad_or_missing_date_is_today() {
        let today = at("2026-09-29 12:00").date();
        assert_eq!(day_from(None, today), today);
        assert_eq!(day_from(Some("nonsense"), today), today);
        assert_eq!(
            day_from(Some("2026-01-05"), today),
            at("2026-01-05 00:00").date()
        );
    }

    #[test]
    fn the_now_line_is_only_in_the_current_hour_of_today() {
        let now = at("2026-09-29 09:30");
        let today = now.date();
        let line = now_line(now, today, 9).expect("in the hour");
        assert!((line.at - 0.5).abs() < 0.001);
        assert_eq!(line.text, "Now, 09:30");
        assert!(now_line(now, today, 10).is_none());
        assert!(now_line(now, at("2026-09-30 00:00").date(), 9).is_none());
    }

    #[test]
    fn the_today_link_lands_on_the_hour_or_keeps_the_pick() {
        let day = at("2026-09-29 00:00").date();
        assert_eq!(
            today_url(2, day, None, false),
            "/board/2?date=2026-09-29#now"
        );
        assert_eq!(
            today_url(2, day, Some(7), false),
            "/board/2?date=2026-09-29&pick=7#pick"
        );
        assert_eq!(
            today_url(2, day, None, true),
            "/board/2?date=2026-09-29&side=off#now"
        );
    }

    #[test]
    fn a_move_says_the_slot_or_the_lists() {
        let mut task = Picked {
            id: 1,
            title: "p/t".into(),
            start: None,
            placed: false,
            priority: Default::default(),
        };
        assert_eq!(moved_to(&task), "Unscheduled");
        task.start = parse_start_time("2026-09-30 09:00");
        assert_eq!(
            moved_note(&task.title, &moved_to(&task)),
            "Moved p/t to Wed 30 Sep, 09:00"
        );
    }
}
