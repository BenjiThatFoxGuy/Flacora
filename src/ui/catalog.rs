use super::widgets::{self, card};
use crate::{app::{App, Page}, catalog::{Item, Kind, ResultPage}, icons, services};
use rune::{Cursor, El, Icon, ScrollPos, SurfaceFit, Tx, col, el, icon, row, text};
use rune_kit::{Button, ButtonVariant, ControlSize, Divider, IconButton, IconButtonVariant, Kit, Spinner, TextInput};
use std::{cell::Cell, collections::BTreeMap, rc::Rc};

fn provider_display_name(name: &str) -> &str {
    name.strip_suffix(" Web").unwrap_or(name)
}

pub fn page(s: &App, k: &Kit) -> El {
    let head = widgets::page_header(k, "Catalog", "Browse music services and choose tracks to send to Downloads.", Vec::new());
    let providers: Vec<_> = s.catalog_providers().collect();
    let body = if providers.is_empty() {
        widgets::empty_state(k, "catalog-empty", &icons::SEARCH, "Install a Catalog Extension",
            s.catalog.error.as_deref().unwrap_or("Install a catalog extension for your music service."),
            Some(widgets::button(k, "catalog-install", "Go to Extensions", Some(&icons::PUZZLE), ButtonVariant::Submit,
                |s: &mut App| s.page = Page::Extensions)))
    } else {
        let mut bar = row().gap(6.0).flex_wrap().center_y();
        let current = s.chosen_catalog_provider().map(|p| p.manifest.id.as_str());
        for provider in providers {
            let id = provider.manifest.id.clone();
            let selected = current == Some(&id);
            let key = format!("catalog-provider-{id}");
            let label = provider_display_name(&provider.manifest.name).to_owned();
            let logo = services::by_name(&label).and_then(|svc| widgets::provider_logo(s, svc, 14.0));
            let mut button = Button::new(label.clone())
                .variant(if selected { ButtonVariant::Submit } else { ButtonVariant::Outline })
                .control_size(ControlSize::Small)
                .on_press(move |s: &mut App| {
                    if !selected {
                        s.catalog.select_provider(id.clone());
                        s.catalog_home();
                    }
                });
            if let Some(logo) = logo {
                button = button.label_with(move |font, ink| {
                    row().gap(6.0).center_y().child(logo.a11y_hidden()).child(text(label).font(font).color(ink))
                });
            }
            bar = bar.child(button.build(k).key(key));
        }
        let mut search = TextInput::new(&s.catalog.input)
            .placeholder("Search or Paste a Music Link…")
            .leading_icon(&icons::SEARCH).height(36.0)
            .on_edit(|s: &mut App| &mut s.catalog.input)
            .on_submit(|s: &mut App, _: &mut Tx| s.submit_catalog());
        if !s.catalog.input.text.is_empty() {
            search = search.clear_button();
        }
        let search = search.build(k).grow(1.0).min_w(0.0).key("catalog-input");
        let actions = row().w_full().gap(8.0).center_y().child(search)
            .child(widgets::button(k, "catalog-submit", "Go", Some(&icons::ARROW_RIGHT), ButtonVariant::Submit, |s: &mut App| s.submit_catalog()).h(36.0))
            .child(widgets::button(k, "catalog-home", "Home Feed", Some(&icons::DISC), ButtonVariant::Outline, |s: &mut App| s.catalog_home()).h(36.0));
        let mut body = col().w_full().grow(1.0).min_h(0.0).gap(12.0)
            .child(card(k, "catalog-controls", col().gap(10.0).child(bar).child(actions)));
        if s.catalog.busy() || s.catalog.can_go_back() {
            let mut status = row().gap(8.0).center_y();
            if s.catalog.can_go_back() {
                status = status.child(widgets::button(k, "catalog-back", "Back", None, ButtonVariant::Ghost, |s: &mut App| s.catalog.back()));
            }
            if s.catalog.busy() {
                status = status.child(Spinner::new().size(14.0).build(k))
                    .child(text("Loading Catalog…").font(k.ty.small).color(k.c.text_muted))
                    .child(widgets::button(k, "catalog-cancel", "Cancel", None, ButtonVariant::Ghost, |s: &mut App| s.catalog.cancel()));
            }
            body = body.child(status);
        }
        if let Some(error) = &s.catalog.error {
            let mut banner = row()
                .key("catalog-error-banner")
                .role("alert")
                .w_full()
                .gap(8.0)
                .center_y()
                .padding(10.0)
                .radius(k.radius.xs)
                .fill(k.c.danger.bg)
                .child(icon(&icons::TRIANGLE_ALERT, 14.0).color(k.c.danger.fg))
                .child(text(error.clone()).key("catalog-error").font(k.ty.small).color(k.c.danger.fg).wrap().grow(1.0).shrink());
            if s.catalog.can_retry() {
                banner = banner.child(widgets::button(k, "catalog-retry", "Try Again", Some(&icons::REFRESH_CW), ButtonVariant::Ghost, |s: &mut App| s.retry_catalog()));
            }
            if error == crate::catalog::MISSING_CATALOG_EXTENSION {
                banner = banner.child(widgets::button(
                    k,
                    "catalog-open-extensions",
                    "Go to Extensions",
                    Some(&icons::PUZZLE),
                    ButtonVariant::Outline,
                    |s: &mut App| s.page = Page::Extensions,
                ));
            }
            body = body.child(banner);
        }
        if let Some(result) = &s.catalog.result {
            body = body.child(results(s, k, result));
        } else if !s.catalog.busy() && s.catalog.error.is_none() {
            body = body.child(widgets::empty_state(
                k,
                "catalog-idle",
                &icons::SEARCH,
                "Browse Music Catalogs",
                "Pick a provider above, search for a track or album, paste a music link, or open Home Feed.",
                None,
            ));
        }
        body
    };
    widgets::page("catalog-page").child(head).child(body)
}

fn results(s: &App, k: &Kit, result: &ResultPage) -> El {
    let tracks = result.items.iter().any(|item| item.kind == Kind::Track);
    let all_selected = tracks && result.items.iter().filter(|item| item.kind == Kind::Track).all(|item| s.catalog.selected.contains(&item.url));
    let mut header = row().gap(10.0).w_full().center_y().flex_wrap();
    if let Some(url) = result.artwork.as_deref() {
        header = header.child(artwork(s, k, "catalog-cover", Some(url), &icons::DISC, 64.0, false));
    }
    header = header.child(text(result.title.clone()).font(k.ty.body_medium).color(k.c.text_strong).truncate().grow(1.0).min_w(0.0));
    if tracks {
        let (label, glyph) = if all_selected { ("Clear Selection", &icons::X) } else { ("Select Loaded Tracks", &icons::CIRCLE_CHECK) };
        header = header.child(widgets::button(k, "catalog-select-all", label, Some(glyph), ButtonVariant::Outline,
            move |s: &mut App| {
                if all_selected {
                    s.catalog.selected.clear();
                } else if let Some(result) = &s.catalog.result {
                    for item in result.items.iter().filter(|item| item.kind == Kind::Track) {
                        s.catalog.selected.insert(item.url.clone());
                    }
                }
            }));
    }
    if !s.catalog.selected.is_empty() {
        header = header.child(widgets::button(k, "catalog-use", &format!("To Downloads ({})", s.catalog.selected.len()),
            Some(&icons::ARROW_RIGHT), ButtonVariant::Submit, |s: &mut App| s.prepare_catalog_tracks()));
    }
    let home = s.catalog.showing_home();
    let mut list = if home && !result.items.is_empty() { feed(s, k, result) } else { result_rows(s, k, result) }.pb(16.0);
    if result.next.is_some() && !s.catalog.busy() {
        list = list.child(widgets::button(k, "catalog-more", "Load More", None, ButtonVariant::Outline,
            |s: &mut App| s.catalog_more()).self_center());
    }
    let mut content = col().key("catalog-result-page").w_full().grow(1.0).min_h(0.0).gap(6.0).child(header);
    if !result.note.is_empty() {
        content = content.child(text(result.note.clone()).font(k.ty.small).color(k.c.text_muted).wrap());
    }
    let key = if home { "catalog-feed" } else { "catalog-results" };
    content.child(widgets::page_scroll(k, col().key(key).child(list)))
}

fn result_rows(s: &App, k: &Kit, result: &ResultPage) -> El {
    let mut list = col().w_full().gap(4.0);
    if result.items.is_empty() {
        list = list.child(text("No items found.").font(k.ty.body).color(k.c.text_muted).py(16.0));
    }
    for (i, item) in result.items.iter().enumerate() {
        if i > 0 {
            list = list.child(Divider::horizontal().build(k));
        }
        list = list.child(item_row(s, k, i, item));
    }
    list
}

fn feed(s: &App, k: &Kit, result: &ResultPage) -> El {
    let shelves: Vec<(&str, Vec<&Item>)> = if result.sections.is_empty() {
        [Kind::Track, Kind::Album, Kind::Playlist, Kind::Artist].into_iter()
            .map(|kind| (kind.plural(), result.items.iter().filter(|item| item.kind == kind).collect()))
            .collect()
    } else {
        let items: BTreeMap<_, _> = result.items.iter().map(|item| (item.url.as_str(), item)).collect();
        result.sections.iter().map(|section| {
            let cards = section.urls.iter().filter_map(|url| items.get(url.as_str()).copied()).collect();
            (section.title.as_str(), cards)
        }).collect()
    };
    col().w_full().gap(22.0).children(shelves.into_iter().filter(|(_, items)| !items.is_empty()).enumerate()
        .map(|(index, (title, items))| feed_shelf(s, k, index, title, &items)))
}

fn feed_shelf(s: &App, k: &Kit, index: usize, title: &str, items: &[&Item]) -> El {
    let key = format!("catalog-shelf-{index}");
    let position = Rc::new(Cell::new(ScrollPos::default()));
    let watch = position.clone();
    let cards = row().gap(10.0).children(items.iter().enumerate().map(|(item_index, item)| {
        feed_card(s, k, format!("catalog-feed-item-{index}-{item_index}"), item)
    }));
    let shelf = col().key(key.clone()).w_full().min_w(0.0).scroll_x().scroll_edge_fade(12.0)
        .on_scroll_offset(move |at| watch.set(at))
        .child(cards);
    let mut head = row().w_full().center_y().gap(4.0)
        .child(widgets::section(k, title, None).grow(1.0).min_w(0.0));
    if items.len() > 1 {
        for (direction, glyph, label, name) in [
            (-1.0, &icons::CHEVRON_LEFT, "Scroll Left", "prev"),
            (1.0, &icons::CHEVRON_RIGHT, "Scroll Right", "next"),
        ] {
            let viewport = key.clone();
            let position = position.clone();
            head = head.child(IconButton::new(glyph).variant(IconButtonVariant::Ghost)
                .control_size(ControlSize::Small).label(label).help(label)
                .on_press_tx(move |_: &mut App, tx| {
                    let at = position.get();
                    let x = (at.offset[0] + direction * at.viewport[0] * 0.85).clamp(0.0, at.max[0]);
                    tx.scroll_to(viewport.clone(), x, 0.0);
                })
                .build(k).key(format!("{key}-{name}")));
        }
    }
    col().w_full().min_w(0.0).gap(6.0).child(head).child(shelf)
}

fn feed_card(s: &App, k: &Kit, key: String, item: &Item) -> El {
    let selected = s.catalog.selected.contains(&item.url);
    let action = match (item.kind, selected) {
        (Kind::Track, true) => "Deselect",
        (Kind::Track, false) => "Select",
        _ => "Open",
    };
    let url = item.url.clone();
    let kind = item.kind;
    let mut title = row().w_full().gap(4.0).center_y()
        .child(text(item.title.clone()).font(k.ty.body_medium).color(k.c.text_strong).truncate().grow(1.0).min_w(0.0));
    if selected {
        title = title.child(icon(&icons::CIRCLE_CHECK, 14.0).color(k.c.success.fg));
    }
    col().key(key.clone()).w(152.0).padding(8.0).gap(4.0)
        .role("button").label(format!("{action} {}", item.title)).focusable(true)
        .focus_ring(2.0, 0.0, k.c.border_focus).cursor(Cursor::Pointer)
        .radius(k.radius.md).fill(k.c.surface_raised_base).hover(k.c.hover)
        .border(1.0, if selected { k.c.border_focus } else { k.c.border_base })
        .on_press(move |s: &mut App| {
            if kind == Kind::Track {
                toggle_track(s, &url);
            } else {
                s.open_catalog(url.clone(), true);
            }
        })
        .child(artwork(s, k, &format!("{key}-artwork"), item.artwork.as_deref(), item_icon(kind), 134.0, kind == Kind::Artist))
        .child(title)
        .child(text(if item.detail.is_empty() { item.kind.label().to_owned() } else { item.detail.clone() })
            .font(k.ty.small).color(k.c.text_muted).truncate())
}

fn toggle_track(s: &mut App, url: &str) {
    if !s.catalog.selected.remove(url) {
        s.catalog.selected.insert(url.to_owned());
    }
}

fn item_icon(kind: Kind) -> &'static Icon {
    match kind {
        Kind::Track => &icons::MUSIC,
        Kind::Album => &icons::DISC,
        Kind::Playlist => &icons::LIST_MUSIC,
        Kind::Artist => &icons::MIC_VOCAL,
    }
}

fn item_row(s: &App, k: &Kit, index: usize, item: &Item) -> El {
    let url = item.url.clone();
    let selected = s.catalog.selected.contains(&url);
    let description = if item.detail.is_empty() { item.kind.label().into() } else { format!("{} · {}", item.kind.label(), item.detail) };
    let art = artwork(s, k, &format!("catalog-artwork-{index}"), item.artwork.as_deref(), item_icon(item.kind), 44.0, item.kind == Kind::Artist);
    let info = col().gap(3.0).grow(1.0).min_w(0.0)
        .child(text(item.title.clone()).font(k.ty.body_medium).color(k.c.text_strong).truncate())
        .child(text(description).font(k.ty.small).color(k.c.text_muted).truncate());
    if item.kind == Kind::Track {
        let action = widgets::button(k, &format!("catalog-item-{index}"), if selected { "Selected" } else { "Select" },
            selected.then_some(&icons::CIRCLE_CHECK), if selected { ButtonVariant::Submit } else { ButtonVariant::Outline },
            move |s: &mut App| toggle_track(s, &url));
        row().w_full().gap(10.0).center_y().px(4.0).py(4.0)
            .child(art)
            .child(info)
            .child(action)
    } else {
        let cue = row().gap(4.0).center_y().px(6.0).a11y_hidden()
            .child(text("Open").font(k.ty.small_medium).color(k.c.text_muted))
            .child(icon(&icons::CHEVRON_RIGHT, 14.0).color(k.c.icon_muted));
        row()
            .key(format!("catalog-item-{index}"))
            .role("button")
            .label(format!("Open {}", item.title))
            .focusable(true)
            .focus_ring(2.0, 1.0, k.c.border_focus)
            .cursor(Cursor::Pointer)
            .radius(k.radius.sm)
            .hover(k.c.hover)
            .w_full().gap(10.0).center_y().px(6.0).py(6.0)
            .on_press(move |s: &mut App| s.open_catalog(url.clone(), true))
            .child(art)
            .child(info)
            .child(cue)
    }
}

fn artwork(s: &App, k: &Kit, key: &str, url: Option<&str>, placeholder: &'static Icon, size: f32, round: bool) -> El {
    let radius = if round { size / 2.0 } else { k.radius.md };
    let frame = el().size(size, size).radius(radius).fill(k.c.layer_02).a11y_hidden();
    let frame = if let Some(image) = url.and_then(|url| s.catalog.artwork(url)) {
        frame.pixels_fit(image.pixels(), SurfaceFit::Fill)
    } else {
        frame.center().child(icon(placeholder, size * 0.42).color(k.c.icon_muted))
    };
    let frame = if let Some(url) = url {
        let url = url.to_owned();
        frame.key(url.clone()).on_visible(move |s: &mut App, visible, _| {
            if visible {
                s.request_catalog_artwork(&url);
            }
        })
    } else {
        frame
    };
    el().key(key.to_owned()).size(size, size).child(frame)
}
