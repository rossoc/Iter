//! Filesystem scaffolding for `iter init`/`new`/`clone`: turning a
//! `base_path` into an actual directory, and seeding one from another when
//! cloning a project as a template.

use crate::error::Result;
use std::path::{Path, PathBuf};

/// Resolves `path` against the cwd into an absolute string for `base_path`, so it stays valid
/// wherever `iter` runs next. A leading `~` is expanded first: editor buffers and web forms never
/// saw a shell.
pub fn absolute_path(path: &str) -> Result<String> {
    let path = crate::config::expand_home(path);
    Ok(std::path::absolute(path)?.to_string_lossy().to_string())
}

/// Makes `project`'s `base_path` absolute (and `~` expanded) for every write path: a relative
/// path names a different directory wherever `iter` later runs. An empty one stays empty, for
/// `Project::validate` to refuse.
pub fn absolutise_base_path(project: &mut crate::models::Project) -> Result<()> {
    let path = project.base_path.trim();
    if !path.is_empty() {
        project.base_path = absolute_path(path)?;
    }
    Ok(())
}

/// The final component of `path` (the default project name); empty if none.
pub fn dir_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Folders a cloned template never brings: its history and the task worktrees iter made in it.
const NOT_COPIED: [&str; 2] = [".git", ".iter-worktrees"];

/// Copies the template folder `src` into a new folder `dst`: refused when `dst` is `src` or inside
/// it (the copy would copy itself until the path is too long), checked before anything is created.
pub fn copy_template(src: &Path, dst: &Path) -> Result<()> {
    let from = std::fs::canonicalize(src)?;
    let to = resolved(dst)?;
    if to.starts_with(&from) {
        return Err(crate::error::IterError::CloneIntoSource {
            src: from.display().to_string(),
            dest: to.display().to_string(),
        });
    }
    std::fs::create_dir_all(dst)?;
    copy_dir_excluding_git(src, dst)
}

/// `path` made absolute with its longest existing ancestor canonicalised (symlinks resolved), so it
/// compares with a canonical path even before it exists.
fn resolved(path: &Path) -> Result<PathBuf> {
    let path = std::path::absolute(path)?;
    let mut missing = Vec::new();
    let mut base = path.as_path();
    loop {
        if let Ok(found) = std::fs::canonicalize(base) {
            return Ok(missing.iter().rev().fold(found, |p, part| p.join(part)));
        }
        match (base.parent(), base.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name.to_os_string());
                base = parent;
            }
            _ => return Ok(path),
        }
    }
}

/// Recursively copies `src` into the existing `dst`, skipping `.git` and `.iter-worktrees` at any
/// depth, so a cloned template brings no history and no task worktrees.
pub fn copy_dir_excluding_git(src: &Path, dst: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if NOT_COPIED.iter().any(|n| name == *n) {
            continue;
        }
        let from = entry.path();
        let to = dst.join(&name);
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_dir_excluding_git(&from, &to)?;
        } else if file_type.is_symlink() {
            copy_symlink(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn copy_symlink(from: &Path, to: &Path) -> Result<()> {
    // Recreate the symlink itself rather than following it, so a
    // template's symlinks don't silently turn into copied files.
    let target: PathBuf = std::fs::read_link(from)?;
    std::os::unix::fs::symlink(&target, to)?;
    Ok(())
}

#[cfg(not(unix))]
fn copy_symlink(from: &Path, to: &Path) -> Result<()> {
    std::fs::copy(from, to)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_name_takes_the_final_path_component() {
        assert_eq!(dir_name("./bar/foo"), "foo");
        assert_eq!(dir_name("/abs/path/proj"), "proj");
    }

    #[test]
    fn copy_dir_excluding_git_skips_dot_git_at_any_depth() {
        let tmp = crate::fixtures::scratch("scaffold-copy");
        let src = tmp.join("src");
        let dst = tmp.join("dst");
        std::fs::create_dir_all(src.join(".git")).unwrap();
        std::fs::write(src.join(".git").join("HEAD"), "ref: refs/heads/main").unwrap();
        std::fs::write(src.join("README.md"), "hello").unwrap();
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("sub").join("file.txt"), "content").unwrap();
        std::fs::create_dir_all(&dst).unwrap();

        copy_dir_excluding_git(&src, &dst).unwrap();

        assert!(!dst.join(".git").exists());
        assert_eq!(
            std::fs::read_to_string(dst.join("README.md")).unwrap(),
            "hello"
        );
        assert_eq!(
            std::fs::read_to_string(dst.join("sub").join("file.txt")).unwrap(),
            "content"
        );

        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn copy_template_refuses_a_destination_inside_the_source() {
        let tmp = crate::fixtures::scratch("scaffold-nested");
        let src = tmp.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.txt"), "a").unwrap();

        for dest in [src.join("copy"), src.join("deep").join("copy"), src.clone()] {
            let err = copy_template(&src, &dest).unwrap_err();
            assert!(matches!(err, crate::error::IterError::CloneIntoSource { .. }), "{err}");
        }
        assert!(!src.join("copy").exists() && !src.join("deep").exists());

        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn copy_template_skips_worktrees_and_copies_beside() {
        let tmp = crate::fixtures::scratch("scaffold-beside");
        let src = tmp.join("src");
        std::fs::create_dir_all(src.join(".iter-worktrees").join("t-1")).unwrap();
        std::fs::write(src.join("a.txt"), "a").unwrap();
        // a sibling whose name starts like the source is not inside it
        let dest = tmp.join("src-copy");

        copy_template(&src, &dest).unwrap();

        assert_eq!(std::fs::read_to_string(dest.join("a.txt")).unwrap(), "a");
        assert!(!dest.join(".iter-worktrees").exists());

        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
