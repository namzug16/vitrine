//! Saved projects, as JSON in `~/Library/Application Support/Vitrine`.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub path: PathBuf,
    pub last_opened_unix_seconds: u64,
}

impl Project {
    pub fn name(&self) -> String {
        self.path.file_name().map_or_else(|| self.path.display().to_string(), |n| n.to_string_lossy().into_owned())
    }
}

fn storage_dir() -> PathBuf {
    let home = std::env::var_os("HOME").expect("HOME is set");
    PathBuf::from(home).join("Library/Application Support/Vitrine")
}

/// Projects, most recently opened first.
pub fn load_projects() -> Vec<Project> {
    let mut projects: Vec<Project> = load("projects.json");
    projects.sort_by_key(|p| std::cmp::Reverse(p.last_opened_unix_seconds));
    projects
}

/// Saves `path` as opened now, updating its entry if it has one, and returns the new list.
pub fn record_project_opened(path: &Path) -> Vec<Project> {
    let mut projects = load_projects();
    projects.retain(|p| p.path != path);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    projects.insert(0, Project { path: path.to_owned(), last_opened_unix_seconds: now });
    save("projects.json", &projects);
    projects
}

/// Forgets a saved project. The folder itself is untouched.
pub fn remove_project(path: &Path) -> Vec<Project> {
    let mut projects = load_projects();
    projects.retain(|p| p.path != path);
    save("projects.json", &projects);
    projects
}

fn load<T: DeserializeOwned + Default>(file: &str) -> T {
    let path = storage_dir().join(file);
    let Ok(bytes) = std::fs::read(&path) else { return T::default() };
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        // Keep the unreadable file so the next save doesn't destroy what the user had.
        let backup = path.with_extension("json.bad");
        eprintln!("vitrine: {} is invalid ({e}); moved to {}", path.display(), backup.display());
        let _ = std::fs::rename(&path, &backup);
        T::default()
    })
}

fn save<T: Serialize + ?Sized>(file: &str, value: &T) {
    let dir = storage_dir();
    let path = dir.join(file);
    let temporary = dir.join(format!(".{file}.tmp"));
    let result = std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(&temporary, serde_json::to_vec_pretty(value).expect("serializable")))
        .and_then(|()| std::fs::rename(&temporary, &path));
    if let Err(e) = result {
        eprintln!("vitrine: could not save {}: {e}", path.display());
    }
}
