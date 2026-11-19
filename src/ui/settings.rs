//! Settings: download folder, appearance, default extensions, helpers, storage.

use super::widgets::{self, card, section};
use crate::app::{App, RESOLVER_KEY};
use crate::extensions::Kind;
use crate::icons;
use crate::output::{Filename, Folders, Layout};
use crate::services::SERVICES;
use crate::store::{self, Theme};
use rune::{Color, Cursor, El, FileDialogOptions, Icon, Tx, col, el, icon, row, text};
use rune_kit::{Button, ButtonVariant, ControlSize, Divider, Kit, SegmentedControl};

pub fn page(s: &App, k: &Kit) -> El {
    let head = widgets::page_header(k, "Settings", "Where downloads go, how Flacora looks, and which extension handles each service.", vec![]);
    let body = col()
        .key("settings-list")
        .gap(10.0)
        .child(section(k, "Downloads", None))
        .child(group(k, "group-downloads", vec![folder_row(s, k), filename_row(s, k), folders_row(s, k), preview_row(s, k)]))
        .child(section(k, "Appearance", None))
        .child(group(k, "group-appearance", vec![theme_row(s, k)]))
        .child(section(k, "Default Extension", None))
        .child(group(k, "group-defaults", default_rows(s, k)))
        .child(
            row()
                .w_full()
                .center_y()
                .justify_between()
                .child(section(k, "Helpers", None))
                .child(widgets::icon_button(k, "refresh-tools", &icons::REFRESH_CW, "Refresh Helpers", |s: &mut App| s.refresh_tools())),
        )
        .child(group(k, "group-helpers", vec![ffmpeg_row(s, k), browser_row(s, k)]))
        .child(section(k, "Storage", None))
        .child(group(k, "group-storage", vec![data_row(s, k), history_row(s, k)]));
    widgets::page("settings-page").child(head).child(widgets::page_scroll(k, body))
}

fn group(k: &Kit, key: &str, rows: Vec<El>) -> El {
    let mut body = col();
    let last = rows.len().saturating_sub(1);
    for (i, row) in rows.into_iter().enumerate() {
        body = body.child(row);
        if i < last {
            body = body.child(Divider::horizontal().build(k));
        }
    }
    card(k, key, body)
}

/// A setting: title and explanation on the left, its control on the right.
fn setting(k: &Kit, key: &str, title: &str, detail: El, control: El) -> El {
    row()
        .key(key.to_owned())
        .w_full()
        .px(4.0)
        .py(10.0)
        .gap(16.0)
        .center_y()
        .flex_wrap()
        .child(
            col().gap(3.0).w(168.0).grow(1.0).shrink().min_w(168.0)
                .child(text(title.to_owned()).font(k.ty.ui_medium).color(k.c.text_strong))
                .child(detail),
        )
        .child(control)
}

fn muted(k: &Kit, t: impl Into<String>) -> El {
    text(t.into()).font(k.ty.small).color(k.c.text_muted).wrap()
}

fn folder_row(s: &App, k: &Kit) -> El {
    let change = Button::new("Change…")
        .icon(&icons::FOLDER)
        .variant(ButtonVariant::Neutral)
        .control_size(ControlSize::Regular)
        .on_press_tx(|s: &mut App, tx: &mut Tx| {
            let options = FileDialogOptions { title: "Save Downloads To".into(), directory: Some(s.settings.output.clone()), ..Default::default() };
            tx.open_folder(options, |s: &mut App, result, _: &mut Tx| match result {
                Ok(Some(path)) => s.set_output(path),
                Ok(None) => {}
                Err(e) => s.error(format!("Folder picker: {e}")),
            });
        })
        .build(k)
        .h(32.0)
        .key("change-output");
    let open = widgets::icon_button(k, "open-output", &icons::SQUARE_ARROW_UP_RIGHT, "Open Folder", |s: &mut App| s.open_output());
    let mut controls = row().gap(4.0).center_y().child(change);
    if s.settings.output != store::default_output() {
        controls = controls.child(widgets::icon_button(k, "reset-output", &icons::ROTATE_CCW, "Reset to Default Folder", |s: &mut App| {
            s.set_output(store::default_output())
        }));
    }
    controls = controls.child(open);
    setting(
        k,
        "setting-folder",
        "Download Folder",
        text(s.settings.output.to_string_lossy().into_owned()).font(k.ty.mono).color(k.c.text_muted).truncate(),
        controls,
    )
}

fn filename_row(s: &App, k: &Kit) -> El {
    let labels = Filename::ALL.map(Filename::label);
    let selected = Filename::ALL.iter().position(|value| *value == s.settings.layout.filename);
    setting(k, "setting-filename", "File Name", muted(k, "Choose how new audio files are named."),
        widgets::segmented(k, "filename", SegmentedControl::new(&labels, selected)
            .on_pick(|s: &mut App, i: usize, _: &mut Tx| {
                s.set_output_layout(Layout { filename: Filename::ALL[i], ..s.settings.layout });
            })))
}

fn folders_row(s: &App, k: &Kit) -> El {
    let labels = Folders::ALL.map(Folders::label);
    let selected = Folders::ALL.iter().position(|value| *value == s.settings.layout.folders);
    setting(k, "setting-folders", "Folder Organization", muted(k, "Group new downloads inside the selected folder."),
        widgets::segmented(k, "folders", SegmentedControl::new(&labels, selected)
            .on_pick(|s: &mut App, i: usize, _: &mut Tx| {
                s.set_output_layout(Layout { folders: Folders::ALL[i], ..s.settings.layout });
            })))
}

fn preview_row(s: &App, k: &Kit) -> El {
    let example = layout_preview(&s.settings.output, s.settings.layout);
    row()
        .key("setting-preview")
        .w_full()
        .px(4.0)
        .py(8.0)
        .gap(10.0)
        .center_y()
        .child(text("Path Preview").font(k.ty.small_medium).color(k.c.text_muted))
        .child(
            el().grow(1.0)
                .min_w(0.0)
                .px(10.0)
                .py(6.0)
                .radius(k.radius.xs)
                .fill(k.c.bg_deep)
                .border(1.0, k.c.border_base)
                .child(text(example).key("layout-preview").font(k.ty.mono).color(k.c.text_base).truncate()),
        )
}

fn layout_preview(output: &std::path::Path, layout: Layout) -> String {
    let file = match layout.filename {
        Filename::ExtensionDefault | Filename::ArtistTitle => "HUNTR_X - Golden.flac",
        Filename::TitleArtist => "Golden - HUNTR_X.flac",
        Filename::Title => "Golden.flac",
    };
    let path = match layout.folders {
        Folders::Single => output.join(file),
        Folders::Artist => output.join("HUNTR_X").join(file),
        Folders::ArtistAlbum => output.join("HUNTR_X").join("KPop Demon Hunters").join(file),
    };
    path.to_string_lossy().into_owned()
}

fn theme_row(s: &App, k: &Kit) -> El {
    let labels: Vec<&str> = Theme::ALL.iter().map(|t| t.label()).collect();
    let current = Theme::ALL.iter().position(|t| *t == s.settings.theme);
    setting(
        k,
        "setting-theme",
        "Theme",
        muted(k, "Follow the system, or always use light or dark."),
        widgets::segmented(k, "theme", SegmentedControl::new(&labels, current)
            .on_pick(|s: &mut App, i: usize, _: &mut Tx| s.set_theme(Theme::ALL[i])))
            .w(248.0),
    )
}

/// One row per service that two or more installed extensions can handle.
fn default_rows(s: &App, k: &Kit) -> Vec<El> {
    let mut rows = Vec::new();
    for service in &SERVICES[..crate::services::LOSSLESS] {
        let candidates = s.installed_downloaders_for(service.primary_host());
        if candidates.len() < 2 {
            continue;
        }
        let mut labels = vec!["Automatic".to_string()];
        labels.extend(candidates.iter().map(|i| i.manifest.name.clone()));
        let ids: Vec<String> = candidates.iter().map(|i| i.manifest.id.clone()).collect();
        let current = match s.settings.preferred.get(service.key) {
            Some(id) => ids.iter().position(|i| i == id).map(|p| p + 1).unwrap_or(0),
            None => 0,
        };
        let key = service.key.to_owned();
        rows.push(setting(
            k,
            &format!("setting-default-{}", service.key),
            service.name,
            muted(k, format!("{} extensions can download from {}.", candidates.len(), service.name)),
            default_picker(k, &format!("default-{}", service.key), &labels, current, ids, key),
        ));
    }
    // The resolver to use when two or more are installed.
    let mut resolvers: Vec<_> = s.installed.iter().filter(|i| i.manifest.kind == Kind::Resolver).collect();
    resolvers.sort_by_key(|i| (crate::extensions::rank(&i.manifest.id), i.manifest.name.to_lowercase()));
    if resolvers.len() > 1 {
        let mut labels = vec!["Automatic".to_string()];
        labels.extend(resolvers.iter().map(|i| i.manifest.name.clone()));
        let ids: Vec<String> = resolvers.iter().map(|i| i.manifest.id.clone()).collect();
        let current = match s.settings.preferred.get(RESOLVER_KEY) {
            Some(id) => ids.iter().position(|i| i == id).map(|p| p + 1).unwrap_or(0),
            None => 0,
        };
        rows.push(setting(
            k,
            "setting-default-resolver",
            Kind::Resolver.label(),
            muted(k, "Finds Spotify, Apple Music and YouTube tracks on a service you can download from.".to_owned()),
            default_picker(k, "default-resolver", &labels, current, ids, RESOLVER_KEY.to_owned()),
        ));
    }
    if rows.is_empty() {
        rows.push(setting(
            k,
            "setting-defaults-empty",
            "One Extension per Service",
            muted(k, "When two installed extensions can download from the same service, choose which one to use here."),
            el(),
        ));
    }
    rows
}

/// Uses a segmented control for 2-3 options, and wraps cleanly as pills when many extensions share a service.
fn default_picker(k: &Kit, key: &str, labels: &[String], current: usize, ids: Vec<String>, pref_key: String) -> El {
    if labels.len() <= 3 {
        return widgets::segmented(
            k,
            key,
            SegmentedControl::new(labels, Some(current))
                .on_pick(move |s: &mut App, i: usize, _: &mut Tx| s.set_preferred(&pref_key, i.checked_sub(1).and_then(|i| ids.get(i).cloned()))),
        );
    }
    let mut group = row()
        .key(key.to_owned())
        .role("radiogroup")
        .gap(4.0)
        .flex_wrap()
        .max_w(520.0);
    for (i, label) in labels.iter().enumerate() {
        let selected = i == current;
        let pref_key = pref_key.clone();
        let chosen = i.checked_sub(1).and_then(|idx| ids.get(idx).cloned());
        let pill = row()
            .key(format!("segment{i}"))
            .role("radio")
            .label(label.clone())
            .focusable(true)
            .focus_ring(2.0, 1.0, k.c.border_focus)
            .cursor(Cursor::Pointer)
            .h(26.0)
            .px(10.0)
            .center()
            .radius(k.radius.xs)
            .fill(if selected { k.c.surface_raised_base } else { Color::TRANSPARENT })
            .hover(if selected { k.c.surface_raised_base } else { k.c.hover })
            .on_press(move |s: &mut App| s.set_preferred(&pref_key, chosen.clone()))
            .child(
                text(label.clone())
                    .font(if selected { k.ty.small_medium } else { k.ty.small })
                    .color(if selected { k.c.text_strong } else { k.c.text_muted })
                    .a11y_hidden(),
            );
        group = group.child(pill);
    }
    row()
        .key(format!("{key}-track"))
        .padding(4.0)
        .radius(k.radius.md)
        .fill(k.c.layer_01)
        .child(group)
}

fn ffmpeg_row(s: &App, k: &Kit) -> El {
    let (detail, status, glyph, color) = match s.tools.as_ref().map(|t| t.ffmpeg.clone()) {
        None => ("Checking…".to_string(), "Checking", &icons::CLOCK, k.c.text_muted),
        Some(Some(version)) => (version, "Ready", &icons::CIRCLE_CHECK, k.c.success.fg),
        Some(None) if store::CAN_DOWNLOAD_FFMPEG => (
            "Extensions that need FFmpeg download FFmpeg and ffprobe on first use.".into(), "On Demand", &icons::DOWNLOAD, k.c.text_muted,
        ),
        Some(None) => ("Install FFmpeg and ffprobe and add them to PATH.".into(), "Missing", &icons::TRIANGLE_ALERT, k.c.warning.fg),
    };
    setting(k, "setting-ffmpeg", "FFmpeg", muted(k, detail), helper_status(k, status, glyph, color))
}

fn browser_row(s: &App, k: &Kit) -> El {
    let (detail, status, glyph, color) = match s.tools.as_ref().map(|t| t.browser.as_ref()) {
        None => ("Checking for CloakBrowser…".into(), "Checking", &icons::CLOCK, k.c.text_muted),
        Some(Some(version)) => (version.clone(), "Ready", &icons::CIRCLE_CHECK, k.c.success.fg),
        Some(None) => (
            "CloakBrowser downloads the first time an extension needs it.".into(),
            "On Demand", &icons::DOWNLOAD, k.c.text_muted,
        ),
    };
    setting(k, "setting-browser", "Browser for Protected Sites", muted(k, detail), helper_status(k, status, glyph, color))
}

fn helper_status(k: &Kit, status: &str, glyph: &'static Icon, color: Color) -> El {
    row().gap(4.0).center_y().child(icon(glyph, 14.0).color(color)).child(text(status.to_owned()).font(k.ty.small_medium).color(color))
}

fn data_row(s: &App, k: &Kit) -> El {
    let root = s.paths.root.clone();
    let open = widgets::icon_button(k, "open-data", &icons::SQUARE_ARROW_UP_RIGHT, "Open Folder", move |s: &mut App| {
        s.open(&root.to_string_lossy())
    });
    setting(
        k,
        "setting-data",
        "Data Folder",
        text(s.paths.root.to_string_lossy().into_owned()).font(k.ty.mono).color(k.c.text_muted).truncate(),
        open,
    )
}

fn history_row(s: &App, k: &Kit) -> El {
    let count = s.history.len();
    let control = if count > 0 {
        widgets::button(k, "settings-clear-history", "Clear", Some(&icons::TRASH_2), ButtonVariant::Danger, |s: &mut App| s.clear_history())
    } else {
        el()
    };
    setting(
        k,
        "setting-history",
        "Library History",
        muted(k, format!("{count} download{} remembered. Clearing it keeps the files.", if count == 1 { "" } else { "s" })),
        control,
    )
}
