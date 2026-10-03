// Finding files, keeping backups, writing safely (`private/cobfmt-proposal.md`
// §5, §6). Standard library only: the same on Windows and Linux.

use std::io::Write;
use std::path::{Path, PathBuf};

// The `.cb` files under `dir`, sorted; hidden directories and `target/`
// skipped.
pub fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if p.is_dir() {
            if name.starts_with('.') || name == "target" {
                continue;
            }
            collect(&p, out);
        } else if name.ends_with(".cb") {
            out.push(p);
        }
    }
}

// The first free backup name beside `path`: `NAME.cb.format.bak`, then
// `NAME.cb.format.bak.1`, `.2`, … — an existing one is never overwritten.
pub fn backup_name(path: &Path) -> PathBuf {
    let base = format!("{}.format.bak", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    let dir = path.parent().unwrap_or_else(|| Path::new(""));
    let first = dir.join(&base);
    if !first.exists() {
        return first;
    }
    let mut k = 1u32;
    loop {
        let p = dir.join(format!("{}.{}", base, k));
        if !p.exists() {
            return p;
        }
        k += 1;
    }
}

// Writes the backup and makes sure it is on disk before the file changes.
pub fn backup(path: &Path, contents: &[u8]) -> std::io::Result<PathBuf> {
    let b = backup_name(path);
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&b)?;
    f.write_all(contents)?;
    f.sync_all()?;
    Ok(b)
}

// Replaces `path`'s contents atomically: a temporary file in the same
// directory, given the original's permissions, renamed over it. An
// interrupted run leaves the old file or the new one, never half of each.
pub fn replace(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = dir.join(format!(".{}.cobfmt-{}.tmp", name, std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(contents)?;
        f.sync_all()?;
    }
    if let Ok(meta) = std::fs::metadata(path) {
        let _ = std::fs::set_permissions(&tmp, meta.permissions());
    }
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}
