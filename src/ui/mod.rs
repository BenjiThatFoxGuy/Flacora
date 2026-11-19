//! The window: a sidebar with the five pages, banners, and the drop overlay.

mod catalog;
mod downloads;
mod extensions;
mod library;
mod settings;
mod theme;
mod widgets;

pub use theme::FONT;

use crate::app::{App, Notification, NotificationKind, Page};
use crate::{icons, store};
use rune::{Chord, ChordKey, ChordMod, Cursor, Cx, El, Icon, Image, ImageCache, SurfaceFit, Transition, col, el, icon, row, text};
use rune_kit::{Divider, Kit, Sidebar, SidebarRow, Toast, ToastRegion};

pub const SIDEBAR: f32 = 232.0;
const SIDEBAR_ROW_HEIGHT: f32 = 36.0;
const PROJECT_ICON_SIZE: f32 = 18.0;

struct Project {
    id: &'static str,
    name: &'static str,
    website: &'static str,
    status: &'static str,
    logo: &'static [u8],
}

const PROJECTS: [Project; 2] = [
    Project {
        id: "spotbye",
        name: "SpotBye",
        website: "https://spotbye.vip/",
        status: "https://status.spotbye.vip/",
        logo: include_bytes!("../../assets/projects/spotbye.svg"),
    },
    Project {
        id: "stream-hoarder",
        name: "Stream Hoarder",
        website: "https://streamhoarder.app/",
        status: "https://status.streamhoarder.app/",
        logo: include_bytes!("../../assets/projects/stream-hoarder.svg"),
    },
];

pub fn project_logos(cache: &ImageCache) -> [Image; 2] {
    PROJECTS.map(|project| cache.svg(project.logo, PROJECT_ICON_SIZE, 3.0))
}

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let k = theme::kit(s.dark(cx.system_dark()));
    // Keep the region mounted while its last notification fades out.
    let mut overlays = vec![notifications(s, k)];
    let page = match s.page {
        Page::Downloads => downloads::page(s, k, &mut overlays),
        Page::Catalog => catalog::page(s, k),
        Page::Library => library::page(s, k),
        Page::Extensions => extensions::page(s, k, &mut overlays),
        Page::Settings => settings::page(s, k),
    };
    let content = col().key("content").grow(1.0).shrink().min_w(0.0).h_full().items_center().child(page);
    // Extension packages can be dropped anywhere on the window.
    let base = row()
        .key("window")
        .w_full()
        .h_full()
        .fill(k.c.page)
        .child(sidebar(s, k))
        .child(Divider::vertical().build(k))
        .child(content)
        .on_drop_files(|s: &mut App, files, _| s.install_files(files))
        .accepts_extensions(&store::PACKAGE_EXTENSIONS)
        .on_drop_hover(|s: &mut App, over, _| s.drop_over = over)
        .on_drop_rejected(|s: &mut App, files, _| s.reject_files(files));
    let empty_extensions = s.page == Page::Extensions && s.loaded && s.installed.is_empty();
    if s.drop_over && !empty_extensions {
        overlays.push(drop_overlay(k));
    }
    rune_kit::root(base, overlays)
}

fn sidebar(s: &App, k: &Kit) -> El {
    // The PNG has a transparent margin around its rounded square, so it draws a little larger.
    let logo = match s.logo.as_ref().filter(|l| l.error().is_none()) {
        Some(image) => el().key("logo").size(38.0, 38.0).pixels_fit(image.pixels(), rune::SurfaceFit::Fit),
        None => el()
            .key("logo")
            .size(32.0, 32.0)
            .radius(k.radius.md)
            .fill(k.c.bg_contrast)
            .center()
            .child(icon(&icons::DISC, 18.0).color(k.c.text_contrast)),
    };
    let brand = row()
        .key("brand")
        .gap(10.0)
        .center_y()
        .px(6.0)
        .child(logo)
        .child(
            col()
                .child(text("Flacora").font(theme::SUBHEADING).color(k.c.text_strong))
                .child(text(concat!("v", env!("CARGO_PKG_VERSION"))).font(k.ty.text_xsmall).color(k.c.text_faint)),
        );
    let active = s.active_jobs();
    let extensions = s.installed.len();
    let items: [(Page, &'static Icon, &str, Option<String>); 5] = [
        (Page::Downloads, &icons::DOWNLOAD, "Downloads", (active > 0).then(|| active.to_string())),
        (Page::Catalog, &icons::SEARCH, "Catalog", None),
        (Page::Library, &icons::LIBRARY, "Library", (!s.history.is_empty()).then(|| s.history.len().to_string())),
        (Page::Extensions, &icons::PUZZLE, "Extensions", s.loaded.then(|| extensions.to_string())),
        (Page::Settings, &icons::SETTINGS, "Settings", None),
    ];
    let mut sidebar = Sidebar::new().inset(12.0).header_padding(16.0, 16.0).row_gap(4.0).label("Pages").header(brand);
    for (index, (page, glyph, label, badge)) in items.into_iter().enumerate() {
        let selected = s.page == page;
        let digit = char::from_digit((index + 1) as u32, 10).expect("sidebar index digit");
        let shortcut = Chord::new(ChordKey::Char(digit)).with(ChordMod::Primary).label();
        let mut item = SidebarRow::new(label)
            .icon(glyph)
            .height(SIDEBAR_ROW_HEIGHT)
            .selected(selected)
            .title_style(if selected { k.ty.body_medium } else { k.ty.ui })
            .on_select(move |s: &mut App, _| {
                s.page = page;
                s.dismiss_page_overlays();
                if page == Page::Library {
                    s.refresh_library_files();
                }
            });
        if let Some(badge) = badge {
            item = item.badge(badge);
        }
        let mut item = item.build(k).key(format!("nav-{}", label.to_lowercase())).shortcut_hint(shortcut);
        if page == Page::Extensions && s.loaded {
            item = item.label(format!("{label}, {extensions} installed"));
        }
        if selected {
            item = item.fill(k.c.layer_04);
        }
        sidebar = sidebar.child(item);
    }
    let coffee = SidebarRow::new("Buy Me a Coffee")
        .icon(&icons::COFFEE)
        .height(SIDEBAR_ROW_HEIGHT)
        .title_style(k.ty.ui)
        .on_select(|s, tx| open_footer_url(s, tx, "https://afkarxyz.gumroad.com/coffee"))
        .build(k).key("buy-me-a-coffee").role("link");
    let mut footer = col().w_full().gap(2.0).pb(8.0);
    if let Some(release) = &s.available_update {
        let url = release.url.clone();
        footer = footer.child(row().h(28.0).center().radius(10.0)
            .key("sidebar-update").label("Update Available")
            .self_start().ml(6.0).mb(4.0)
            .role("link").focusable(true)
            .help(format!("Open Flacora v{} on GitHub", release.version))
            .cursor(Cursor::Pointer)
            .hover(k.c.hover)
            .focus_ring(theme::FOCUS_RING_WIDTH, theme::FOCUS_RING_OFFSET, k.c.border_focus)
            .on_press_tx(move |s, tx| open_footer_url(s, tx, url.as_str()))
            .child(widgets::status_badge(k, &icons::CIRCLE_CHECK, "Update Available", Some(k.c.success)).a11y_hidden()));
    }
    let footer = footer.child(coffee).child(project_shortcuts(s, k));
    sidebar.footer(footer).build(k).w(SIDEBAR).fill(k.c.bg_base)
}

fn project_shortcuts(s: &App, k: &Kit) -> El {
    let links = PROJECTS.iter().enumerate().map(|(i, project)| {
        let mut logo = el().size(PROJECT_ICON_SIZE, PROJECT_ICON_SIZE).a11y_hidden();
        if let Some(images) = &s.project_logos {
            logo = logo.pixels_fit(images[i].pixels(), SurfaceFit::Fit);
        }
        let website = footer_link(k, format!("project-{}", project.id), project.name.into(), project.website)
            .grow(1.0)
            .min_w(0.0)
            .pr(4.0)
            .gap(8.0)
            .child(logo)
            .child(text(project.name).font(k.ty.ui).color(k.c.text_base).truncate().a11y_hidden());
        let status = footer_link(k, format!("project-{}-status", project.id), format!("{} Server Status", project.name), project.status)
            .w(66.0)
            .min_w(66.0)
            .gap(4.0)
            .child(icon(&icons::ACTIVITY, 13.0).color(k.c.icon_muted).a11y_hidden())
            .child(text("Status").font(k.ty.small).color(k.c.text_muted).a11y_hidden());
        row().w_full().gap(4.0).center_y().child(website).child(status)
    });
    col().key("project-shortcuts").role("group").label("Projects").w_full().gap(2.0).children(links)
}

fn footer_link(k: &Kit, key: String, label: String, url: &'static str) -> El {
    row().key(key).role("link").label(label.clone()).help(label).focusable(true)
        .h(30.0).px(8.0).center_y().radius(k.radius.md)
        .cursor(Cursor::Pointer).hover(k.c.hover).focus_ring(theme::FOCUS_RING_WIDTH, theme::FOCUS_RING_OFFSET, k.c.border_focus)
        .on_press_tx(move |s, tx| open_footer_url(s, tx, url))
}

fn open_footer_url(s: &mut App, tx: &mut rune::Tx, url: &str) {
    if let Err(error) = tx.open_url(url) {
        s.error(format!("Could not open website: {error}"));
    }
}

fn notifications(s: &App, k: &Kit) -> El {
    let mut region = ToastRegion::new();
    if let Some(notice) = &s.notice {
        region = region.toast(notification(k, "notice", notice, false, |s| s.notice = None));
    }
    if let Some(info) = &s.info {
        region = region.toast(notification(k, "info", info, s.undo.is_some(), |s| s.dismiss_info()));
    }
    // Keep the overlay's origin fixed on the bottom-left of the content pane so right-aligned card actions stay clear.
    region.build(k).pin(Some(SIDEBAR + 24.0), Some(0.0), None, Some(24.0)).w(340.0).justify_end()
}

fn notification(k: &Kit, key: &str, note: &Notification, has_undo: bool, dismiss: fn(&mut App)) -> El {
    let (title, glyph, colors) = match note.kind {
        NotificationKind::Success => ("Success", &icons::CIRCLE_CHECK, k.c.success),
        NotificationKind::Warning => ("Warning", &icons::TRIANGLE_ALERT, k.c.warning),
        NotificationKind::Error => ("Error", &icons::CIRCLE_X, k.c.danger),
    };
    let status_kit = theme::toast_kit(k.dark, note.kind);

    let mut toast = Toast::new(key, title)
        .description(&note.message)
        .lead(icon(glyph, 16.0).color(colors.icon))
        .on_close(dismiss);
    if has_undo {
        toast = toast.action(status_kit, "Undo", true, |s: &mut App, _| s.undo_last());
    }
    toast
        .build(status_kit)
        .w(340.0)
        .border(1.0, colors.border.alpha(0.28))
        .radius(theme::CARD_RADIUS)
        .transition(Transition::opacity())
        .on_hover_change(|s: &mut App, inside, _| s.toast_hovered = inside)
}

fn drop_overlay(k: &Kit) -> El {
    let card = col()
        .key("drop-card")
        .w_full()
        .h_full()
        .gap(10.0)
        .center()
        .radius(theme::CARD_RADIUS)
        .border(2.0, k.c.border_focus)
        .border_dash(8.0, 4.0)
        .fill(k.c.bg_base.alpha(0.97))
        .child(
            el().size(56.0, 56.0)
                .radius(999.0)
                .fill(k.c.layer_02)
                .center()
                .child(icon(&icons::FILE_UP, 28.0).color(k.c.icon_base)),
        )
        .child(text("Drop to Install").font(theme::HEADING).color(k.c.text_strong))
        .child(text(format!("Flacora Extension Packages (.{})", store::PACKAGE)).font(k.ty.small).color(k.c.text_muted));
    // Absolute, so it covers the window instead of being laid out beside it.
    el().key("drop-overlay").absolute(0.0, 0.0).w_full().h_full().padding(28.0).fill(k.c.scrim).child(card)
}

#[cfg(test)]
mod tests;
