//! A board's Eisenhower matrix (`/board/{id}/matrix`): four quadrants and the list of unplaced tasks.

use super::Id;
use super::board_cards::CardCtx;
use super::board_header::{BoardSection, title_parts};
use super::board_page::{BoardPage, board_screen, load_board};
use super::drop::{Drop, back_or, drop_on};
use super::pick::{PickLinks, PickWords, Picked, moved_note, pick_fields};
use super::quadrants::{Matrix, QUADRANTS, place, quadrant_index, split};
use super::ui::button::post_button;
use super::ui::layout::{jump_link, matrix_grid, side_lists, sticky_head};
use super::ui::pick_bar::pick_bar;
use super::ui::pick_here::pick_here;
use super::ui::quadrant::quadrant;
use super::ui::side_rail::{side_fold, side_rail};
use super::ui::task_card::{Edge, task_card};
use super::ui::unplaced_list::unplaced_list;
use super::url::{board_matrix_url, with_side};
use crate::db::Table;
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::{Form, Json},
        page, path_param, query_params, route,
    },
    view::{Child, View, component, view},
};

const NOT_PLACED: &str = "not-placed";

const FORM_POST: (&str, &str) = ("form", "1");

#[query_params(error = bad_request)]
struct MatrixQuery {
    pick: Option<String>,
    moved: Option<String>,
    side: Option<String>,
}

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(matrix_page).route(place_task)
}

/// A pick form also sends the `form` marker: the answer is then a redirect to the moved card.
#[route(POST "/board/{id}/matrix")]
async fn place_task(cx: &Cx, Form(drop): Form<Drop>) -> Result<Json<bool>> {
    let id = *path_param::<Id>(cx)?;
    let done = drop_on(id, &drop, place)?;
    // the URL is built from the ids, never from the request's text
    let back = drop
        .form
        .is_some()
        .then(|| PickLinks::new(board_matrix_url(id)).moved(drop.task_id));
    back_or(done, back)
}

#[page("/board/{id}/matrix")]
async fn matrix_page(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let query = query_params::<MatrixQuery>(cx)?;
    let page = load_board(id, query.pick.as_deref(), query.moved.as_deref(), split)?;
    let lead = page
        .picked
        .as_ref()
        .map(|p| pick_words(p).title(&p.title))
        .unwrap_or_default();
    let folded = query.side.as_deref() == Some("off");
    Ok(view! {
        screen(page: &page, lead: &lead, folded: folded)
    })
}

fn quadrant_of(task: &Picked) -> Option<usize> {
    task.placed.then(|| quadrant_index(task.priority))
}

fn place_name(task: &Picked) -> &'static str {
    quadrant_of(task).map_or("Backlog", |q| QUADRANTS[q].name)
}

fn pick_words(task: &Picked) -> PickWords {
    PickWords::of(
        quadrant_of(task).is_some(),
        PickWords {
            verb: "Placing",
            action: "Place",
        },
    )
}

#[component]
async fn screen(page: &BoardPage<Matrix>, lead: &str, folded: bool) -> Result<impl View> {
    let BoardPage {
        board,
        colors,
        picked,
        moved,
        data: matrix,
    } = page;
    let id = board.id();
    let ctx = CardCtx {
        picked: picked.as_ref().map(|p| p.id),
    };
    let base = board_matrix_url(id);
    let kept = with_side(base.clone(), folded);
    let links = PickLinks::new(&kept);
    let here = match picked {
        Some(p) => PickLinks::new(&base).pick(p.id),
        None => base.clone(),
    };
    let fold = with_side(here.clone(), true);
    let style = colors.style();
    let now = picked.as_ref().and_then(quadrant_of);
    let state = now.map(|q| QUADRANTS[q].name).unwrap_or_default();
    let words = picked.as_ref().map(pick_words);
    let (verb, action) = words.map_or(("", ""), |w| (w.verb, w.action));
    let title = title_parts(lead, &[BoardSection::Matrix.label(), board.name.as_str()]);
    let note = moved
        .as_ref()
        .map(|m| moved_note(&m.title, place_name(m)))
        .unwrap_or_default();
    // the side's view owns its own (`ctx` is copied)
    let side_links = links.clone();
    Ok(view! {
        board_screen(board: board, section: BoardSection::Matrix, title: &title, post: &base, style: &style, note: &note, folded: folded, empty: matrix.is_empty(),
            side: Child::new(view! {
                if folded {
                    side_rail(href: here.as_str(), label: "Backlog", total: matrix.waiting.len(), overdue: 0, target: "left")
                } else {
                    side_lists(id: NOT_PLACED, label: "Backlog",
                        side_fold(href: fold.as_str(), label: "Backlog")
                        unplaced_list(id: "waiting-title", title: "Backlog", empty: "Everything is placed.", count: matrix.waiting.len(), target: "left", alone: true,
                            for card in matrix.waiting.iter() {
                                task_card(card: ctx.view(card, "Place", side_links.pick(card.task.id())), draggable: true)
                            }
                        )
                    )
                }
            }),
            if let Some(p) = picked {
                sticky_head(
                    pick_bar(verb: verb, title: &p.title, state: state, prompt: "Choose a quadrant.", cancel: links.cancel(p.id),
                        if now.is_some() {
                            post_button(action: base.as_str(), fields: pick_fields(p.id, "left", Some(FORM_POST)), small: true, "Unplace")
                        }
                    )
                )
            }
            if !folded {
                jump_link(target: format!("#{NOT_PLACED}"), "Skip to the backlog")
            }
            matrix_grid(
                for (n, (q, cards)) in QUADRANTS.iter().zip(matrix.placed.iter()).enumerate() {
                    quadrant(id: q.id, target: q.target, number: n + 1, name: q.name, rule: q.rule, empty: q.empty, edge: Edge::of(q.flags), count: cards.len(),
                        if let Some(p) = picked {
                            if now != Some(n) {
                                pick_here(action: base.as_str(), fields: pick_fields(p.id, q.target, Some(FORM_POST)), verb: action, at: q.name)
                            }
                        }
                        for card in cards.iter() {
                            task_card(card: ctx.view(card, "Move", links.pick(card.task.id())), draggable: true)
                        }
                    )
                }
            )
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Priority;

    fn task(placed: bool, urgent: bool, important: bool) -> Picked {
        Picked {
            id: 1,
            title: "p/t".into(),
            start: None,
            placed,
            priority: Priority { urgent, important },
        }
    }

    #[test]
    fn a_placed_task_is_moved_and_any_other_placed() {
        let waiting = task(false, true, true);
        assert_eq!(
            (pick_words(&waiting).verb, pick_words(&waiting).action),
            ("Placing", "Place")
        );
        assert_eq!(place_name(&waiting), "Backlog");
        let placed = task(true, true, false);
        assert_eq!(pick_words(&placed), PickWords::MOVING);
        assert_eq!(quadrant_of(&placed), Some(2));
        assert_eq!(place_name(&placed), "Delegate");
    }

    #[test]
    fn a_move_says_where_the_task_is() {
        let plan = task(true, false, true);
        assert_eq!(
            moved_note(&plan.title, place_name(&plan)),
            "Moved p/t to Plan"
        );
        let left = task(false, false, true);
        assert_eq!(
            moved_note(&left.title, place_name(&left)),
            "Moved p/t to Backlog"
        );
    }

    #[test]
    fn the_pick_mode_names_the_document() {
        assert_eq!(PickWords::MOVING.title("p/t"), "Moving p/t");
    }
}
