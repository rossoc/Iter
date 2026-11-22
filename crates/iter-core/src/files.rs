//! Filesystem helpers shared across modules.

use std::path::Path;

/// Writes through a uniquely named file beside `target`, then renames it over.
pub(crate) fn write_whole(dir: &Path, target: &Path, text: &str) -> std::io::Result<()> {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!(".write.{}.{n}.tmp", std::process::id()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, target).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// [`write_whole`] for any `target`: a crash or a concurrent reader sees the old or the new text, never half.
pub fn write_atomic(target: &Path, text: &str) -> std::io::Result<()> {
    let dir = target.parent().filter(|d| !d.as_os_str().is_empty());
    write_whole(dir.unwrap_or(Path::new(".")), target, text)
}

/// Opens `path` for writing, created or truncated, readable by the user alone (`0600`; also when the
/// file already existed). For what iter writes that holds prompts, agent output or keys' surroundings:
/// a shared host's other users must not read it.
pub(crate) fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    let file = private_options().truncate(true).open(path)?;
    restrict(&file)?;
    Ok(file)
}

/// [`create_private`] and write `text`.
pub(crate) fn write_private(path: &Path, text: impl AsRef<str>) -> std::io::Result<()> {
    use std::io::Write;
    create_private(path)?.write_all(text.as_ref().as_bytes())
}

/// A new editor buffer holding `text` in the temp folder: a name no other file has (`create_new`,
/// so a planted file or symlink of that name is never followed), readable by the user alone.
pub fn temp_buffer(text: &str) -> std::io::Result<std::path::PathBuf> {
    use std::io::Write;
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    loop {
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "iter-edit-{}-{}-{n}.md",
            std::process::id(),
            chrono::Local::now().format("%Y%m%d%H%M%S%f")
        ));
        match private_options().create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(text.as_bytes())?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}

/// Makes `path` readable by the user alone, if it exists (the database, once opened).
pub(crate) fn make_private(path: &Path) -> std::io::Result<()> {
    match std::fs::File::open(path) {
        Ok(file) => restrict(&file),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn private_options() -> std::fs::OpenOptions {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options
}

#[cfg(unix)]
fn restrict(file: &std::fs::File) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict(_: &std::fs::File) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn mode(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn private_files_are_the_users_alone_even_when_they_existed() {
        let dir = crate::fixtures::scratch("fs-private");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("run.log");
        std::fs::write(&path, "old, world-readable").unwrap();
        write_private(&path, "new").unwrap();
        assert_eq!(
            (
                std::fs::read_to_string(&path).unwrap().as_str(),
                mode(&path)
            ),
            ("new", 0o600)
        );

        let buffer = temp_buffer("draft").unwrap();
        assert_eq!(
            (
                std::fs::read_to_string(&buffer).unwrap().as_str(),
                mode(&buffer)
            ),
            ("draft", 0o600)
        );
        assert_ne!(
            temp_buffer("x").unwrap(),
            buffer,
            "each buffer is a new file"
        );
        std::fs::remove_file(&buffer).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn write_atomic_replaces_the_file_and_leaves_no_temp() {
        let dir = std::env::temp_dir().join(format!("iter-fs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("a.md");
        write_atomic(&target, "one").unwrap();
        write_atomic(&target, "two").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "two");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
