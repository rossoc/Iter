//! Filesystem scaffolding for `iter init`/`new`/`clone`: turning a
//! `base_path` into an actual directory, and seeding one from another when
//! cloning a project as a template.

use crate::error::Result;
use std::path::{Path, PathBuf};

/// Resolves `path` (relative or absolute) against the current directory
/// into an absolute string suitable for storing as a project's
/// `base_path` -- so it stays valid however/wherever `iter` is next run
/// from, unlike a relative path that only meant something from today's cwd.
pub fn absolute_path(path: &str) -> Result<String> {
    Ok(std::path::absolute(path)?.to_string_lossy().to_string())
}

/// The final component of `path` (e.g. `"foo"` for `"./bar/foo"`), used as
/// the default project name for `iter new`/`init`. Empty if `path` has no
/// final component (e.g. `"/"`).
pub fn dir_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Recursively copies `src`'s contents into `dst` (`dst` must already
/// exist), skipping any `.git` entry at any depth. Used by `iter clone` to
/// seed a new project's `base_path` from another project's, as a template,
/// without dragging that project's git history along.
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
        let tmp = std::env::temp_dir().join(format!(
            "iter-scaffold-test-{}-{}",
            std::process::id(),
            chrono::Local::now().format("%Y%m%d%H%M%S%f")
        ));
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
