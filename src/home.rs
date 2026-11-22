//! The welcome screen: Open Folder… and the saved projects, most recent first.

use rune::{Cursor, El, col, el, row, text};
use rune_kit::{Button, ButtonVariant, Kit};

use crate::App;
use crate::store::Project;

pub const WINDOW_SIZE: (f32, f32) = (820.0, 480.0);

pub fn view(s: &App, k: &Kit) -> El {
    let c = &k.c;
    let intro = col()
        .w(300.0)
        .h_full()
        .padding(32.0)
        .gap(8.0)
        .center()
        .fill(c.surface_base)
        .child(text("Vitrine").font(rune::TextStyle { size: 34.0, weight: 600.0, ..k.ty.ui }).color(c.text_strong))
        .child(text(format!("Version {}", env!("CARGO_PKG_VERSION"))).font(k.ty.small).color(c.text_muted))
        .child(el().h(24.0))
        .child(
            Button::new("Open Folder…")
                .variant(ButtonVariant::Contrast)
                .on_press_tx(|s: &mut App, tx| crate::choose_folder(s, tx))
                .build(k)
                .key("open-folder"),
        );

    let mut list = col().key("projects").w_full().h_full().min_h(0.0).padding(12.0).gap(2.0).scroll(rune::ScrollSpec { stick_bottom: false });
    if s.projects.is_empty() {
        list = list.center().child(text("No saved projects").font(k.ty.ui).color(c.text_faint));
    }
    for (index, project) in s.projects.iter().enumerate() {
        list = list.child(project_row(project, k).key(format!("project-{index}")));
    }

    row().w_full().h_full().fill(c.bg_base).child(intro).child(col().grow(1.0).h_full().min_w(0.0).child(list))
}

fn project_row(project: &Project, k: &Kit) -> El {
    let c = &k.c;
    let path = project.path.clone();
    let available = path.is_dir();
    let details = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(2.0)
        .child(text(project.name()).font(k.ty.ui_medium).color(if available { c.text_base } else { c.text_muted }).truncate())
        .child(text(path.display().to_string()).font(k.ty.small).color(c.text_muted).truncate());
    let mut r = row().w_full().padding(10.0).gap(8.0).center_y().radius(k.radius.md).child(details);
    if available {
        return r.hover(c.surface_hover).cursor(Cursor::Pointer).on_press_tx(move |s: &mut App, tx| crate::open_project(s, path.clone(), tx));
    }
    let locate_old = path.clone();
    r = r
        .child(text("Folder not found").font(k.ty.small).color(c.warning.icon))
        .child(
            Button::new("Locate…")
                .variant(ButtonVariant::Ghost)
                .on_press_tx(move |s: &mut App, tx| crate::locate_project(s, locate_old.clone(), tx))
                .build(k),
        )
        .child(
            Button::new("Remove")
                .variant(ButtonVariant::GhostMuted)
                .on_press(move |s: &mut App| s.projects = crate::store::remove_project(&path))
                .build(k),
        );
    r
}
