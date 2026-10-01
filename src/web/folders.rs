//! The folder browser (`/folders`): where the New project pop-up gets its
//! base path from. It lists a folder's subfolders as links (into, and up),
//! and "Use this folder" goes back to the pop-up with the folder as the base
//! path (and the name). Plain GET pages, no script. The
//! server only lists directory names; it is the user's own machine (the
//! server answers loopback only).

use super::project_new::Home;
use super::project_rows::tilde;
use super::ui::field::field;
use super::ui::folder_list::{FolderLink, folder_list};
use super::ui::frame::frame;
use super::ui::page_header::{Kicker, page_header};
use super::url::folders_url;
use crate::config::expand_home;
use std::path::{Component, Path, PathBuf};
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, error::bad_request, page, query_params},
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(browse)
}

#[query_params(error = bad_request)]
struct FoldersQuery {
    home: Option<String>,
    name: Option<String>,
    base_path: Option<String>,
    hidden: Option<String>,
}

/// `path` without `.` and `..` segments (`..` takes the segment before it
/// off), so a typed path cannot climb by name.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The folder to show for what was typed: the path with `~` expanded, made
/// clean, and, when it is not a folder (or is empty, or relative), the nearest
/// folder above it, else the home folder.
pub(super) fn start_folder(typed: &str, home: &Path) -> PathBuf {
    let typed = typed.trim();
    if typed.is_empty() {
        return home.to_path_buf();
    }
    let path = normalize(&expand_home(typed));
    if !path.is_absolute() {
        return home.to_path_buf();
    }
    path.ancestors()
        .find(|p| p.is_dir())
        .map_or_else(|| home.to_path_buf(), Path::to_path_buf)
}

/// A folder's subfolders, by name without regard to case; the ones that start
/// with a dot only when `hidden`. `None` when the folder cannot be read.
fn subfolders(dir: &Path, hidden: bool) -> Option<Vec<String>> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| hidden || !name.starts_with('.'))
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    Some(names)
}

#[page("/folders")]
async fn browse(cx: &Cx) -> Result<impl View> {
    let query = query_params::<FoldersQuery>(cx)?;
    let home = query
        .home
        .as_deref()
        .and_then(Home::parse)
        .ok_or_else(|| bad_request("Unknown list."))?;
    home.exists()?;
    let hidden = query.hidden.is_some();
    let typed_name = query.name.as_deref().unwrap_or("").trim().to_string();
    let user_home = expand_home("~");
    let dir = start_folder(query.base_path.as_deref().unwrap_or(""), &user_home);
    let listing = subfolders(&dir, hidden);
    let token = home.token();
    let link = |path: &Path| folders_url(&token, &typed_name, &path.to_string_lossy(), hidden);
    let up = dir.parent().map(link);
    let dirs: Vec<FolderLink> = listing
        .iter()
        .flatten()
        .map(|name| FolderLink {
            name: name.clone(),
            href: link(&dir.join(name)),
        })
        .collect();
    // the name the project gets: what was typed, else the folder's own name
    let name = if typed_name.is_empty() {
        dir.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        typed_name.clone()
    };
    let none = if listing.is_none() {
        "This folder cannot be read."
    } else {
        "No folders in here."
    };
    let toggle = (
        folders_url(&token, &typed_name, &dir.to_string_lossy(), !hidden),
        if hidden {
            "Hide hidden folders"
        } else {
            "Show hidden folders"
        },
    );
    let shown = tilde(&dir.to_string_lossy());
    let view = FolderView {
        shown,
        path: dir.to_string_lossy().into_owned(),
        name,
        action: home.page_url(),
        back: home.open_url_with(&typed_name, ""),
        up,
        dirs,
        none,
        toggle,
    };
    Ok(view! { screen(view: &view) })
}

/// What the page shows, worked out.
struct FolderView {
    /// The folder, with the home folder as `~`.
    shown: String,
    /// The folder, in full: the base path "Use this folder" sends back.
    path: String,
    /// The name the new project gets.
    name: String,
    /// The page "Use this folder" opens the pop-up on, and where Cancel goes
    /// (the pop-up, as it was).
    action: String,
    back: String,
    up: Option<String>,
    dirs: Vec<FolderLink>,
    none: &'static str,
    toggle: (String, &'static str),
}

#[component]
async fn screen(view: &FolderView) -> Result<impl View> {
    Ok(view! {
        frame(
            title: &["Choose a folder"],
            page_header(title: "Choose a folder", kicker: Kicker::Eyebrow("New project"))
            <p class="value mono"><bdi>(view.shown.as_str())</bdi></p>
            <form class="folder-use" method="get" action=(view.action.as_str())>
                <input type="hidden" name="new" value="project">
                <input type="hidden" name="base_path" value=(view.path.as_str())>
                field(name: "name", label: "Project name", value: &view.name, identifier: true)
                <button class="button primary" type="submit">"Use this folder"</button>
                <a class="u" href=(view.back.as_str())>"Cancel"</a>
            </form>
            folder_list(up: view.up.clone(), dirs: &view.dirs, none: view.none)
            <p><a class="u" href=(view.toggle.0.as_str())>(view.toggle.1)</a></p>
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway folder tree under the system temp folder.
    fn tree(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("iter-folders-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["b", "A", ".git", "c/inner"] {
            std::fs::create_dir_all(root.join(dir)).expect("dir");
        }
        std::fs::write(root.join("file.txt"), "x").expect("file");
        root
    }

    #[test]
    fn subfolders_are_sorted_without_files_and_hide_dot_folders() {
        let root = tree("list");
        assert_eq!(
            subfolders(&root, false),
            Some(vec!["A".into(), "b".into(), "c".into()])
        );
        assert_eq!(
            subfolders(&root, true),
            Some(vec![".git".into(), "A".into(), "b".into(), "c".into()])
        );
        assert_eq!(subfolders(&root.join("nope"), false), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_typed_path_becomes_the_nearest_folder() {
        let root = tree("start");
        let home = PathBuf::from("/");
        let typed = root.join("c").join("missing").join("deeper");
        assert_eq!(
            start_folder(&typed.to_string_lossy(), &home),
            root.join("c")
        );
        let climbing = root.join("c").join("..").join("b");
        assert_eq!(
            start_folder(&climbing.to_string_lossy(), &home),
            root.join("b")
        );
        assert_eq!(start_folder("", &home), home);
        assert_eq!(start_folder("relative/path", &home), home);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dot_segments_do_not_climb_past_the_root() {
        assert_eq!(
            normalize(Path::new("/a/../../b/./c")),
            PathBuf::from("/b/c")
        );
    }
}
