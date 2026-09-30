//! Projects as rows: the name (a link), then its whole path in mono under it,
//! left-aligned. The last segment is what tells projects apart, so it is the
//! strong one and the directories before it are muted. Nothing is cut: a long
//! path wraps, preferably after a `/`.

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
/// and `group_card` are the cards.)
#[component]
pub async fn project_list(
    items: &[ProjectItem<'_>],
    #[default] compact: bool,
) -> Result<impl View> {
    Ok(view! {
        <ul class=(if compact { "project-list compact" } else { "project-list" })>
            for p in items.iter() {
                let (dirs, leaf) = split_leaf(&p.path);
                <li>
                    <a class="u" href=(p.href.as_str())>(p.name)</a>
                    <span class="path">
                        for dir in dirs.split_inclusive('/') {
                            (dir)<wbr>
                        }
                        <b class="leaf">(leaf)</b>
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
