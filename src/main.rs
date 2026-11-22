mod home;
mod store;

use std::path::PathBuf;

use rune::{AppSpec, Cx, El, FileDialogOptions, KeyEvent, MenuItem, Tx, col, text};
use rune_kit::Kit;

use store::Project;

const PROJECT_WINDOW_SIZE: (f32, f32) = (1200.0, 780.0);

struct App {
    projects: Vec<Project>,
    open_project: Option<PathBuf>,
}

fn init() -> App {
    App { projects: store::load_projects(), open_project: None }
}

fn view(s: &App, cx: &mut Cx<App>) -> El {
    let k = Kit::neutral(cx.system_dark());
    match &s.open_project {
        None => home::view(s, &k),
        Some(path) => col().w_full().h_full().center().fill(k.c.bg_base).child(text(path.display().to_string()).color(k.c.text_base)),
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
    tx.set_title(path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned()));
    s.open_project = Some(path);
    tx.resize_window(PROJECT_WINDOW_SIZE);
}

fn close_project(s: &mut App, tx: &mut Tx) {
    s.open_project = None;
    s.projects = store::load_projects();
    tx.set_title("Vitrine");
    tx.resize_window(home::WINDOW_SIZE);
}

fn on_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    // ⌘W leaves a project for the welcome screen; on the welcome screen it closes the window.
    if ev.is_command('w') && s.open_project.is_some() {
        close_project(s, tx);
        return true;
    }
    false
}

fn main() {
    rune::prewarm_gpu();
    rune::App::new(AppSpec { on_key: Some(on_key), ..AppSpec::new("Vitrine", home::WINDOW_SIZE, init, view) })
        .menu_item(MenuItem::new("Open Folder…", choose_folder).menu("File").key_equivalent("cmd+o".parse().expect("valid shortcut")))
        .run();
}
