mod home;
mod icons;
mod project;
mod scan;
mod store;

use std::path::PathBuf;

use rune::{AppSpec, Chrome, Cx, El, FileDialogOptions, KeyEvent, MenuItem, Tx};
use rune_kit::Kit;

use project::Project;
use store::SavedProject;

const PROJECT_WINDOW_SIZE: (f32, f32) = (1200.0, 780.0);

struct App {
    projects: Vec<SavedProject>,
    project: Option<Project>,
}

fn init() -> App {
    App { projects: store::load_projects(), project: None }
}

fn view(s: &App, cx: &mut Cx<App>) -> El {
    let k = Kit::neutral(cx.system_dark());
    match &s.project {
        None => home::view(s, &k),
        Some(_) => project::view(s, &k, cx),
    }
}

fn choose_folder(_: &mut App, tx: &mut Tx) {
    let options = FileDialogOptions { title: "Open Folder".into(), ..Default::default() };
    tx.open_folder(options, |s: &mut App, result, tx| match result {
        Ok(Some(path)) => open_project(s, path, tx),
        Ok(None) => {}
        Err(e) => eprintln!("vitrine: folder dialog failed: {e}"),
    });
}

/// Asks for the new place of a saved project whose folder is gone, and replaces its entry.
fn locate_project(_: &mut App, old_path: PathBuf, tx: &mut Tx) {
    let options = FileDialogOptions { title: format!("Locate “{}”", old_path.display()), ..Default::default() };
    tx.open_folder(options, move |s: &mut App, result, tx| {
        if let Ok(Some(path)) = result {
            store::remove_project(&old_path);
            open_project(s, path, tx);
        }
    });
}

fn open_project(s: &mut App, path: PathBuf, tx: &mut Tx) {
    s.projects = store::record_project_opened(&path);
    let project = Project::open(path);
    tx.set_title(project.name());
    s.project = Some(project);
    tx.resize_window(PROJECT_WINDOW_SIZE);
}

fn close_project(s: &mut App, tx: &mut Tx) {
    s.project = None;
    s.projects = store::load_projects();
    tx.set_title("Vitrine");
    tx.resize_window(home::WINDOW_SIZE);
}

fn on_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    // ⌘W leaves a project for the welcome screen; on the welcome screen it closes the window.
    if ev.is_command('w') && s.project.is_some() {
        close_project(s, tx);
        return true;
    }
    false
}

fn main() {
    rune::prewarm_gpu();
    rune::App::new(AppSpec { on_key: Some(on_key), ..AppSpec::new("Vitrine", home::WINDOW_SIZE, init, view) })
        // Full-size content so the project's split view draws its own toolbar band.
        .chrome(Chrome { full_size_content: true, traffic_lights: None, drag_regions: true })
        .menu_item(MenuItem::new("Open Folder…", choose_folder).menu("File").key_equivalent("cmd+o".parse().expect("valid shortcut")))
        .run();
}
