//! Locally installed extensions and the package drop target.

use super::theme::{CARD_RADIUS, SUBHEADING};
use super::widgets::{self, badge, badge_with_icon, card, section, service_chip, status_badge};
use crate::app::{App, ConfirmAction};
use crate::extensions::Kind;
use crate::icons;
use crate::services;
use crate::store::{self, ExtensionStatus, Installed};
use rune::{Cursor, El, GrowthSpec, Link, LinkPaint, Spring, Tx, col, el, icon, row, text};
use rune_kit::{Button, ButtonVariant, ControlSize, Divider, Kit, Spinner};

const CATEGORIES: [(Kind, &str); 4] = [
    (Kind::Downloader, "downloaders"),
    (Kind::Resolver, "resolvers"),
    (Kind::Metadata, "metadata"),
    (Kind::Lyrics, "lyrics"),
];
const LIST_SPRING: Spring = Spring::new(0.26, 0.0);
const EXTENSIONS_URL: &str = "https://rentry.org/flacora";

fn select_filter(s: &mut App, tx: &mut Tx, filter: Option<Kind>) {
    if s.extension_filter != filter {
        s.extension_filter = filter;
        tx.scroll_to_with("extension-list", 0.0, 0.0, LIST_SPRING);
    }
}

pub fn page(s: &App, k: &Kit) -> El {
    let mut actions = Vec::new();
    if s.loaded && !s.installed.is_empty() {
        let checking = s.any_checking();
        let label = if checking { "Checking Status…" } else { "Check Status" };
        actions.push(widgets::button(k, "check-extension-status", label, Some(&icons::REFRESH_CW), ButtonVariant::Outline,
            App::check_all).disabled(checking || s.importing > 0));
        let label = if s.importing > 0 { "Installing Packages…" } else { "Install Extension" };
        actions.push(Button::new(label)
            .variant(ButtonVariant::Submit)
            .icon(&icons::FILE_UP)
            .help("Browse for Extension Packages")
            .disabled(s.browsing_packages)
            .on_press_tx(App::browse_packages)
            .build(k).h(32.0).key("install-extension"));
    }
    let head = widgets::page_header_with_subtitle(k, "Extensions", download_link(k), actions);
    let mut page = widgets::page("extensions-page").child(head);
    if s.loaded && s.installed.is_empty() {
        return page.child(col().w_full().grow(1.0).min_h(0.0).center().child(drop_zone(s, k)));
    }

    let mut list = col().key("extension-cards").growth(GrowthSpec::default());
    let mut filter_bar = None;
    if !s.loaded {
        list = list.child(text("Loading Installed Extensions…").font(k.ty.body).color(k.c.text_muted));
    } else {
        let present_categories: Vec<_> = CATEGORIES
            .iter()
            .filter_map(|&(kind, key)| {
                let count = s.installed.iter().filter(|i| i.manifest.kind == kind).count();
                (count > 0).then_some((kind, key, count))
            })
            .collect();
        let active_filter = s.extension_filter.filter(|kind| present_categories.iter().any(|(k, _, _)| k == kind));
        if present_categories.len() > 1 {
            let mut chips = row().key("extension-filters").gap(6.0).center_y().flex_wrap();
            let all_selected = active_filter.is_none();
            chips = chips.child(
                Button::new(format!("All ({})", s.installed.len()))
                    .variant(if all_selected { ButtonVariant::Submit } else { ButtonVariant::Outline })
                    .control_size(ControlSize::Small)
                    .on_press_tx(|s: &mut App, tx| select_filter(s, tx, None))
                    .build(k)
                    .key("ext-filter-all"),
            );
            for (kind, key, count) in &present_categories {
                let kind = *kind;
                let selected = active_filter == Some(kind);
                chips = chips.child(
                    Button::new(format!("{} ({count})", kind.label()))
                        .variant(if selected { ButtonVariant::Submit } else { ButtonVariant::Outline })
                        .control_size(ControlSize::Small)
                        .on_press_tx(move |s: &mut App, tx| select_filter(s, tx, Some(kind)))
                        .build(k)
                        .key(format!("ext-filter-{key}")),
                );
            }
            filter_bar = Some(chips);
        }
        let mut rows = Vec::new();
        for (kind, key) in CATEGORIES {
            if active_filter.is_some_and(|filter| filter != kind) {
                continue;
            }
            let mut installed: Vec<_> = s.installed.iter().filter(|i| i.manifest.kind == kind).collect();
            if installed.is_empty() {
                continue;
            }
            installed.sort_by_cached_key(|i| i.manifest.name.to_lowercase());
            rows.push(section(k, kind.label(), Some(installed.len())).key(format!("extension-category-{key}")));
            rows.push(el().key(format!("gap-after-category-{key}")).h(10.0).a11y_hidden());
            for extension in installed {
                rows.push(extension_card(s, k, extension));
                rows.push(el().key(format!("gap-after-card-{}", extension.manifest.id)).h(10.0).a11y_hidden());
            }
        }
        // Grow the gaps with the rows so they do not snap shut at the end of a collapse.
        rows.pop();
        list = list.children(rows.into_iter().map(|row| row.fold_glide(LIST_SPRING).fold_feather(4.0)));
    }
    if let Some(chips) = filter_bar {
        page = page.child(chips);
    }
    page.child(widgets::page_scroll(k, col().key("extension-list").child(list)))
}

fn download_link(k: &Kit) -> El {
    let label = "Get Extension on Rentry";
    let link = Link::new(0..label.len(), EXTENSIONS_URL)
        .hover(LinkPaint::new().underline(k.c.info.fg));
    text(label).key("download-extensions").self_start().font(k.ty.body_medium).color(k.c.info.fg)
        .link(link)
        .on_link(|s: &mut App, url, tx| {
            if let Err(error) = tx.open_url(url) {
                s.error(format!("Could not open Rentry: {error}"));
            }
        })
}

fn drop_zone(s: &App, k: &Kit) -> El {
    let active = s.drop_over;
    let installing = s.importing > 0 && !active;
    let label = if installing {
        "Installing Packages…".to_owned()
    } else if active {
        "Drop to Install".to_owned()
    } else {
        format!("Drop .{} Packages", store::PACKAGE)
    };

    let accent = if active { k.c.info.fg } else { k.c.text_strong };
    let symbol = if installing {
        Spinner::new().size(24.0).build(k)
    } else {
        icon(&icons::FILE_UP, 28.0).color(accent)
    };
    let mut message = col().center().gap(6.0)
        .child(text(label).role(if installing { "status" } else { "heading" }).font(SUBHEADING).color(accent));
    if !installing {
        message = message.child(
            text("Drop them anywhere in this window, or click here to browse.").font(k.ty.body).color(k.c.text_muted),
        );
    }

    col().w_full().max_w(560.0).h(224.0).center().gap(16.0)
        .child(el().size(56.0, 56.0).center().radius(999.0).fill(if active { k.c.info.bg } else { k.c.layer_02 })
            .child(symbol).a11y_hidden())
        .child(message)
        .key("drop-zone")
        .role("button")
        .label("Browse for Extension Packages")
        .focusable(true)
        .focus_ring(2.0, 2.0, k.c.border_focus)
        .cursor(Cursor::Pointer)
        .radius(CARD_RADIUS)
        .fill(if active { k.c.info.bg.alpha(0.38) } else { k.c.bg_base })
        .hover(k.c.hover)
        .border(if active { 2.0 } else { 1.0 }, if active { k.c.info.fg } else { k.c.border_strong })
        .border_dash(6.0, 4.0)
        .on_press_tx(App::browse_packages)
}

fn extension_card(s: &App, k: &Kit, installed: &Installed) -> El {
    let manifest = &installed.manifest;
    let id = &manifest.id;
    let kind = manifest.kind;
    let website = manifest
        .description
        .trim()
        .strip_prefix("https://")
        .or_else(|| manifest.description.trim().strip_prefix("http://"))
        .unwrap_or(manifest.description.trim());
    let site_url = crate::extensions::site_url(&manifest.description);

    let mut title = row().gap(6.0).center_y().flex_wrap()
        .child(text(manifest.name.clone()).font(k.ty.body_medium).color(k.c.text_strong));
    if manifest.catalog {
        title = title.child(badge(k, "Catalog").key(format!("catalog-badge-{id}")));
    }
    let title = title.child(badge(k, format!("v{}", manifest.version)));
    let mut info = col().items_start().gap(6.0).grow(1.0).shrink().min_w(0.0).child(title);
    if !website.is_empty() && let Some(href) = site_url {
        let link = Link::new(0..website.len(), href)
            .hover(LinkPaint::new().color(k.c.text_base).underline(k.c.text_base));
        info = info.child(
            text(website.to_owned()).key(format!("site-{id}"))
                .font(k.ty.small.lh(20.0)).color(k.c.text_muted).truncate()
                .link(link)
                .on_link(|s: &mut App, url, tx| {
                    if let Err(error) = tx.open_url(url) {
                        s.error(format!("Could not open website: {error}"));
                    }
                }),
        );
    }
    if !kind.is_lookup() {
        let service_names = s.services_of(id, &manifest.services);
        if !service_names.is_empty() {
            info = info.child(row().key(format!("services-{id}")).gap(6.0).center_y().flex_wrap()
                .children(service_names.iter().map(|service| service_badge(s, k, service))));
        }
    }
    if let Some((reason, color)) = status_issue(s, k, id) {
        info = info.child(
            text(reason)
                .key(format!("status-detail-{id}"))
                .font(k.ty.text_xsmall)
                .color(color)
                .wrap(),
        );
    }

    let header = row().key("header").w_full().gap(14.0).center_y()
        .child(kind_tile(k, id, kind))
        .child(info)
        .child(card_actions(s, k, installed));
    let mut body = col().w_full().child(header);
    if !manifest.options.is_empty() && s.extension_settings.as_deref() == Some(id) {
        let mut options = col().key(format!("extension-options-{id}")).w_full().gap(8.0);
        for option in &manifest.options {
            options = options.child(
                row().w_full().gap(12.0).center_y().flex_wrap()
                    .child(text(widgets::title_case_label(&option.label)).font(k.ty.small_medium).color(k.c.text_strong).grow(1.0))
                    .child(widgets::extension_option(k, s, id, option, &format!("setting-{id}-{}", option.key))),
            );
        }
        body = body.child(col().key("settings-panel").w_full().pt(12.0).gap(12.0)
            .child(Divider::horizontal().build(k))
            .child(options));
    }
    card(k, format!("ext-{id}"), body)
}

fn kind_tile(k: &Kit, id: &str, kind: Kind) -> El {
    let (glyph, light, dark) = match kind {
        Kind::Downloader => (&icons::CLOUD_DOWNLOAD, ["#b9d7ff", "#003978"], ["#253c5a", "#a1caff"]),
        Kind::Resolver => (&icons::SHARE_2, ["#a9e2d6", "#00463d"], ["#28413c", "#81c9bb"]),
        Kind::Lyrics => (&icons::MIC_VOCAL, ["#f7d5ff", "#5c0e6c"], ["#4a2f51", "#eaacf8"]),
        Kind::Metadata => (&icons::TAG, ["#fae19e", "#4b3900"], ["#453a1c", "#e3c36c"]),
    };
    let [fill, ink] = if k.dark { dark } else { light }.map(|hex| rune::color::hex(hex).expect("extension accent"));
    el().key(format!("kind-{id}"))
        .role("img")
        .label(kind.label())
        .help(kind.label())
        .size(44.0, 44.0)
        .radius(k.radius.md)
        .fill(fill)
        .center()
        .child(icon(glyph, 22.0).key(glyph.name).color(ink).a11y_hidden())
}

fn service_badge(s: &App, k: &Kit, name: &str) -> El {
    let service = services::by_name(name);
    if let Some(service) = service
        && let Some(logo) = widgets::provider_logo(s, service, 14.0)
    {
        return badge_with_icon(k, service.name, logo);
    }
    service_chip(k, service, name)
}

fn card_actions(s: &App, k: &Kit, installed: &Installed) -> El {
    let id = &installed.manifest.id;
    if s.removing.contains(id) {
        return row().gap(6.0).center_y()
            .child(Spinner::new().size(14.0).build(k))
            .child(text("Removing…").font(k.ty.small).color(k.c.text_muted));
    }
    let confirming = s.confirm_action.as_ref() == Some(&ConfirmAction::RemoveExtension(id.clone()));
    let remove_id = id.to_owned();
    let disabled = s.importing > 0 || s.running_extension(id) || s.status_command_running(installed);
    let remove = if confirming {
        widgets::button(k, &format!("remove-{id}"), "Confirm", Some(&icons::TRASH_2), ButtonVariant::Danger, move |s: &mut App| {
            s.confirm_action = None;
            s.remove(&remove_id);
        })
        .disabled(disabled)
    } else {
        widgets::danger_icon_button(k, &format!("remove-{id}"), &icons::TRASH_2, "Remove", move |s: &mut App| {
            s.confirm_action = Some(ConfirmAction::RemoveExtension(remove_id.clone()));
        })
        .disabled(disabled)
    };
    let mut actions = row().gap(8.0).center_y();
    if !installed.manifest.options.is_empty() {
        let id = id.clone();
        let expanded = s.extension_settings.as_deref() == Some(&id);
        let mut gear = widgets::icon_button(k, &format!("configure-{id}"), &icons::SETTINGS, "Extension Settings", move |s: &mut App| {
            s.extension_settings = (s.extension_settings.as_deref() != Some(&id)).then(|| id.clone());
        })
        .expanded(expanded);
        if expanded {
            gear = gear.fill(k.c.layer_04);
        }
        actions = actions.child(gear);
    }
    actions
        .child(online_status(s, k, id))
        .child(remove)
}

fn status_issue(s: &App, k: &Kit, id: &str) -> Option<(String, rune::Color)> {
    if s.is_checking(id) {
        return None;
    }
    match s.extension_status.get(id) {
        Some(ExtensionStatus::Offline(reason)) if !reason.trim().is_empty() => Some((reason.clone(), k.c.danger.fg)),
        Some(ExtensionStatus::Unknown(reason)) if !reason.trim().is_empty() => Some((reason.clone(), k.c.warning.fg)),
        _ => None,
    }
}

fn online_status(s: &App, k: &Kit, id: &str) -> El {
    let checking = s.is_checking(id);
    let (glyph, label, colors, detail) = if checking {
        (&icons::CLOCK, "Checking…", None, "Checking online status.")
    } else {
        match s.extension_status.get(id) {
            Some(ExtensionStatus::Online) => (&icons::CIRCLE_CHECK, "Online", Some(k.c.success), "The status endpoint responded successfully."),
            Some(ExtensionStatus::Degraded) => (&icons::TRIANGLE_ALERT, "Degraded", Some(k.c.warning), "The endpoint reports degraded service."),
            Some(ExtensionStatus::Offline(reason)) => (&icons::CIRCLE_X, "Offline", Some(k.c.danger), reason.as_str()),
            Some(ExtensionStatus::Unknown(reason)) => (&icons::TRIANGLE_ALERT, "Status Unknown", Some(k.c.warning), reason.as_str()),
            Some(ExtensionStatus::Services(services)) if services.is_empty() =>
                (&icons::CIRCLE_X, "Offline", Some(k.c.danger), "The extension reported no online services."),
            Some(ExtensionStatus::Services(_)) => (&icons::CIRCLE_CHECK, "Online", Some(k.c.success), "The extension reported online services."),
            None => (&icons::CLOCK, "Not Checked", None, "Online status has not been checked yet."),
        }
    };
    let can_recheck = !checking && s.importing == 0 && !s.removing.contains(id);
    let help = if can_recheck { format!("{detail} Click to check again.") } else { detail.to_owned() };
    let mut badge = status_badge(k, glyph, label, colors).key(format!("online-status-{id}")).help(help);
    if can_recheck {
        let id = id.to_owned();
        badge = badge
            .role("button")
            .focusable(true)
            .focus_ring(2.0, 1.0, k.c.border_focus)
            .cursor(Cursor::Pointer)
            .on_press(move |s: &mut App| s.check_extension(&id));
    }
    badge
}
