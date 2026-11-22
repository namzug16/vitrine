//! Mermaid diagrams drawn by Pele without a browser: QuickJS runs `pele.global.js` on a worker
//! thread, and resvg rasterises its SVG with the system's fonts.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use resvg::{tiny_skia, usvg};
use rune::{Pixels, Waker};
use rune_kit::Kit;

const PELE_JS: &str = include_str!("../assets/pele/pele.global.js");
pub const PELE_VERSION: &str = "1.13.0";

/// What a diagram is drawn for: its source, and the look it takes from the reading pane.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DiagramRequest {
    pub source: String,
    pub dark: bool,
    pub font_family: String,
    pub font_size_points: u32,
    /// Device pixels per point the bitmap is drawn at.
    pub scale: u32,
}

pub struct Diagram {
    pub pixels: Pixels,
    pub width_points: f32,
    pub height_points: f32,
}

pub type Rendered = Result<Arc<Diagram>, String>;

struct Shared {
    // shortcut: results are kept for the whole session; evict when large documents make it matter.
    results: Mutex<HashMap<DiagramRequest, Rendered>>,
    requested: Mutex<std::collections::HashSet<DiagramRequest>>,
    changed: AtomicBool,
}

#[derive(Clone)]
pub struct Diagrams {
    shared: Arc<Shared>,
    jobs: Sender<(DiagramRequest, Theme)>,
}

type Theme = HashMap<&'static str, String>;

impl Diagrams {
    pub fn start(waker: Waker) -> Diagrams {
        let shared = Arc::new(Shared { results: Mutex::default(), requested: Mutex::default(), changed: AtomicBool::new(false) });
        let (jobs, inbox) = channel();
        let worker_shared = shared.clone();
        std::thread::Builder::new()
            .name("pele".into())
            .spawn(move || work(&inbox, &worker_shared, &waker))
            .expect("start diagram thread");
        Diagrams { shared, jobs }
    }

    /// The drawn diagram, or `None` while it is being drawn (asking starts it).
    pub fn get(&self, request: &DiagramRequest, k: &Kit) -> Option<Rendered> {
        if let Some(done) = self.shared.results.lock().unwrap().get(request) {
            return Some(done.clone());
        }
        if self.shared.requested.lock().unwrap().insert(request.clone()) {
            let _ = self.jobs.send((request.clone(), theme(k, &request.font_family)));
        }
        None
    }

    /// Whether a diagram finished since the last call.
    pub fn take_changed(&self) -> bool {
        self.shared.changed.swap(false, Ordering::AcqRel)
    }
}

fn work(inbox: &Receiver<(DiagramRequest, Theme)>, shared: &Shared, waker: &Waker) {
    let mut fonts = usvg::fontdb::Database::new();
    fonts.load_system_fonts();
    let fonts = Arc::new(fonts);
    let runtime = rquickjs::Runtime::new().expect("QuickJS runtime");
    let context = rquickjs::Context::full(&runtime).expect("QuickJS context");
    context.with(|ctx| ctx.eval::<(), _>(PELE_JS)).expect("pele.global.js loads");
    let mut measured_family = None;

    for (request, theme) in inbox {
        if measured_family.as_ref() != Some(&request.font_family) {
            install_measurer(&context, &fonts, &request.font_family);
            measured_family = Some(request.font_family.clone());
        }
        let rendered = render_svg(&context, &request)
            .map(|svg| resolve_css_vars(&svg, &theme))
            .and_then(|svg| rasterise(&svg, &fonts, &request))
            .map(Arc::new);
        shared.results.lock().unwrap().insert(request, rendered);
        shared.changed.store(true, Ordering::Release);
        waker();
    }
}

/// Gives Pele the real advance widths of `family`, so labels fit the boxes resvg draws them in.
/// Without the font, Pele keeps its built-in metrics.
fn install_measurer(context: &rquickjs::Context, fonts: &usvg::fontdb::Database, family: &str) {
    let face = fonts
        .query(&usvg::fontdb::Query { families: &[usvg::fontdb::Family::Name(family)], ..Default::default() })
        .and_then(|id| fonts.with_face_data(id, |data, index| (data.to_vec(), index)));
    context.with(|ctx| {
        let globals = ctx.globals();
        let Some((data, index)) = face else {
            globals.set("__measure", rquickjs::Undefined).expect("clear measurer");
            return;
        };
        let measure = rquickjs::Function::new(ctx.clone(), move |label: String, size: f64, _style: i32| -> f64 {
            let Ok(face) = ttf_parser::Face::parse(&data, index) else { return label.chars().count() as f64 * size * 0.6 };
            let advance: u32 = label.chars().map(|c| face.glyph_index(c).and_then(|g| face.glyph_hor_advance(g)).unwrap_or(0) as u32).sum();
            advance as f64 / face.units_per_em() as f64 * size
        })
        .expect("measure function");
        globals.set("__measure", measure).expect("set measurer");
    });
}

fn render_svg(context: &rquickjs::Context, request: &DiagramRequest) -> Result<String, String> {
    context.with(|ctx| {
        let options = rquickjs::Object::new(ctx.clone()).map_err(|e| e.to_string())?;
        options.set("fontFamily", request.font_family.as_str()).map_err(|e| e.to_string())?;
        options.set("fontFamilyMono", request.font_family.as_str()).map_err(|e| e.to_string())?;
        options.set("fontSize", request.font_size_points).map_err(|e| e.to_string())?;
        let render: rquickjs::Function = ctx
            .eval(
                "(src, options) => { if (globalThis.__measure) options.measurer = { width: (t, s, st) => __measure(t, s, st) };
                 try { return { svg: pele.render(src, options).svg }; } catch (e) { return { error: String(e && e.message || e) }; } }",
            )
            .map_err(|e| e.to_string())?;
        let result: rquickjs::Object = render.call((request.source.as_str(), options)).map_err(|e| e.to_string())?;
        match result.get::<_, Option<String>>("svg").map_err(|e| e.to_string())? {
            Some(svg) => Ok(svg),
            None => Err(result.get::<_, String>("error").unwrap_or_else(|_| "Pele failed".into())),
        }
    })
}

fn theme(k: &Kit, font_family: &str) -> Theme {
    let c = &k.c;
    HashMap::from([
        ("--pele-bg", c.bg_base.hex()),
        ("--pele-fg", c.text_base.hex()),
        ("--pele-muted", c.text_muted.hex()),
        ("--pele-line", c.text_muted.hex()),
        ("--pele-surface", c.surface_base.hex()),
        ("--pele-surface-alt", c.surface_raised.hex()),
        ("--pele-border", c.border_strong.hex()),
        ("--pele-accent", c.text_accent.hex()),
        ("--pele-error", c.danger.icon.hex()),
        ("--pele-critical", c.danger.icon.hex()),
        ("--pele-font", format!("'{font_family}'")),
        ("--pele-font-mono", format!("'{font_family}'")),
    ])
}

/// resvg has no CSS custom properties. Pele's root `style` maps `--pele-*` onto its own `--_*`
/// names and the shapes use those, so resolve the theme first, then the SVG's own declarations.
fn resolve_css_vars(svg: &str, theme: &Theme) -> String {
    let themed = substitute_vars(svg, &|name| theme.get(name).cloned(), true);
    let mut declared = HashMap::new();
    for chunk in themed.split(['"', ';']) {
        if let Some((name, value)) = chunk.trim().split_once(':')
            && name.starts_with("--")
        {
            declared.insert(name.to_owned(), value.trim().to_owned());
        }
    }
    let resolved = substitute_vars(&themed, &|name| declared.get(name).cloned(), false);
    // resvg skips the CSS `font` shorthand Pele sets on the root, so copy its size over.
    let size = resolved.split_once(";font:").and_then(|(_, rest)| rest.split_once("px")).map_or("16", |(n, _)| n);
    resolved.replacen("<svg ", &format!("<svg font-size=\"{size}\" "), 1)
}

/// Replaces each `var(--name, fallback)` with `lookup(name)`, else its fallback, else (unless
/// `keep_unknown`) `none`.
fn substitute_vars(svg: &str, lookup: &dyn Fn(&str) -> Option<String>, keep_unknown: bool) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(start) = rest.find("var(") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 4..];
        let mut depth = 1;
        let end = after.char_indices().find_map(|(i, ch)| {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(i)
        });
        let Some(end) = end else {
            out.push_str(&rest[start..]);
            return out;
        };
        let inner = &after[..end];
        let (name, fallback) = inner.split_once(',').map_or((inner.trim(), None), |(n, f)| (n.trim(), Some(f.trim())));
        match lookup(name).or_else(|| fallback.map(|f| substitute_vars(f, lookup, keep_unknown))) {
            Some(value) => out.push_str(&value),
            None if keep_unknown => out.push_str(&rest[start..start + 4 + end + 1]),
            None => out.push_str("none"),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn rasterise(svg: &str, fonts: &Arc<usvg::fontdb::Database>, request: &DiagramRequest) -> Result<Diagram, String> {
    let options = usvg::Options { fontdb: fonts.clone(), font_family: request.font_family.clone(), ..Default::default() };
    let tree = usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    let size = tree.size();
    let scale = request.scale as f32;
    let mut pixmap = tiny_skia::Pixmap::new((size.width() * scale).ceil() as u32, (size.height() * scale).ceil() as u32).ok_or("The diagram is empty")?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let rgba = pixmap.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    });
    let pixels = Pixels::new();
    pixels.set(pixmap.width(), pixmap.height(), rgba.collect());
    Ok(Diagram { pixels, width_points: size.width(), height_points: size.height() })
}
