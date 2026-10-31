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

/// The final component of `path` (the default project name); empty if none.
pub fn dir_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Recursively copies `src` into the existing `dst`, skipping `.git` at any depth, so a cloned
/// template brings no history.
pub fn copy_dir_excluding_git(src: &Path, dst: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" {
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
}
