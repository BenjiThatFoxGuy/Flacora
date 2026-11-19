//! Small pieces shared by the pages.

use super::theme::{CARD_PADDING, CARD_RADIUS, HEADING, PAGE_CONTENT_MAX_WIDTH, PAGE_INSET, SUBHEADING};
use crate::app::App;
use crate::extensions::ExtOption;
use crate::icons;
use crate::services::Service;
use rune::{Color, El, Icon, Image, ScrollSpec, SurfaceFit, Tx, col, el, icon, row, text};
use rune_kit::{Badge, BadgeVariant, Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit, ScrollBar, ScrollBarStyle, SegmentedControl, StateColors};

/// Formats extension-provided captions for display while preserving mixed-case names and paths.
pub fn title_case_label(label: &str) -> String {
    let mut words = label.split_whitespace().peekable();
    let mut title = String::with_capacity(label.len());
    while let Some(word) = words.next() {
        let middle = !title.is_empty() && words.peek().is_some();
        if !title.is_empty() {
            title.push(' ');
        }
        let connector = matches!(word, "a" | "an" | "and" | "as" | "at" | "by" | "for" | "from" | "in" | "of" | "on" | "or" | "per" | "the" | "to" | "via" | "with");
        if (middle && connector) || word.contains(['.', '/', '\\', '@']) || word.chars().any(char::is_uppercase) {
            title.push_str(word);
        } else if let Some(first) = word.chars().next() {
            title.extend(first.to_uppercase());
            title.push_str(&word[first.len_utf8()..]);
        }
    }
    title
}

pub fn page(key: &'static str) -> El {
    col().key(key).w_full().max_w(PAGE_CONTENT_MAX_WIDTH + 2.0 * PAGE_INSET)
        .grow(1.0).min_h(0.0).padding(PAGE_INSET).gap(16.0)
}

/// Page title with a one-line description and actions on the right.
pub fn page_header(k: &Kit, title: &str, subtitle: &str, actions: Vec<El>) -> El {
    page_header_with_subtitle(k, title, text(subtitle.to_owned()).font(k.ty.body).color(k.c.text_muted).wrap(), actions)
}

pub fn page_header_with_subtitle(k: &Kit, title: &str, subtitle: El, actions: Vec<El>) -> El {
    let mut head = row()
        .key("page-head")
        .w_full()
        .gap(12.0)
        .center_y()
        .flex_wrap()
        .child(
            col()
                .gap(4.0)
                .grow(1.0)
                .shrink()
                .min_w(220.0)
                .child(text(title.to_owned()).role("heading").font(HEADING).color(k.c.text_strong))
                .child(subtitle),
        );
    for action in actions {
        head = head.child(action);
    }
    head
}

/// A heading above a group of rows, with an optional count.
pub fn section(k: &Kit, title: &str, count: Option<usize>) -> El {
    let mut head = row().gap(8.0).center_y().child(text(title.to_owned()).role("heading").font(SUBHEADING).color(k.c.text_strong));
    if let Some(count) = count {
        head = head.child(badge(k, count.to_string()));
    }
    head
}

pub fn card(k: &Kit, key: impl Into<String>, body: El) -> El {
    col()
        .key(key)
        .role("group")
        .w_full()
        .min_w(0.0)
        .padding(CARD_PADDING)
        .radius(CARD_RADIUS)
        .fill(k.c.surface_raised_base)
        .border(1.0, k.c.border_base)
        .child(body)
}

pub fn badge(k: &Kit, label: impl Into<String>) -> El {
    Badge::new(label).variant(BadgeVariant::Subtle).build(k).px(8.0).h(20.0).radius(999.0)
}

pub fn badge_with_icon(k: &Kit, label: &str, leading: El) -> El {
    row()
        .center()
        .gap(4.0)
        .pr(8.0)
        .child(leading.a11y_hidden())
        .child(badge(k, label).px(0.0).fill(Color::TRANSPARENT))
}

pub fn segmented(k: &Kit, key: &str, control: SegmentedControl) -> El {
    row()
        .key(format!("{key}-track"))
        .padding(4.0)
        .radius(k.radius.md)
        .fill(k.c.layer_01)
        .child(
            control.fit().build(super::theme::segmented_kit(k.dark))
                .gap(4.0)
                .grow(1.0)
                .max_w(760.0)
                .key(key.to_owned()),
        )
}

pub fn extension_option(k: &Kit, s: &App, extension: &str, option: &ExtOption, key: &str) -> El {
    let labels: Vec<String> = option.choices.iter().map(|choice| title_case_label(&choice.label)).collect();
    let selected = s.pick_index(extension, option);
    let (extension, option_key) = (extension.to_owned(), option.key.clone());
    segmented(k, key, SegmentedControl::new(&labels, Some(selected))
        .label(title_case_label(&option.label))
        .on_pick(move |s: &mut App, i: usize, _: &mut Tx| s.set_pick(&extension, &option_key, i)))
}

pub fn scroll_bar_style(k: &Kit) -> ScrollBarStyle {
    ScrollBarStyle::from_kit(k).thumb(k.c.icon_muted.alpha(0.72)).thumb_hover(k.c.icon_base.alpha(0.72))
}

pub fn page_scroll(k: &Kit, body: El) -> El {
    let bar = scroll_bar_style(k);
    body.scroll(ScrollSpec { stick_bottom: false }).grow(1.0).min_h(0.0)
        // Place the thumb in the page's right gutter.
        .pr(bar.strip())
        .mr(-bar.strip())
        .scroll_bar_styled(bar)
}

pub fn status_badge(k: &Kit, glyph: &'static Icon, label: &str, status: Option<StateColors>) -> El {
    let (bg, ink) = status.map_or((k.c.layer_03, k.c.text_muted), |c| (c.bg, c.fg));
    row()
        .role("status")
        .label(label.to_owned())
        .h(24.0)
        .px(8.0)
        .gap(4.0)
        .center()
        .radius(999.0)
        .fill(bg)
        .child(icon(glyph, 14.0).color(ink).a11y_hidden())
        .child(text(label.to_owned()).font(k.ty.small_medium).color(ink).a11y_hidden())
}

/// The detected service's name, or the supplied label.
pub fn service_chip(k: &Kit, service: Option<&Service>, fallback: &str) -> El {
    badge(k, service.map_or(fallback, |s| s.name))
}

pub fn provider_logo(s: &App, service: &Service, size: f32) -> Option<El> {
    let image = s.provider_logos.get(service.key).filter(|image| image.error().is_none())?;
    Some(el().key(format!("provider-icon-{}", service.key)).size(size, size).pixels_fit(image.pixels(), SurfaceFit::Fit))
}

/// Cover art with a music icon as its placeholder.
pub fn cover(k: &Kit, image: Option<&Image>, size: f32) -> El {
    let base = el().size(size, size).radius(k.radius.md).fill(k.c.layer_02).a11y_hidden();
    match image.filter(|i| i.error().is_none()) {
        Some(image) => base.pixels_fit(image.pixels(), SurfaceFit::Fill),
        None => base.center().child(icon(&icons::MUSIC, size * 0.42).color(k.c.icon_muted)),
    }
}

pub fn icon_button(k: &Kit, key: &str, glyph: &'static Icon, label: &str, on_press: impl Fn(&mut App) + 'static) -> El {
    IconButton::new(glyph).variant(IconButtonVariant::Ghost).control_size(ControlSize::Large).label(label).help(label).on_press(on_press)
        .build(k).key(key.to_owned())
}

pub fn danger_icon_button(k: &Kit, key: &str, glyph: &'static Icon, label: &str, on_press: impl Fn(&mut App) + 'static) -> El {
    icon_button(super::theme::danger_icon_kit(k.dark), key, glyph, label, on_press)
}

pub fn button(k: &Kit, key: &str, label: &str, glyph: Option<&'static Icon>, variant: ButtonVariant, on_press: impl Fn(&mut App) + 'static) -> El {
    let mut b = Button::new(label).variant(variant).control_size(ControlSize::Regular).on_press(on_press);
    if let Some(glyph) = glyph {
        b = b.icon(glyph);
    }
    let button = b.build(k).h(32.0).key(key.to_owned());
    if variant == ButtonVariant::Danger { button.fill(k.c.danger.bg) } else { button }
}

/// A centred illustration, message and optional action, for empty pages.
pub fn empty_state(k: &Kit, key: &str, glyph: &'static Icon, title: &str, body: &str, action: Option<El>) -> El {
    let mut state = col()
        .key(key.to_owned())
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(16.0)
        .center()
        .child(
            el().size(56.0, 56.0).center().radius(999.0).fill(k.c.layer_02)
                .child(icon(glyph, 28.0).color(k.c.icon_base)).a11y_hidden(),
        )
        .child(
            col().center().gap(4.0).max_w(360.0)
                .child(text(title.to_owned()).role("heading").font(SUBHEADING).color(k.c.text_strong).align(rune::Align::Center))
                .child(text(body.to_owned()).font(k.ty.body).color(k.c.text_muted).wrap().align(rune::Align::Center)),
        );
    if let Some(action) = action {
        state = state.child(el().h(4.0)).child(action);
    }
    state
}

/// "Just Now", "5 min ago", "3 h ago", "2 d ago", else a date.
pub fn ago(date: u64, now: u64) -> String {
    let seconds = now.saturating_sub(date);
    match seconds {
        0..60 => "Just Now".into(),
        60..3600 => format!("{} min ago", seconds / 60),
        3600..86_400 => format!("{} h ago", seconds / 3600),
        86_400..2_592_000 => format!("{} d ago", seconds / 86_400),
        _ => {
            let days = date / 86_400;
            // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
            let z = days as i64 + 719_468;
            let era = z.div_euclid(146_097);
            let doe = z - era * 146_097;
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let day = doy - (153 * mp + 2) / 5 + 1;
            let month = if mp < 10 { mp + 3 } else { mp - 9 };
            let year = yoe + era * 400 + i64::from(month <= 2);
            format!("{year}-{month:02}-{day:02}")
        }
    }
}
