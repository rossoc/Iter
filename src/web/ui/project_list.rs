//! Projects as rows of a `data_table` (or, in a card, of a plain list): the
//! name (a link), then its path in mono on the same line. The last segment is what tells projects apart, so it is the strong
//! one and the directories before it are muted; a path too long for the row
//! loses its start ("…/src/app"), never its end. The whole path is the
//! tooltip.

use super::table::{AddRow, cell, data_table, row};
use topcoat::{
    Result,
    view::{View, component, view},
};

/// `path` in two parts: the directories (up to and including the last `/`)
/// and the last segment, which keeps a trailing `/` if there is one.
fn split_leaf(path: &str) -> (&str, &str) {
    let trimmed = path.trim_end_matches('/');
    let cut = trimmed.rfind('/').map_or(0, |i| i + 1);
    path.split_at(cut)
}

/// One row, ready to show (see `web/project_rows.rs` for the mapping from
/// projects). `path` is the text shown for the path.
pub struct ProjectItem<'a> {
    pub name: &'a str,
    pub href: String,
    pub path: String,
}

/// A project's path, cut at the start when it does not fit, so the last
/// segment shows.
#[component]
async fn path_span(path: &str) -> Result<impl View> {
    let (dirs, leaf) = split_leaf(path);
    // the outer span is right-to-left so an overflow cuts the start; the
    // inner one keeps the path's own order
    Ok(view! {
        <span class="path" title=(path)>
            <span class="path-in">(dirs)<b class="leaf">(leaf)</b></span>
        </span>
    })
}

/// The projects as the rows of a headerless `data_table` (see `table.rs`);
/// `add` is where the first row leads, when the list has one: the whole row
/// is a link ("New project…") that opens the New project pop-up
/// (`modal.rs`); `empty` the line shown when there are no rows.
///
/// `compact` is the variant for inside a card, a plain list without the
/// add row. (Not called `card`: `task_card` and `group_card` are the cards.)
#[component]
pub async fn project_list(
    items: &[ProjectItem<'_>],
    #[default] compact: bool,
    #[default] add: Option<String>,
    #[default] empty: Option<&str>,
) -> Result<impl View> {
    let add = add.map(|href| AddRow::link(href, "New project\u{2026}"));
    Ok(view! {
        if compact {
            <ul class="project-list compact">
                for p in items.iter() {
                    <li>
                        <a class="u" href=(p.href.as_str())>(p.name)</a>
                        path_span(path: &p.path)
                    </li>
                }
            </ul>
        } else {
            data_table(caption: "Projects", columns: &["Project", "Path"], class: "projects inline", headed: false, add: add, empty: empty,
                for p in items.iter() {
                    row(
                        cell(<a class="u" href=(p.href.as_str())>(p.name)</a>)
                        cell(class: "path-cell", path_span(path: &p.path))
                    )
                }
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use super::split_leaf;

    #[test]
    fn the_last_segment_is_split_off() {
        assert_eq!(split_leaf("~/src/app"), ("~/src/", "app"));
        assert_eq!(split_leaf("/src/app"), ("/src/", "app"));
        assert_eq!(split_leaf("/app"), ("/", "app"));
    }

    #[test]
    fn a_trailing_slash_stays_with_the_last_segment() {
        assert_eq!(split_leaf("/src/app/"), ("/src/", "app/"));
    }

    #[test]
    fn a_path_without_a_slash_is_all_leaf() {
        assert_eq!(split_leaf("app"), ("", "app"));
        assert_eq!(split_leaf(""), ("", ""));
        assert_eq!(split_leaf("/"), ("", "/"));
    }
}
