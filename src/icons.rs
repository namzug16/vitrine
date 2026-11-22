//! Lucide icons (https://lucide.dev, ISC license, lucide-static 1.77.0) for what rune-kit's glyphs lack.
//! Stroke 1.5 on the 24 point grid draws 1 point at 16, the weight of the kit's glyphs.

use rune::Icon;

macro_rules! lucide {
    ($name:ident, $id:literal, $paths:literal) => {
        pub static $name: Icon = Icon {
            name: concat!("lucide-", $id),
            view: 24.0,
            body: concat!(r#"<g fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">"#, $paths, "</g>"),
        };
    };
}

lucide!(MAXIMIZE, "maximize-2", r#"<path d="M15 3h6v6"/><path d="m21 3-7 7"/><path d="m3 21 7-7"/><path d="M9 21H3v-6"/>"#);
lucide!(REFRESH, "refresh-cw", r#"<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>"#);
