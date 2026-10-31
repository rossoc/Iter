//! The system folder chooser behind a path field's folder button: `POST
//! /folders/pick` opens it (`folder_picker.rs`) and answers with the chosen
//! path. `path_pick.js` posts to it; the path field itself is `ui/field.rs`.

use super::folder_picker::{Picked, pick};
use crate::config::expand_home;
use std::path::{Component, Path, PathBuf};
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::Form,
        response::{IntoResponse, Response},
        route,
    },
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.route(pick_folder)
}

/// What the folder button of a `path_field` posts: the path typed so far.
#[derive(serde::Deserialize)]
struct PickStart {
    #[serde(default)]
    start: String,
}

/// `POST /folders/pick`: opens the system's folder chooser at the nearest
/// existing folder of `start` (else the home folder) and answers JSON:
/// `{"path": ...}` for a chosen folder, `{}` when cancelled, and
/// `{"unavailable": true}` when the system has no chooser. `path_pick.js`
/// calls it; the guard (`guard.rs`) lets only same-origin posts through.
#[route(POST "/folders/pick")]
async fn pick_folder(cx: &Cx, Form(posted): Form<PickStart>) -> Result<Response> {
    let start = start_folder(&posted.start, &expand_home("~"));
    let body = match pick(&start) {
        Picked::Folder(folder) => serde_json::json!({ "path": folder.to_string_lossy() }),
        Picked::Cancelled => serde_json::json!({}),
        Picked::Unavailable => serde_json::json!({ "unavailable": true }),
    };
    (
        [(
            topcoat::router::header::CONTENT_TYPE,
            topcoat::router::HeaderValue::from_static("application/json"),
        )],
        body.to_string(),
    )
        .into_response(cx)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway folder tree under the system temp folder.
    fn tree(name: &str) -> PathBuf {
        let root = crate::fixtures::scratch(&format!("folders-{name}"));
        for dir in ["b", "A", ".git", "c/inner"] {
            std::fs::create_dir_all(root.join(dir)).expect("dir");
        }
        std::fs::write(root.join("file.txt"), "x").expect("file");
        root
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
