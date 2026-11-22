//! Read-only Git facts for the status bar, from the `git` command line.

use std::path::Path;
use std::process::Command;

const GIT: &str = "/usr/bin/git";

pub struct GitInfo {
    /// The branch name, or `detached at <short hash>`.
    pub head: String,
    pub file: FileStatus,
    /// Stash references (`stash@{0}`) whose snapshot includes the file's path.
    pub stashes_with_file: Vec<String>,
}

#[derive(Default)]
pub struct FileStatus {
    pub untracked: bool,
    pub ignored: bool,
    pub conflicted: bool,
    /// Changes in the index.
    pub staged: bool,
    /// Changes in the working tree that aren't staged.
    pub modified: bool,
}

/// `None` outside a Git repository, or when Git isn't installed.
pub fn inspect(project_root: &Path, file: Option<&Path>) -> Option<GitInfo> {
    let toplevel = git(project_root, &["rev-parse", "--show-toplevel"])?;
    let toplevel = Path::new(toplevel.trim()).canonicalize().ok()?;
    // Paths relative to the repository; canonical so /tmp and /private/tmp agree.
    let repo_path = file.and_then(|f| {
        let parent = f.parent()?.canonicalize().ok()?;
        Some(parent.join(f.file_name()?).strip_prefix(&toplevel).ok()?.to_string_lossy().into_owned())
    });

    // Untracked and ignored listings only for the one file; for the whole repo they'd walk every
    // ignored folder just to read the branch.
    let args = match &repo_path {
        Some(path) => vec!["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=all", "--ignored=matching", "--", path.as_str()],
        None => vec!["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=no"],
    };
    let status = git(&toplevel, &args)?;
    let (head, file) = parse_status(&status);

    let stashes_with_file = match &repo_path {
        Some(path) => git(&toplevel, &["stash", "list", "-z", "--format=%gd"])
            .unwrap_or_default()
            .split('\0')
            .filter(|stash| !stash.is_empty())
            .filter(|stash| {
                git(&toplevel, &["stash", "show", "--include-untracked", "--name-only", "-z", stash])
                    .is_some_and(|names| names.split('\0').any(|name| name == path))
            })
            .map(str::to_owned)
            .collect(),
        None => Vec::new(),
    };
    Some(GitInfo { head, file, stashes_with_file })
}

/// Runs a read-only Git command, `None` when it fails.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(GIT)
        .arg("-C")
        .arg(dir)
        .args(args)
        // Keeps `git status` from refreshing the index, so inspecting never writes to the repo.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Reads the branch headers and the selected file's entry from `status --porcelain=v2 -z`.
fn parse_status(status: &str) -> (String, FileStatus) {
    let (mut branch, mut oid) = (String::new(), String::new());
    let mut file = FileStatus::default();
    let mut records = status.split('\0');
    while let Some(record) = records.next() {
        let mut fields = record.splitn(3, ' ');
        match (fields.next(), fields.next()) {
            (Some("#"), Some("branch.head")) => branch = fields.next().unwrap_or_default().to_owned(),
            (Some("#"), Some("branch.oid")) => oid = fields.next().unwrap_or_default().to_owned(),
            (Some("?"), _) => file.untracked = true,
            (Some("!"), _) => file.ignored = true,
            (Some("u"), _) => file.conflicted = true,
            (Some(kind @ ("1" | "2")), Some(xy)) => {
                let mut codes = xy.chars();
                file.staged = codes.next().is_some_and(|c| c != '.');
                file.modified = codes.next().is_some_and(|c| c != '.');
                // A rename's record is followed by its original path.
                if kind == "2" {
                    records.next();
                }
            }
            _ => {}
        }
    }
    let head = if branch == "(detached)" { format!("detached at {}", oid.get(..7).unwrap_or(&oid)) } else { branch };
    (head, file)
}
