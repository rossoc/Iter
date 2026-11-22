//! The system's folder picker (`osascript` on macOS, `zenity` elsewhere): a page cannot learn a picked folder's path, but the server runs on the user's machine (loopback only, `guard.rs`).

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, PartialEq)]
pub enum Picked {
    Folder(PathBuf),
    Cancelled,
    Unavailable,
}

pub fn pick(start: &Path) -> Picked {
    let run = if cfg!(target_os = "macos") {
        // the folder goes in as an argument, never into the script's text
        Command::new("osascript")
            .args([
                "-e",
                "on run argv",
                "-e",
                "POSIX path of (choose folder with prompt \"Choose the project's folder\" default location (POSIX file (item 1 of argv)))",
                "-e",
                "end run",
            ])
            .arg(start)
            .output()
    } else {
        // zenity starts inside a folder given with a trailing slash
        let mut at = start.as_os_str().to_owned();
        at.push("/");
        let mut filename = std::ffi::OsString::from("--filename=");
        filename.push(at);
        Command::new("zenity")
            .args([
                "--file-selection",
                "--directory",
                "--title=Choose the project's folder",
            ])
            .arg(filename)
            .output()
    };
    match run {
        Err(_) => Picked::Unavailable,
        // both exit with 1 when the user cancels
        Ok(out) if !out.status.success() => Picked::Cancelled,
        Ok(out) => match chosen(&String::from_utf8_lossy(&out.stdout)) {
            Some(path) => Picked::Folder(path),
            None => Picked::Cancelled,
        },
    }
}

fn chosen(output: &str) -> Option<PathBuf> {
    let line = output.trim_end_matches(['\n', '\r']);
    if line.is_empty() {
        return None;
    }
    let trimmed = line.trim_end_matches('/');
    Some(PathBuf::from(if trimmed.is_empty() {
        "/"
    } else {
        trimmed
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_is_read_without_its_trailing_slash() {
        assert_eq!(
            chosen("/Users/me/src/app/\n"),
            Some("/Users/me/src/app".into())
        );
        assert_eq!(chosen("/home/me/app\n"), Some("/home/me/app".into()));
        assert_eq!(chosen("/\n"), Some("/".into()));
        assert_eq!(chosen("\n"), None);
    }
}
