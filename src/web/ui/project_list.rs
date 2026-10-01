//! Projects as rows: the name (a link), then its path in mono on the same
//! line. The last segment is what tells projects apart, so it is the strong
//! one and the directories before it are muted; a path too long for the row
//! loses its start ("…/src/app"), never its end. The whole path is the
//! tooltip.

use super::{PLUS, icon};
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

/// `compact` is the variant for inside a card. (Not called `card`: `task_card`
/// and `group_card` are the cards.) `add` is where the first row leads, when
/// the list has one: the whole row is a link ("New project…") that opens the
/// New project pop-up (`modal.rs`).
#[component]
pub async fn project_list(
    items: &[ProjectItem<'_>],
    #[default] compact: bool,
    #[default] add: Option<String>,
) -> Result<impl View> {
    Ok(view! {
        <ul class=(if compact { "project-list compact" } else { "project-list" })>
            if let Some(href) = &add {
                <li class="add"><a class="add-link" href=(href.as_str())>(icon(PLUS)) "New project\u{2026}"</a></li>
            }
            for p in items.iter() {
                let (dirs, leaf) = split_leaf(&p.path);
                <li>
                    <a class="u" href=(p.href.as_str())>(p.name)</a>
                    // the outer span is right-to-left so an overflow cuts
                    // the start; the inner one keeps the path's own order
                    <span class="path" title=(p.path.as_str())>
                        <span class="path-in">(dirs)<b class="leaf">(leaf)</b></span>
                    </span>
                </li>
            }
        </ul>
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
