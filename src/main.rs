mod diagrams;
mod git;
mod home;
mod icons;
mod project;
mod reader;
mod scan;
mod settings;
mod store;
mod viewer;

use std::path::PathBuf;

use rune::{AppHandle, AppSpec, Chrome, Cx, El, FileDialogOptions, ImageCache, KeyEvent, MenuItem, Tx, Waker, WindowOptions, WindowSpec};
use rune_kit::Kit;

use project::Project;
use settings::SettingsWindow;
use store::{SavedProject, Settings};

const PROJECT_WINDOW_SIZE: (f32, f32) = (1200.0, 780.0);

struct App {
    projects: Vec<SavedProject>,
    project: Option<Project>,
    image_cache: Option<ImageCache>,
    diagrams: Option<diagrams::Diagrams>,
    settings: Settings,
    settings_window: AppHandle<SettingsWindow>,
}

/// Takes settings changed in the Settings window.
fn apply_settings(s: &mut App, settings: Settings, tx: &mut Tx) {
    tx.set_font_families("Inter", settings::code_families(&settings));
    s.settings = settings;
}

fn open_settings(s: &mut App, _: &mut Tx) {
    s.settings_window.open();
}

fn setup(s: &mut App, waker: Waker) {
    s.image_cache = Some(ImageCache::new(waker.clone()));
    s.diagrams = Some(diagrams::Diagrams::start(waker));
}

/// Rebuilds the view when a diagram finished drawing on its thread.
fn poll(s: &mut App, _: f64, _: &mut Tx) -> bool {
    s.diagrams.as_ref().is_some_and(diagrams::Diagrams::take_changed)
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
    if ev.key == rune::Key::Escape && s.project.as_ref().is_some_and(|p| p.viewer.is_some()) {
        viewer::close(s);
        return true;
    }
    // The View menu takes ⌘R first in a window; this catches it where no menu does (headless).
    if ev.is_command('r') && s.project.is_some() {
        project::refresh(s, tx);
        return true;
    }
    // ⌘W leaves a project for the welcome screen; on the welcome screen it closes the window.
    if ev.is_command('w') && s.project.is_some() {
        close_project(s, tx);
        return true;
    }
    false
}

fn main() {
    rune::prewarm_gpu();
    let settings = store::load_settings();
    let mut app = rune::App::new(AppSpec {
        on_key: Some(on_key),
        setup: Some(setup),
        poll: Some(poll),
        families: ("Inter", settings::code_families(&settings)),
        ..AppSpec::new("Vitrine", home::WINDOW_SIZE, || unreachable!("App::init builds the state"), view)
    })
    // Full-size content so the project's split view draws its own toolbar band.
    .chrome(Chrome { full_size_content: true, traffic_lights: None, drag_regions: true })
    .menu_item(MenuItem::new("Settings…", open_settings).key_equivalent("cmd+,".parse().expect("valid shortcut")))
    .menu_item(MenuItem::new("Open Folder…", choose_folder).menu("File").key_equivalent("cmd+o".parse().expect("valid shortcut")))
    .menu_item(MenuItem::new("Refresh", project::refresh).menu("View").key_equivalent("cmd+r".parse().expect("valid shortcut")));
    let hidden = WindowOptions { visible: false, ..Default::default() };
    let (settings_window, _) = app.window(WindowSpec::new("settings", settings::spec()).options(hidden));
    let main_window = app.handle();
    settings_window.update(move |ui: &mut SettingsWindow, _| ui.main_window = Some(main_window));
    let app = app.init(move || App {
        projects: store::load_projects(),
        project: None,
        image_cache: None,
        diagrams: None,
        settings,
        settings_window,
    });
    app.run();
}
