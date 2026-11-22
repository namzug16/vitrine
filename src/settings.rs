//! The Settings window (⌘,): reading fonts and size, and what this build is.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use rune::{AppHandle, AppSpec, Cx, El, Family, TextStyle, Tx, Waker, col, row, text};
use rune_kit::{Button, ButtonVariant, Combobox, ComboboxOption, ComboboxState, Kit};

use crate::App;
use crate::store::{self, FONT_SIZE_RANGE, Settings};

pub const WINDOW_SIZE: (f32, f32) = (520.0, 560.0);

pub struct SettingsWindow {
    pub settings: Settings,
    /// Installed font families, filled by a background thread.
    loaded_families: Arc<Mutex<Option<Vec<String>>>>,
    family_options: Vec<ComboboxOption>,
    text_font_picker: ComboboxState,
    code_font_picker: ComboboxState,
    /// The main window, which applies each change.
    pub main_window: Option<AppHandle<App>>,
}

pub fn spec() -> AppSpec<SettingsWindow> {
    AppSpec { setup: Some(setup), poll: Some(poll), ..AppSpec::new("Settings", WINDOW_SIZE, init, view) }
}

fn init() -> SettingsWindow {
    SettingsWindow {
        settings: store::load_settings(),
        loaded_families: Arc::default(),
        family_options: Vec::new(),
        text_font_picker: ComboboxState::default(),
        code_font_picker: ComboboxState::default(),
        main_window: None,
    }
}

fn setup(s: &mut SettingsWindow, waker: Waker) {
    let loaded = s.loaded_families.clone();
    std::thread::spawn(move || {
        let mut fonts = resvg::usvg::fontdb::Database::new();
        fonts.load_system_fonts();
        let families: BTreeSet<String> =
            fonts.faces().filter_map(|face| face.families.first().map(|(name, _)| name.clone())).filter(|name| !name.starts_with('.')).collect();
        *loaded.lock().unwrap() = Some(families.into_iter().collect());
        waker();
    });
}

fn poll(s: &mut SettingsWindow, _: f64, _: &mut Tx) -> bool {
    let Some(families) = s.loaded_families.lock().unwrap().take() else { return false };
    s.family_options = families.into_iter().map(|name| ComboboxOption::new(name.clone(), name)).collect();
    true
}

/// A family name for `Family::Named`, which takes `&'static str`. Each distinct name leaks once.
pub fn intern(name: &str) -> &'static str {
    static NAMES: Mutex<BTreeSet<&'static str>> = Mutex::new(BTreeSet::new());
    let mut names = NAMES.lock().unwrap();
    if let Some(known) = names.get(name) {
        return known;
    }
    let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
    names.insert(leaked);
    leaked
}

/// The mono slot's family list: the chosen code font, then SF Mono when it isn't installed.
pub fn code_families(settings: &Settings) -> &'static str {
    intern(&format!("{}, ui-monospace", settings.code_font_family))
}

/// Saves the settings and hands them to the main window.
fn change(s: &mut SettingsWindow, edit: impl FnOnce(&mut Settings)) {
    edit(&mut s.settings);
    store::save_settings(&s.settings);
    let settings = s.settings.clone();
    if let Some(main) = &s.main_window {
        main.update(move |app: &mut App, tx: &mut Tx| crate::apply_settings(app, settings, tx));
    }
}

pub fn step_font_size(settings: &mut Settings, by: f32) {
    settings.font_size_points = (settings.font_size_points + by).clamp(FONT_SIZE_RANGE.0, FONT_SIZE_RANGE.1);
}

fn field(label: &str, control: El, k: &Kit) -> El {
    row().w_full().gap(12.0).center_y().child(text(label.to_owned()).font(k.ty.ui).color(k.c.text_base).w(120.0)).child(control)
}

fn heading(title: &str, k: &Kit) -> El {
    text(title.to_owned()).font(k.ty.small_medium).color(k.c.text_muted)
}

fn view(s: &SettingsWindow, cx: &mut Cx<SettingsWindow>) -> El {
    let k = Kit::neutral(cx.system_dark());
    let mut layer = Vec::new();
    let loading = s.family_options.is_empty();
    let picker = |key: &str, current: &str, state: &ComboboxState, access: fn(&mut SettingsWindow) -> &mut ComboboxState| {
        Combobox::new(key, state, &s.family_options, access)
            .label(key.to_owned())
            .current(Some(current))
            .placeholder(current.to_owned())
            .search_placeholder("Search fonts")
            .width(260.0)
            .disabled(loading)
    };
    let text_font = picker("Text font", &s.settings.text_font_family, &s.text_font_picker, |s| &mut s.text_font_picker)
        .on_choose(|s: &mut SettingsWindow, family: &str, _| {
            let family = family.to_owned();
            change(s, |settings| settings.text_font_family = family);
        })
        .build(&k, &mut layer);
    let code_font = picker("Code font", &s.settings.code_font_family, &s.code_font_picker, |s| &mut s.code_font_picker)
        .on_choose(|s: &mut SettingsWindow, family: &str, _| {
            let family = family.to_owned();
            change(s, |settings| settings.code_font_family = family);
        })
        .build(&k, &mut layer);
    let size = s.settings.font_size_points;
    let font_size = row()
        .gap(6.0)
        .center_y()
        .child(Button::new("−").variant(ButtonVariant::Outline).disabled(size <= FONT_SIZE_RANGE.0).on_press(|s: &mut SettingsWindow| change(s, |x| step_font_size(x, -1.0))).build(&k).key("smaller"))
        .child(text(format!("{size:.0} pt")).font(k.ty.ui).color(k.c.text_base).w(48.0).align(rune::Align::Center))
        .child(Button::new("+").variant(ButtonVariant::Outline).disabled(size >= FONT_SIZE_RANGE.1).on_press(|s: &mut SettingsWindow| change(s, |x| step_font_size(x, 1.0))).build(&k).key("bigger"));
    let preview = text("The quick brown fox reads the plan.")
        .font(TextStyle { family: Family::Named(intern(&s.settings.text_font_family)), size, line_height: size * 1.6, ..k.ty.body })
        .color(k.c.text_base);

    let about_line = |label: &str, value: String| field(label, text(value).font(k.ty.ui).color(k.c.text_muted).selectable(true), &k);
    let body = col()
        .w_full()
        .h_full()
        .padding(28.0)
        .gap(14.0)
        .fill(k.c.bg_base)
        .child(heading("READING", &k))
        .child(field("Text font", text_font, &k))
        .child(field("Code font", code_font, &k))
        .child(field("Font size", font_size, &k))
        .child(col().w_full().padding(14.0).radius(k.radius.md).fill(k.c.surface_base).child(preview))
        .child(rune::el().h(8.0))
        .child(heading("ABOUT", &k))
        .child(about_line("Version", env!("CARGO_PKG_VERSION").into()))
        .child(about_line("Build", env!("VITRINE_COMMIT").into()))
        .child(about_line("Diagrams", format!("Pele {} (QuickJS, resvg)", crate::diagrams::PELE_VERSION)))
        .child(about_line("Licenses", "Pele MIT · Rune MIT · Lucide ISC".into()));
    rune_kit::root(body, layer)
}
