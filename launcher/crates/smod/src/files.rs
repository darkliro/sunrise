//! A mod's `assets/` and `data/` paths and caps, checked as the game's content fence checks them.
use crate::manifest::Manifest;
use std::{io::Read, path::Path};

/// Content files per mod, bytes per file and bytes per mod.
pub const MAX_FILES: usize = 65_536;
pub const MAX_FILE: u64 = 16 << 20;
pub const MAX_TOTAL: u64 = 512 << 20;

/// Checks a content path: `assets/` or `data/`, then `/`-separated parts of
/// `[a-z0-9_.-]`, none of them `.` or `..` (vanilla's resource-location
/// characters, so the path means the same on every OS).
pub fn check_path(name: &str) -> Result<(), String> {
    let bad = || format!("{name:?} is not a plain assets/ or data/ path");
    let mut parts = name.split('/');
    if !matches!(parts.next(), Some("assets" | "data")) {
        return Err(bad());
    }
    let mut count = 0;
    for part in parts {
        count += 1;
        let chars_ok = part
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b));
        if part.is_empty() || part == "." || part == ".." || !chars_ok {
            return Err(bad());
        }
    }
    // assets/<namespace>/<file> at least.
    if count < 2 {
        return Err(bad());
    }
    Ok(())
}

/// The shared resource namespace fence for folders and native archives.
pub(crate) fn check_namespace(name: &str, manifest: &Manifest) -> Result<(), String> {
    let mut parts = name.splitn(3, '/');
    let (top, namespace, rest) = (
        parts.next().unwrap_or(""),
        parts.next().unwrap_or(""),
        parts.next().unwrap_or(""),
    );
    let id = manifest.id.as_str();
    let own = namespace == id;
    let owned = own || manifest.namespaces.iter().any(|n| n == namespace);
    // Vanilla's own files stay vanilla's; a mod may only add to its
    // tags and atlases, and to the convention tags mods share.
    let shared = matches!(namespace, "minecraft" | "c");
    let atlas = top == "assets" && namespace == "minecraft" && rest.starts_with("atlases/");
    let allowed = owned || (top == "data" && shared && rest.starts_with("tags/")) || atlas;
    if !allowed {
        return Err(format!(
            "{name}: a mod adds files in its own namespaces ({top}/{id}/ and those listed in namespaces), to vanilla and convention (c) tags, or to vanilla's atlases"
        ));
    }

    Ok(())
}

/// The mod folder's sorted `assets/` and `data/` files, listed and capped as `Content::read` does.
pub(crate) fn content_files(root: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    let mut total = 0u64;
    for (name, size) in folder_files(root)? {
        check_path(&name)?;
        if size > MAX_FILE {
            return Err(format!("{name} is over {} MiB", MAX_FILE >> 20));
        }
        total += size;
        files.push(name);
    }
    if files.len() > MAX_FILES {
        return Err(format!("more than {MAX_FILES} content files"));
    }
    if total > MAX_TOTAL {
        return Err(format!("content is over {} MiB", MAX_TOTAL >> 20));
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// One content file of the mod folder `root`, read as the game's `FileReader` reads it.
pub(crate) fn read_file(root: &Path, name: &str) -> Result<Vec<u8>, String> {
    check_path(name)?;
    let file = std::fs::File::open(root.join(name)).map_err(|e| format!("{name}: {e}"))?;
    let mut out = Vec::new();
    file.take(MAX_FILE + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("{name}: {e}"))?;
    if out.len() as u64 > MAX_FILE {
        return Err(format!("{name} is over {} MiB", MAX_FILE >> 20));
    }
    Ok(out)
}

/// Files under `dir`, relative to `root` with `/` separators. Symbolic links
/// are not followed: a mod folder holds its own files.
pub(crate) fn folder_files(root: &Path) -> Result<Vec<(String, u64)>, String> {
    let mut out = Vec::new();
    for top in ["assets", "data"] {
        walk(root, &root.join(top), &mut out)?;
    }
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, u64)>) -> Result<(), String> {
    match std::fs::symlink_metadata(dir) {
        Ok(m) if !m.is_dir() => {
            return Err(format!(
                "{}: unsupported file type in a content mod",
                dir.display()
            ));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    }
    let read = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in read {
        let path = entry.map_err(|e| e.to_string())?.path();
        let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if meta.is_dir() {
            walk(root, &path, out)?;
        } else if meta.is_file() {
            let rel = path.strip_prefix(root).map_err(|e| e.to_string())?;
            let parts: Option<Vec<&str>> = rel.iter().map(|p| p.to_str()).collect();
            let name = parts.ok_or_else(|| format!("{} is not UTF-8", rel.display()))?;
            out.push((name.join("/"), meta.len()));
        } else {
            return Err(format!(
                "{}: unsupported file type in a content mod",
                path.display()
            ));
        }
        if out.len() > MAX_FILES {
            return Err(format!("more than {MAX_FILES} content files"));
        }
    }
    Ok(())
}
