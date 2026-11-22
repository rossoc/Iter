//! Projects as `data_table` rows (or a plain list in a card): name link, then
//! the path in mono, leaf strong and directories muted; a long path loses its start.

use super::table::{AddRow, cell, data_table, row};
use topcoat::{
    Result,
    view::{View, component, view},
};

/// `path` split after the last `/`: directories, then the leaf.
fn split_leaf(path: &str) -> (&str, &str) {
    let trimmed = path.trim_end_matches('/');
    let cut = trimmed.rfind('/').map_or(0, |i| i + 1);
    path.split_at(cut)
}

/// One row, ready to show (mapping in `web/project_rows.rs`).
pub struct ProjectItem<'a> {
    pub name: &'a str,
    pub href: String,
    pub path: String,
}

#[component]
async fn path_span(path: &str) -> Result<impl View> {
    let (dirs, leaf) = split_leaf(path);
    // outer span is right-to-left so overflow cuts the start; the inner keeps path order
    Ok(view! {
        <span class="path" title=(path)>
            <span class="path-in">(dirs)<b class="leaf">(leaf)</b></span>
        </span>
    })
}

/// Projects as a headerless `data_table`; `add` is the first row's link (opens
/// the New project pop-up); `empty` the line when no rows. `compact` is a plain
/// list without the add row, for inside a card.
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
