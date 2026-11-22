//! Renderer spike: the same Markdown drawn twice, with rune-mermaid (left) and headless Pele
//! (right: QuickJS runs pele.global.js, resvg rasterises its SVG with system fonts).
//! `cargo run --example renderer_spike -- --snap spike.png --size 1600x4200`

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use rune::{AppSpec, Cx, El, Image, ImageCache, Pixels, ScrollSpec, Waker, col, el, row, text};
use rune_kit::Kit;
use rune_kit::markdown::Markdown;
use resvg::{tiny_skia, usvg};

const SAMPLE: &str = include_str!("spike/sample.md");
const PELE_JS: &str = include_str!("spike/pele.global.js");
const FONT: &str = "Departure Mono";
const SPIKE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/spike");

enum Diagram {
    Drawn { pixels: Pixels, width: f32, height: f32 },
    Failed(String),
}

#[derive(Default)]
struct App {
    image: Option<Image>,
    // Pele output keyed by fence source, rendered once for one theme.
    pele: Rc<HashMap<String, Diagram>>,
}

fn setup(s: &mut App, waker: Waker) {
    let cache = ImageCache::new(waker);
    let image = cache.file(format!("{SPIKE_DIR}/tree.png"), 520.0, 2.0);
    let _ = image.wait();
    s.image = Some(image);
    s.pele = Rc::new(render_all_with_pele(&Kit::neutral(true)));
}

fn mermaid_fences(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Option<String> = None;
    for line in src.lines() {
        match (&mut current, line.trim_end()) {
            (None, "```mermaid") => current = Some(String::new()),
            (Some(body), "```") => {
                out.push(std::mem::take(body));
                current = None;
            }
            (Some(body), l) => {
                body.push_str(l);
                body.push('\n');
            }
            _ => {}
        }
    }
    out
}

fn render_all_with_pele(k: &Kit) -> HashMap<String, Diagram> {
    let mut fonts = usvg::fontdb::Database::new();
    fonts.load_system_fonts();
    let face_bytes = fonts
        .query(&usvg::fontdb::Query { families: &[usvg::fontdb::Family::Name(FONT)], ..Default::default() })
        .and_then(|id| fonts.with_face_data(id, |data, index| (data.to_vec(), index)))
        .expect("Departure Mono is installed");
    let fonts = Arc::new(fonts);

    let runtime = rquickjs::Runtime::new().expect("quickjs runtime");
    let context = rquickjs::Context::full(&runtime).expect("quickjs context");
    let theme = theme_colors(k);
    let started = std::time::Instant::now();
    context.with(|ctx| {
        ctx.eval::<(), _>(PELE_JS).expect("pele loads");
        let (data, index) = face_bytes.clone();
        let measure = rquickjs::Function::new(ctx.clone(), move |label: String, size: f64, _style: i32| -> f64 {
            let face = ttf_parser::Face::parse(&data, index).expect("font parses");
            let advance: u32 = label.chars().map(|c| face.glyph_index(c).and_then(|g| face.glyph_hor_advance(g)).unwrap_or(0) as u32).sum();
            advance as f64 / face.units_per_em() as f64 * size
        })
        .expect("measure fn");
        ctx.globals().set("__measure", measure).expect("set measure");
        ctx.eval::<(), _>(format!(
            "globalThis.__render = (src) => {{ try {{ const r = pele.render(src, {{ fontFamily: '{FONT}', fontFamilyMono: '{FONT}', measurer: {{ width: (t, s, st) => __measure(t, s, st) }} }}); return {{ ok: true, svg: r.svg }}; }} catch (e) {{ return {{ ok: false, error: String(e && e.message || e) }}; }} }};"
        ))
        .expect("render fn");
    });
    eprintln!("pele: loaded in {:?}", started.elapsed());

    let mut out = HashMap::new();
    for source in mermaid_fences(SAMPLE) {
        let started = std::time::Instant::now();
        let result: Result<String, String> = context.with(|ctx| {
            let render: rquickjs::Function = ctx.globals().get("__render").expect("render fn");
            let o: rquickjs::Object = render.call((source.clone(),)).expect("render call");
            if o.get::<_, bool>("ok").unwrap_or(false) { Ok(o.get("svg").expect("svg")) } else { Err(o.get("error").unwrap_or_default()) }
        });
        let diagram = match result {
            Ok(svg) => {
                let svg = resolve_css_vars(&svg, &theme);
                if std::env::var_os("SPIKE_DUMP").is_some() {
                    let n = out.len();
                    std::fs::write(format!("{}/pele-{n}.svg", std::env::var("SPIKE_DUMP").unwrap()), &svg).ok();
                }
                rasterise(&svg, &fonts).unwrap_or_else(Diagram::Failed)
            }
            Err(e) => Diagram::Failed(e),
        };
        eprintln!("pele: diagram in {:?}", started.elapsed());
        out.insert(fence_key(&source), diagram);
    }
    out
}

fn theme_colors(k: &Kit) -> HashMap<&'static str, String> {
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
        ("--pele-font", format!("'{FONT}'")),
        ("--pele-font-mono", format!("'{FONT}'")),
    ])
}

/// resvg has no CSS custom properties. Pele's root `style` maps `--pele-*` onto its own `--_*`
/// names and the shapes use those, so resolve the theme first, then the SVG's own declarations.
fn resolve_css_vars(svg: &str, theme: &HashMap<&'static str, String>) -> String {
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

/// Fences keyed without trailing whitespace, which the fence parser and Markdown differ on.
fn fence_key(source: &str) -> String {
    source.lines().map(str::trim_end).collect::<Vec<_>>().join("\n").trim_end().to_owned()
}

/// Replaces each `var(--name, fallback)` with `lookup(name)`, else its fallback, else (unless
/// `keep_unknown`) `none`.
fn substitute_vars(svg: &str, lookup: &dyn Fn(&str) -> Option<String>, keep_unknown: bool) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(start) = rest.find("var(") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 4..];
        // Find the matching close paren.
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

fn rasterise(svg: &str, fonts: &Arc<usvg::fontdb::Database>) -> Result<Diagram, String> {
    let options = usvg::Options { fontdb: fonts.clone(), font_family: FONT.into(), ..Default::default() };
    let tree = usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    let size = tree.size();
    let scale = 2.0;
    let mut pixmap = tiny_skia::Pixmap::new((size.width() * scale).ceil() as u32, (size.height() * scale).ceil() as u32).ok_or("empty svg")?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let rgba = pixmap.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    });
    let pixels = Pixels::new();
    pixels.set(pixmap.width(), pixmap.height(), rgba.collect());
    Ok(Diagram::Drawn { pixels, width: size.width(), height: size.height() })
}

fn pane(title: &str, body: El, k: &Kit) -> El {
    col().grow(1.0).h_full().min_h(0.0).min_w(0.0).gap(8.0)
        .child(text(title.to_owned()).color(k.c.text_muted))
        .child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body))
}

fn view(s: &App, cx: &mut Cx<App>) -> El {
    let k = Kit::neutral(cx.system_dark());
    let image = s.image.clone();
    let images = move |src: &str| (src == "tree.png").then(|| image.clone()).flatten();

    let native = Markdown::new(SAMPLE).selectable(true).images(images.clone()).build(&k);

    let pele = s.pele.clone();
    let danger = k.c.danger.icon;
    let pele_md = Markdown::new(SAMPLE.replace("```mermaid", "```pele"))
        .selectable(true)
        .images(images)
        .code_frame(move |code, frame| {
            if frame.language != Some("pele") {
                return code;
            }
            match pele.get(&fence_key(frame.source)) {
                Some(Diagram::Drawn { pixels, width, height }) => el().w_full().max_w(*width).aspect_ratio(width / height).pixels(pixels),
                Some(Diagram::Failed(e)) => col().w_full().gap(4.0).child(text(format!("Diagram error: {e}")).color(danger)).child(code),
                None => code,
            }
        })
        .build(&k);

    row().w_full().h_full().gap(24.0).padding(24.0).fill(k.c.bg_base)
        .child(pane("rune-mermaid", native, &k))
        .child(pane("Pele (headless)", pele_md, &k))
}

fn main() {
    rune::run(AppSpec::<App> {
        families: (FONT, FONT),
        setup: Some(setup),
        ..AppSpec::new("Renderer spike", (1600.0, 1000.0), App::default, view)
    });
}
