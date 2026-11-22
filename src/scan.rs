//! Finds the Markdown files in a project folder.

use std::io;
use std::path::Path;

/// Folders never worth scanning: Git internals and installed packages. Hidden folders such as
/// `.claude/` stay, since agents write their artifacts there.
const SKIPPED_FOLDERS: [&str; 2] = [".git", "node_modules"];

/// Project-relative paths (with `/`) of every `.md` and `.markdown` file under `root`.
/// Fails only when `root` itself can't be read; unreadable subfolders are skipped.
pub fn markdown_files(root: &Path) -> io::Result<Vec<String>> {
    let mut found = Vec::new();
    let mut pending = vec![(root.read_dir()?, String::new())];
    while let Some((entries, prefix)) = pending.pop() {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // `file_type` doesn't follow symlinks, so a linked folder can't loop the scan.
            let Ok(kind) = entry.file_type() else { continue };
            let relative = format!("{prefix}{name}");
            if kind.is_dir() {
                if !SKIPPED_FOLDERS.contains(&name.as_str())
                    && let Ok(children) = entry.path().read_dir()
                {
                    pending.push((children, format!("{relative}/")));
                }
            } else if is_markdown(&name) {
                found.push(relative);
            }
        }
    }
    Ok(found)
}

fn is_markdown(name: &str) -> bool {
    Path::new(name).extension().is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
}
