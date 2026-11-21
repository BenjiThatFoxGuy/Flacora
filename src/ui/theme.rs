//! Flacora's Rune Kit styling and its Astryx theme presets.

mod presets;

use crate::app::NotificationKind;
use crate::icons;
use crate::store::ThemePreset;
use rune::{Color, Family, Shadow, TextStyle};
use rune_kit::{Kit as RuneKit, Radius, StateColors, Type};
use std::{ops::Deref, sync::OnceLock};

pub use presets::FONTS;
pub const PAGE_INSET: f32 = 24.0;
pub const PAGE_CONTENT_MAX_WIDTH: f32 = 1120.0;
pub const CARD_PADDING: f32 = 12.0;
pub const FOCUS_RING_WIDTH: f32 = 2.0;
pub const FOCUS_RING_OFFSET: f32 = 2.0;

pub struct Kit {
    base: RuneKit,
    pub heading: TextStyle,
    pub subheading: TextStyle,
    pub card_radius: f32,
    pub drop_hint: Color,
    pub segmented: RuneKit,
    pub danger_button: RuneKit,
    pub danger_icon: RuneKit,
    toasts: [RuneKit; 3],
}

impl Deref for Kit {
    type Target = RuneKit;

    fn deref(&self) -> &RuneKit {
        &self.base
    }
}

impl Kit {
    pub fn toast(&self, kind: NotificationKind) -> &RuneKit {
        &self.toasts[match kind {
            NotificationKind::Success => 0,
            NotificationKind::Warning => 1,
            NotificationKind::Error => 2,
        }]
    }
}

pub fn kit(preset: ThemePreset, dark: bool) -> &'static Kit {
    static KITS: [[OnceLock<Kit>; 2]; ThemePreset::ALL.len()] =
        [const { [const { OnceLock::new() }; 2] }; ThemePreset::ALL.len()];
    // Gothic uses the same dark palette in both modes.
    let dark = dark || preset == ThemePreset::Gothic;
    KITS[preset as usize][usize::from(dark)].get_or_init(|| build(preset, dark))
}

fn build(preset: ThemePreset, dark: bool) -> Kit {
    let mut base = neutral(dark);
    let mut heading = TextStyle::new(24.0, 600.0).lh(32.0);
    let mut subheading = TextStyle::new(17.0, 700.0).lh(24.0);
    let mut card_radius = 12.0;
    if let Some((tokens, body_family, heading_family)) = presets::load(preset, dark) {
        presets::apply(&tokens, &mut base);
        presets::set_body_family(&mut base.ty, body_family);
        heading = heading.family(heading_family);
        subheading = subheading.family(heading_family);
        card_radius = tokens.num("radius.container");
    }

    let drop_hint = if dark && preset == ThemePreset::Neutral {
        rune::color::hex("#c6c6c6").expect("Astryx color")
    } else {
        base.c.text_muted
    };
    let mut segmented = base.clone();
    segmented.c.border_base = Color::TRANSPARENT;
    segmented.c.border_strong = Color::TRANSPARENT;
    segmented.c.text_muted = drop_hint;

    let mut danger_button = base.clone();
    danger_button.c.border_focus = base.c.danger.border;

    let mut danger_icon = danger_button.clone();
    danger_icon.c.text_base = base.c.danger.icon;
    danger_icon.c.hover = base.c.danger.fg.alpha(0.10);
    danger_icon.c.pressed = base.c.danger.fg.alpha(0.18);

    let toasts = [base.c.success, base.c.warning, base.c.danger].map(|colors| {
        let mut k = base.clone();
        k.c.layer_01 = colors.bg;
        k.c.text_base = colors.fg;
        k.c.text_muted = colors.fg;
        k.c.icon_muted = colors.icon;
        k.c.border_focus = colors.fg;
        k
    });
    Kit { base, heading, subheading, card_radius, drop_hint, segmented, danger_button, danger_icon, toasts }
}

// Neutral follows Astryx at 82ff7bbf64867ab047db3235d24667d15a72f97d.
fn neutral(dark: bool) -> RuneKit {
    let color = |light, night| rune::color::hex(if dark { night } else { light }).expect("Astryx color");
    let body = color("#f1f1f1", "#1b1b1b");
    let surface = color("#ffffff", "#262626");
    let card = color("#ffffff", "#262626");
    let text = color("#000000", "#ffffff");
    let secondary = color("#474747", "#9e9e9e");
    let icon_secondary = color("#6a6a6a", "#9e9e9e");
    let accent = color("#1b1b1b", "#f1f1f1");
    let on_accent = color("#ffffff", "#111111");
    let selected = color("#d4d4d4", "#474747");
    let gray = color("#e2e2e2", "#303030");
    let border = text.alpha(if dark { 0.10 } else { 0.08 });
    let emphasized = color("#d4d4d4", "#525252");
    let hover = text.alpha(0.05);
    let pressed = text.alpha(0.10);
    let darken = Color::BLACK.alpha(0.10);
    let lighten = Color::WHITE.alpha(0.12);

    let mut k = RuneKit::neutral(dark);
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
    if dark {
        k.elevation.floating.push(Shadow { spread: 1.0, color: Color::WHITE.alpha(0.08), inset: true, ..Shadow::default() });
    }
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
