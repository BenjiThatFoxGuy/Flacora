//! Native token mapping for the Astryx Neutral component gallery.
//! Source: github.com/meta/astryx at 82ff7bbf64867ab047db3235d24667d15a72f97d,
//! packages/themes/neutral/src/neutralTheme.ts and neutralPaletteRefs.generated.ts.

use crate::icons;
use rune::{Color, Family, Shadow, TextStyle};
use rune_kit::{Kit, Radius, StateColors, Type};
use std::sync::OnceLock;

pub const FONT: &[u8] = include_bytes!("../../assets/Figtree.ttf");
pub const PAGE_INSET: f32 = 24.0;
pub const PAGE_CONTENT_MAX_WIDTH: f32 = 1120.0;
pub const CARD_RADIUS: f32 = 12.0;
pub const CARD_PADDING: f32 = 12.0;
pub const HEADING: TextStyle = TextStyle::new(24.0, 600.0).lh(32.0);
pub const SUBHEADING: TextStyle = TextStyle::new(17.0, 700.0).lh(24.0);

pub fn kit(dark: bool) -> &'static Kit {
    static DARK: OnceLock<Kit> = OnceLock::new();
    static LIGHT: OnceLock<Kit> = OnceLock::new();
    let cache = if dark { &DARK } else { &LIGHT };
    cache.get_or_init(|| build(dark))
}

pub fn segmented_kit(dark: bool) -> &'static Kit {
    static DARK: OnceLock<Kit> = OnceLock::new();
    static LIGHT: OnceLock<Kit> = OnceLock::new();
    let cache = if dark { &DARK } else { &LIGHT };
    cache.get_or_init(|| {
        let mut k = kit(dark).clone();
        k.c.border_base = Color::TRANSPARENT;
        k.c.border_strong = Color::TRANSPARENT;
        k
    })
}

pub fn danger_icon_kit(dark: bool) -> &'static Kit {
    static DARK: OnceLock<Kit> = OnceLock::new();
    static LIGHT: OnceLock<Kit> = OnceLock::new();
    let cache = if dark { &DARK } else { &LIGHT };
    cache.get_or_init(|| {
        let mut k = kit(dark).clone();
        k.c.text_base = k.c.danger.icon;
        k.c.hover = k.c.danger.fg.alpha(0.10);
        k.c.pressed = k.c.danger.fg.alpha(0.18);
        k.c.border_focus = k.c.danger.border;
        k
    })
}

pub fn toast_kit(dark: bool, kind: crate::app::NotificationKind) -> &'static Kit {
    use crate::app::NotificationKind;
    static DARK: OnceLock<[Kit; 3]> = OnceLock::new();
    static LIGHT: OnceLock<[Kit; 3]> = OnceLock::new();
    let cache = if dark { &DARK } else { &LIGHT };
    let kits = cache.get_or_init(|| {
        let base = kit(dark);
        [base.c.success, base.c.warning, base.c.danger].map(|colors| {
            let mut k = base.clone();
            k.c.layer_01 = colors.bg;
            k.c.text_base = colors.fg;
            k.c.text_muted = colors.fg;
            k.c.icon_muted = colors.icon;
            k.c.border_focus = colors.fg;
            k.elevation.floating.clear();
            k
        })
    });
    match kind {
        NotificationKind::Success => &kits[0],
        NotificationKind::Warning => &kits[1],
        NotificationKind::Error => &kits[2],
    }
}

fn build(dark: bool) -> Kit {
    let color = |light, night| rune::color::hex(if dark { night } else { light }).expect("Astryx color");
    let body = color("#f1f1f1", "#141414");
    let surface = color("#ffffff", "#191919");
    let card = color("#ffffff", "#1e1e1e");
    let text = color("#000000", "#ffffff");
    let secondary = color("#474747", "#9e9e9e");
    let icon_secondary = color("#6a6a6a", "#9e9e9e");
    let accent = color("#1b1b1b", "#f1f1f1");
    let on_accent = color("#ffffff", "#111111");
    let selected = color("#d4d4d4", "#3d3d3d");
    let gray = color("#e2e2e2", "#2b2b2b");
    let border = text.alpha(if dark { 0.10 } else { 0.08 });
    let emphasized = color("#d4d4d4", "#525252");
    let hover = text.alpha(0.05);
    let pressed = text.alpha(0.10);
    let darken = Color::BLACK.alpha(0.10);
    let lighten = Color::WHITE.alpha(0.12);

    let mut k = Kit::neutral(dark);
    let c = &mut k.c;
    c.page = body;
    c.bg_base = surface;
    c.bg_deep = body;
    c.layer_01 = gray;
    c.layer_02 = gray;
    c.layer_03 = gray;
    c.layer_04 = selected;
    c.bg_inverse = text;
    c.bg_contrast = accent;
    c.bg_button_neutral = text.alpha(if dark { 0.10 } else { 0.06 });
    c.bg_icon_button_contrast = accent;
    c.bg_accent = accent;
    c.submit_highlight = Color::TRANSPARENT;
    c.submit_hover = if dark { darken } else { lighten };
    c.submit_fg = on_accent;
    c.submit_icon = on_accent;
    c.text_base = text;
    c.text_strong = text;
    c.text_muted = secondary;
    c.text_weak = secondary;
    c.text_faint = secondary;
    c.text_accent = accent;
    c.text_inverse = on_accent;
    c.text_contrast = on_accent;
    c.text_invert_strong = on_accent;
    c.icon_base = text;
    c.icon_muted = icon_secondary;
    c.icon_faint = icon_secondary;
    c.icon_weaker = icon_secondary;
    c.icon_accent = accent;
    c.icon_inverse = on_accent;
    c.icon_contrast = on_accent;
    c.border_muted = emphasized;
    c.border_base = border;
    c.border_weak = border;
    c.border_strong = emphasized;
    c.border_hover = emphasized;
    c.border_disabled = border;
    c.border_focus = accent;
    c.border_active = accent;
    c.surface_base = emphasized;
    c.surface_weak = gray;
    c.surface_hover = hover;
    c.surface_raised = card;
    c.surface_raised_base = card;
    c.surface_raised_hover = hover;
    c.surface_float = card;
    c.surface_disabled = gray;
    c.std_border = border;
    c.std_icon = icon_secondary;
    c.std_icon_invert = on_accent;
    c.std_icon_strong = text;
    c.std_text = text;
    c.hover = hover;
    c.pressed = pressed;
    c.contrast_hover = c.submit_hover;
    c.contrast_pressed = if dark { Color::BLACK.alpha(0.18) } else { Color::WHITE.alpha(0.22) };
    c.scrim = Color::BLACK.alpha(if dark { 0.8 } else { 0.5 });
    c.knob = surface;
    c.knob_on = on_accent;

    c.info = status(color("#b9d7ff", "#253c5a"), color("#003978", "#a1caff"));
    c.success = status(color("#bce0bb", "#2b422b"), color("#00490b", "#90ca90"));
    c.warning = status(color("#fae19e", "#453a1c"), color("#4b3900", "#eec448"));
    c.danger = status(color("#ffc4be", "#5b2b28"), color("#76000c", "#ffaea7"));

    k.elevation.button_neutral.clear();
    k.elevation.button_contrast.clear();
    k.elevation.floating = vec![
        Shadow { y: 2.0, blur: 4.0, color: Color::BLACK.alpha(if dark { 0.25 } else { 0.05 }), ..Shadow::default() },
        Shadow { y: 4.0, blur: 8.0, color: Color::BLACK.alpha(if dark { 0.40 } else { 0.10 }), ..Shadow::default() },
    ];
    k.elevation = k.elevation.with_input(vec![Shadow { spread: 1.0, color: emphasized, inset: true, ..Shadow::default() }]);
    k.radius = Radius { xs: 6.0, sm: 10.0, md: 10.0, lg: 10.0, xl: 10.0 };
    k.ty = Type {
        ui: TextStyle::new(14.0, 400.0).lh(20.0),
        ui_medium: TextStyle::new(14.0, 500.0).lh(20.0),
        ui_compact: TextStyle::new(14.0, 400.0).lh(20.0),
        body: TextStyle::new(14.0, 400.0).lh(20.0),
        body_medium: TextStyle::new(14.0, 600.0).lh(20.0),
        small: TextStyle::new(12.0, 400.0).lh(16.0),
        small_medium: TextStyle::new(12.0, 500.0).lh(16.0),
        text_small: TextStyle::new(12.0, 400.0).lh(20.0),
        text_small_medium: TextStyle::new(12.0, 500.0).lh(20.0),
        text_xsmall: TextStyle::new(12.0, 400.0).lh(16.0),
        text_xsmall_medium: TextStyle::new(12.0, 500.0).lh(16.0),
        mono: TextStyle::new(12.0, 400.0).lh(18.0).family(Family::SystemMono),
    };
    k.glyph.clear = &icons::X;
    k.glyph.close = &icons::X;
    k
}

fn status(bg: Color, fg: Color) -> StateColors {
    StateColors { bg, fg, border: fg, icon: fg }
}
