//! The reading pane: one Markdown file, read when selected, with its links and local images.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use rune::{Family, Image, ImageCache, TextStyle, Tx, col, el, text};
use rune_kit::markdown::{Markdown, MarkdownStyle};
use rune_kit::{Button, ButtonVariant, Kit};

use crate::App;
use crate::diagrams::{DiagramRequest, Diagrams};
use crate::project::Project;

// shortcut: fixed until the settings window lets the reader pick fonts.
const TEXT_FONT: &str = "Departure Mono";
pub const CODE_FONTS: &str = "Departure Mono, ui-monospace";
const READING_WIDTH: f32 = 760.0;
/// The info word `mermaid` fences get, so rune-kit hands them to `code_frame` like other code.
const DIAGRAM_FENCE: &str = "vitrine-mermaid";

pub enum Document {
    Loaded { source: String, images: HashMap<String, Image> },
    Missing,
    ReadError(String),
}

/// Reads `relative` from the project and starts loading the local images it shows.
pub fn load(root: &Path, relative: &str, cache: &ImageCache) -> Document {
    let path = root.join(relative);
    match std::fs::read(&path) {
        Ok(bytes) => {
            let source = rename_mermaid_fences(&String::from_utf8_lossy(&bytes));
            let folder = path.parent().unwrap_or(root);
            let images = image_sources(&source)
                .into_iter()
                .filter_map(|src| local_path(folder, &src).map(|p| (src, cache.file(p, READING_WIDTH, 2.0))))
                .collect();
            Document::Loaded { source, images }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Document::Missing,
        Err(e) => Document::ReadError(e.to_string()),
    }
}

/// Without its `mermaid` feature rune-kit draws `mermaid` fences as plain code and skips
/// `code_frame`, so give those fences another info word.
fn rename_mermaid_fences(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let marker_len = trimmed.len() - trimmed.trim_start_matches(['`', '~']).len();
        let info = trimmed[marker_len..].trim();
        if marker_len >= 3 && (info == "mermaid" || info.starts_with("mermaid ")) {
            let indent = &line[..line.len() - trimmed.len()];
            out.push_str(&format!("{indent}{}{DIAGRAM_FENCE}{}\n", &trimmed[..marker_len], &info["mermaid".len()..]));
        } else {
            out.push_str(line);
        }
    }
    out
}

fn image_sources(source: &str) -> Vec<String> {
    use pulldown_cmark::{Event, Parser, Tag};
    Parser::new(source)
        .filter_map(|event| match event {
            Event::Start(Tag::Image { dest_url, .. }) => Some(dest_url.into_string()),
            _ => None,
        })
        .collect()
}

/// A link or image target as a file next to the document, or `None` for a URL or anchor.
fn local_path(folder: &Path, target: &str) -> Option<PathBuf> {
    let target = target.split('#').next().unwrap_or_default();
    if target.is_empty() || target.contains("://") || target.starts_with("mailto:") {
        return None;
    }
    let decoded = percent_decode(target);
    Some(normalize(&if decoded.starts_with('/') { PathBuf::from(decoded) } else { folder.join(decoded) }))
}

/// Resolves `.` and `..` without touching the disk, so a missing file still has a path.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn follow_link(s: &mut App, target: &str, tx: &mut Tx) {
    if target.starts_with("http://") || target.starts_with("https://") || target.starts_with("mailto:") {
        let _ = std::process::Command::new("/usr/bin/open").arg(target).spawn();
        return;
    }
    let Some(project) = s.project.as_ref() else { return };
    let Some(selected) = &project.selected_file else { return };
    let folder = project.root.join(selected).parent().map(Path::to_owned).unwrap_or_else(|| project.root.clone());
    let Some(path) = local_path(&folder, target) else { return };
    match path.strip_prefix(&project.root) {
        Ok(relative) if crate::scan::is_markdown(&relative.to_string_lossy()) => {
            let relative = relative.to_string_lossy().into_owned();
            crate::project::open_file(s, &relative, tx);
        }
        // Reveal instead of open: a link to an app or script must not run it.
        _ if path.exists() => {
            let _ = std::process::Command::new("/usr/bin/open").arg("-R").arg(&path).spawn();
        }
        _ => {}
    }
}

fn reading_style(k: &Kit) -> MarkdownStyle {
    let body = TextStyle { family: Family::Named(TEXT_FONT), size: 15.0, line_height: 24.0, ..k.ty.body };
    let heading = |size, line_height| TextStyle { size, line_height, weight: 600.0, ..body };
    MarkdownStyle::from_kit(k)
        .body_font(body)
        .heading_font(1, heading(26.0, 34.0))
        .heading_font(2, heading(20.0, 28.0))
        .heading_font(3, heading(16.0, 24.0))
        .code_block_font(TextStyle { family: Family::Mono, size: 13.0, line_height: 20.0, ..k.ty.mono })
}

/// Draws `mermaid` fences with Pele and leaves other code blocks as they are.
fn diagram_frame(diagrams: Diagrams, dark: bool, k: Kit) -> impl Fn(rune::El, &rune_kit::markdown::CodeFrame<'_>) -> rune::El + 'static {
    move |code, frame| {
        if frame.language != Some(DIAGRAM_FENCE) {
            return code;
        }
        let request = DiagramRequest { source: frame.source.to_owned(), dark, font_family: TEXT_FONT.into(), font_size_points: 15, scale: 2 };
        match diagrams.get(&request, &k) {
            Some(Ok(d)) => el().w_full().max_w(d.width_points).aspect_ratio(d.width_points / d.height_points).pixels(&d.pixels),
            Some(Err(error)) => col()
                .w_full()
                .gap(6.0)
                .child(text(format!("Diagram error: {error}")).font(k.ty.text_small).color(k.c.danger.icon).wrap())
                .child(code),
            None => col().w_full().h(120.0).center().child(text("Drawing diagram…").font(k.ty.text_small).color(k.c.text_faint)),
        }
    }
}

fn message(title: &str, detail: String, k: &Kit) -> rune::El {
    col()
        .w_full()
        .h_full()
        .center()
        .gap(6.0)
        .padding(32.0)
        .child(text(title.to_owned()).font(k.ty.body_medium).color(k.c.text_base))
        .child(text(detail).font(k.ty.ui).color(k.c.text_muted).wrap().max_w(460.0))
}

pub fn view(p: &Project, diagrams: &Diagrams, dark: bool, k: &Kit) -> rune::El {
    if let Some(error) = &p.scan_error {
        return message("Project folder unavailable", format!("{} can't be read: {error}", p.root.display()), k);
    }
    if p.tree.is_empty() {
        return message("No Markdown files found", format!("{} has no .md or .markdown files.", p.root.display()), k);
    }
    let (Some(relative), Some(document)) = (&p.selected_file, &p.document) else {
        return message("Select a Markdown file", "Choose a file in the sidebar to read it.".into(), k);
    };
    match document {
        Document::Missing => message(
            "File no longer available",
            format!("{relative} couldn’t be found in this project. It may have been moved, renamed, or deleted. Select another file or refresh to check again."),
            k,
        ),
        Document::ReadError(error) => message("Can’t read this file", format!("{relative}: {error}"), k).child(
            Button::new("Try Again").variant(ButtonVariant::Outline).on_press_tx(|s: &mut App, tx| crate::project::reload_selected(s, tx)).build(k).key("retry"),
        ),
        Document::Loaded { source, images } => {
            let images = images.clone();
            let markdown = Markdown::new(source.as_str())
                .style(reading_style(k))
                .selectable(true)
                .images(move |src| images.get(src).cloned())
                .on_link_tx(follow_link)
                .code_frame(diagram_frame(diagrams.clone(), dark, k.clone()))
                .build(k);
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            relative.hash(&mut hasher);
            // A new key per file opens each file at its top; the anchor keeps the place on refresh.
            col()
                .key(format!("document-{:x}", hasher.finish()))
                .w_full()
                .h_full()
                .min_h(0.0)
                .scroll(rune::ScrollSpec { stick_bottom: false })
                .scroll_anchor()
                .items_center()
                .child(el().w_full().max_w(READING_WIDTH).padding(32.0).child(markdown))
        }
    }
}
