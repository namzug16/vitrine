//! The expanded diagram: near the full window, with drag to pan, scroll or pinch to zoom,
//! Fit to window and Reset zoom. Esc closes it; the document under it keeps its place.

use rune::{Cursor, El, Phase, col, el, row, text};
use rune_kit::{Button, ButtonVariant, Kit};

use crate::App;
use crate::diagrams::Diagram;

const ZOOM_RANGE: (f32, f32) = (0.1, 8.0);

#[derive(Clone, Copy)]
pub struct Camera {
    pub zoom: f32,
    /// The diagram's top-left in canvas points.
    pub x: f32,
    pub y: f32,
}

pub struct Viewer {
    pub source: String,
    /// `None` fits the diagram to the canvas, also as the canvas resizes.
    pub camera: Option<Camera>,
    pub canvas: [f32; 2],
    drag_start: Option<Camera>,
}

impl Viewer {
    pub fn new(source: String) -> Viewer {
        Viewer { source, camera: None, canvas: [0.0, 0.0], drag_start: None }
    }

    fn current(&self, size: [f32; 2]) -> Camera {
        self.camera.unwrap_or_else(|| {
            let [cw, ch] = self.canvas;
            let zoom = (cw / size[0]).min(ch / size[1]).clamp(ZOOM_RANGE.0, ZOOM_RANGE.1) * 0.94;
            centred(zoom, size, self.canvas)
        })
    }

    fn zoom_about(&mut self, size: [f32; 2], factor: f32, px: f32, py: f32) {
        let c = self.current(size);
        let zoom = (c.zoom * factor).clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
        let k = zoom / c.zoom;
        self.camera = Some(Camera { zoom, x: px - (px - c.x) * k, y: py - (py - c.y) * k });
    }
}

fn centred(zoom: f32, size: [f32; 2], canvas: [f32; 2]) -> Camera {
    Camera { zoom, x: (canvas[0] - size[0] * zoom) / 2.0, y: (canvas[1] - size[1] * zoom) / 2.0 }
}

fn viewer_mut(s: &mut App) -> &mut Viewer {
    s.project.as_mut().and_then(|p| p.viewer.as_mut()).expect("viewer is open")
}

pub fn close(s: &mut App) {
    if let Some(p) = s.project.as_mut() {
        p.viewer = None;
    }
}

pub fn view(v: &Viewer, diagram: &Diagram, k: &Kit) -> El {
    let size = [diagram.width_points, diagram.height_points];
    let camera = v.current(size);
    let picture = el()
        .absolute(camera.x, camera.y)
        .size(size[0] * camera.zoom, size[1] * camera.zoom)
        .pixels(&diagram.pixels);
    let canvas = el()
        .key("diagram-canvas")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .clip_rounded(k.radius.md)
        .cursor(Cursor::Grab)
        .on_layout(|s: &mut App, [_, _, w, h]| viewer_mut(s).canvas = [w, h])
        .on_drag(move |s: &mut App, drag, _| {
            let v = viewer_mut(s);
            match drag.phase {
                Phase::Down => v.drag_start = Some(v.current(size)),
                Phase::Move => {
                    if let Some(start) = v.drag_start {
                        v.camera = Some(Camera { x: start.x + drag.dx, y: start.y + drag.dy, ..start });
                    }
                }
                Phase::Up => v.drag_start = None,
            }
        })
        .on_wheel(move |s: &mut App, e, _| {
            viewer_mut(s).zoom_about(size, (-e.dy * 0.01).exp(), e.x, e.y);
            true
        })
        .on_magnify(move |s: &mut App, e, _| {
            viewer_mut(s).zoom_about(size, 1.0 + e.delta, e.x, e.y);
            true
        })
        .child(picture);
    let toolbar = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(text(format!("{:.0}%", camera.zoom * 100.0)).font(k.ty.ui).color(k.c.text_muted).grow(1.0))
        .child(Button::new("Fit to Window").variant(ButtonVariant::Ghost).on_press(|s: &mut App| viewer_mut(s).camera = None).build(k).key("fit"))
        .child(
            Button::new("Reset Zoom")
                .variant(ButtonVariant::Ghost)
                .on_press(move |s: &mut App| {
                    let v = viewer_mut(s);
                    v.camera = Some(centred(1.0, size, v.canvas));
                })
                .build(k)
                .key("reset-zoom"),
        )
        .child(Button::new("Close").variant(ButtonVariant::Outline).on_press(close).build(k).key("close-diagram"));
    let panel = col()
        .role("dialog")
        .label("Expanded diagram")
        .w_full()
        .h_full()
        .padding(16.0)
        .gap(12.0)
        .radius(k.radius.lg)
        .fill(k.c.bg_base)
        .border(1.0, k.c.border_base)
        .child(toolbar)
        .child(canvas);
    // The top padding clears the traffic lights, which sit over the window's top-left corner.
    el().key("diagram-viewer").absolute(0.0, 0.0).w_full().h_full().padding(24.0).pt(48.0).fill(k.c.scrim).child(panel)
}
