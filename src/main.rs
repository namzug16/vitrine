use rune::{AppSpec, Cx, El, col, text};

#[derive(Default)]
struct App;

fn view(_: &App, cx: &mut Cx<App>) -> El {
    let k = rune_kit::Kit::neutral(cx.system_dark());
    col().w_full().h_full().center().fill(k.c.bg_base).child(text("Vitrine").color(k.c.text_base))
}

fn main() {
    rune::run(AppSpec::new("Vitrine", (1100.0, 720.0), App::default, view));
}
