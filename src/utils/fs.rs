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
pub(crate) fn write_atomic(target: &Path, text: &str) -> std::io::Result<()> {
    let dir = target.parent().filter(|d| !d.as_os_str().is_empty());
    write_whole(dir.unwrap_or(Path::new(".")), target, text)
}

#[cfg(test)]
mod tests {
    use super::*;

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
