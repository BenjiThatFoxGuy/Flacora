//! Library: every finished download, newest first.

use super::widgets::{self, card, cover};
use crate::app::{App, LibrarySort, Page};
use crate::extensions::Kind;
use crate::icons;
use crate::jobs::now;
use crate::library::Entry;
use crate::services;
use rune::{Cursor, El, GrowthSpec, Tx, col, el, icon, row, text};
use rune_kit::{ButtonVariant, Collapsible, Divider, Kit, SegmentedControl, TextInput};
use std::{borrow::Cow, rc::Rc};

pub fn page(s: &App, k: &Kit) -> El {
    let mut actions = Vec::new();
    if !s.history.is_empty() {
        let mut search = TextInput::new(&s.search)
            .placeholder("Search the Library")
            .leading_icon(&icons::SEARCH)
            .width(Some(240.0))
            .height(32.0)
            .on_edit(|s: &mut App| &mut s.search);
        if !s.search.text.is_empty() {
            search = search.trailing(
                widgets::clear_input_button(k, "search", "Clear Search", |s| &mut s.search)
                    .key("library-clear-search"),
            );
        }
        actions.push(search.build(k).key("search"));
        actions.push(widgets::button(k, "clear-history", "Clear", Some(&icons::TRASH_2), ButtonVariant::Danger, |s: &mut App| s.clear_history()));
    }
    let entries = s.filtered_history();
    let count = s.history.len();
    let filtering = !s.search.text.trim().is_empty() || s.active_library_service().is_some();
    let subtitle = if count == 0 {
        "Your finished downloads appear here.".to_string()
    } else if filtering {
        format!("{} of {count} Download{}", entries.len(), if count == 1 { "" } else { "s" })
    } else if count == 1 {
        "1 Download".to_string()
    } else {
        format!("{count} Downloads")
    };
    let head = widgets::page_header(k, "Library", &subtitle, actions);
    let body = if s.history.is_empty() {
        widgets::empty_state(
            k,
            "library-empty",
            &icons::LIBRARY,
            "No Downloads Yet",
            "Finished downloads are listed here with their artwork, so you can play them or find them again.",
            Some(widgets::button(k, "go-downloads", "Start Downloading", Some(&icons::DOWNLOAD), ButtonVariant::Submit, |s: &mut App| {
                s.page = Page::Downloads
            })),
        )
    } else {
        list(s, k, entries)
    };
    let mut page = widgets::page("library-page").child(head);
    if s.history.len() > 1 {
        page = page.child(toolbar(s, k));
    }
    page.child(body)
}

fn toolbar(s: &App, k: &Kit) -> El {
    let mut services: Vec<String> = Vec::new();
    for entry in &s.history {
        let name = entry.service.trim();
        if !name.is_empty() && !services.iter().any(|seen| seen.eq_ignore_ascii_case(name)) {
            services.push(name.to_owned());
        }
    }
    let mut filters = widgets::filter_group("library-filters", "Library Service").grow(1.0).min_w(0.0);
    let active = s.active_library_service();
    if services.len() > 1 || active.is_some() {
        filters = filters.child(widgets::filter_button(
            k, "library-filter-all".into(), "All".into(), active.is_none(), None,
            |s, _| s.library_service = None,
        ));
        for name in services {
            let selected = active.is_some_and(|v| v.eq_ignore_ascii_case(&name));
            let key_slug = name.to_lowercase().replace(' ', "-");
            let value = name.clone();
            filters = filters.child(widgets::filter_button(
                k, format!("library-filter-{key_slug}"), name, selected, None,
                move |s, _| s.library_service = Some(value.clone()),
            ));
        }
    }
    let sort_labels = LibrarySort::ALL.map(LibrarySort::label);
    let current = LibrarySort::ALL.iter().position(|sort| *sort == s.library_sort);
    let sort = widgets::segmented(
        k,
        "library-sort",
        SegmentedControl::new(&sort_labels, current)
            .label("Sort Library")
            .on_pick(|s: &mut App, i: usize, _: &mut Tx| s.library_sort = LibrarySort::ALL[i]),
    );
    row().key("library-toolbar").w_full().gap(10.0).center_y().flex_wrap().child(filters).child(sort)
}

fn list(_s: &App, k: &Kit, entries: Vec<(usize, &Entry)>) -> El {
    if entries.is_empty() {
        return widgets::empty_state(k, "library-no-match", &icons::SEARCH, "No Matches", "Nothing in the library matches that search.", None);
    }
    let now = now();
    let dark = k.dark;
    let indices: Rc<Vec<usize>> = Rc::new(entries.into_iter().map(|(index, _)| index).collect());
    let key_indices = indices.clone();
    let build_indices = indices.clone();
    let rows = col().key("library-rows").w_full().growth(GrowthSpec::default()).virtual_rows(
        indices.len(),
        65.0,
        move |s: &App, position| {
            let index = key_indices[position];
            let date = s.history.get(index).map_or(0, |entry| entry.date);
            Cow::Owned(format!("library-row-{date}-{index}"))
        },
        move |s: &App, position| {
            let k = super::theme::kit(dark);
            let index = build_indices[position];
            let Some(entry) = s.history.get(index) else {
                return el();
            };
            let mut wrap = col().w_full();
            if position > 0 {
                wrap = wrap.child(Divider::horizontal().build(k));
            }
            wrap.child(entry_row(s, k, index, entry, now))
        },
    );
    // The card keeps its own height; the column around it scrolls.
    widgets::page_scroll(k, col().key("library-scroll").child(card(k, "library-list", rows)))
}

fn entry_row(s: &App, k: &Kit, index: usize, entry: &Entry, now: u64) -> El {
    let mut subtitle = vec![entry.artist.clone()];
    if !entry.album.is_empty() {
        subtitle.push(entry.album.clone());
    }
    subtitle.retain(|t| !t.is_empty());
    let mut info = col().gap(3.0).grow(1.0).shrink().min_w(0.0).child(text(entry.title.clone()).font(k.ty.body_medium).color(k.c.text_strong).truncate());
    if !subtitle.is_empty() {
        info = info.child(text(subtitle.join(" · ")).font(k.ty.small).color(k.c.text_muted).truncate());
    }
    let file_info = entry.main_file().and_then(|file| s.library_files.get(file));
    let missing = file_info.is_some_and(|info| !info.exists);
    let mut meta = row().gap(8.0).center_y();
    if missing {
        meta = meta.child(
            widgets::status_badge(k, &icons::TRIANGLE_ALERT, "Missing", Some(k.c.warning))
                .key(format!("missing-entry-{index}"))
                .help("Audio file was moved or deleted from disk."),
        );
    }
    if let Some(info) = file_info {
        for (label, sidecar_path) in &info.sidecars {
            let path = sidecar_path.clone();
            let slug = label.to_lowercase();
            let filename = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            meta = meta.child(
                row()
                    .key(format!("sidecar-{index}-{slug}"))
                    .role("button")
                    .label(format!("Open {label}"))
                    .help(format!("Open {filename}"))
                    .focusable(true)
                    .focus_ring(super::theme::FOCUS_RING_WIDTH, super::theme::FOCUS_RING_OFFSET, k.c.border_focus)
                    .cursor(Cursor::Pointer)
                    .h(20.0)
                    .px(7.0)
                    .center()
                    .radius(999.0)
                    .fill(k.c.layer_02)
                    .hover(k.c.layer_04)
                    .on_press(move |s: &mut App| s.open(&path))
                    .child(text(label.clone()).font(k.ty.text_xsmall_medium).color(k.c.text_base).a11y_hidden()),
            );
        }
    }
    if !entry.quality.is_empty() {
        meta = meta.child(text(entry.quality.clone()).font(k.ty.small).color(k.c.text_muted).truncate());
    }
    let meta = meta.child(source_icon(s, k, entry));
    let mut actions = row().gap(2.0).center_y();
    let has_lookups = s.installed.iter().any(|i| i.manifest.supports_lookup());
    let lookup_target = entry.flac_file().filter(|_| has_lookups && !missing).map(|file| (entry.date, file.to_owned()));
    let lookup_open = lookup_target.is_some() && lookup_target == s.lookup_picker;
    if let Some(target) = lookup_target {
        let mut trigger = widgets::icon_button(k, &format!("lookup-{index}"), &icons::TAG, "Lyrics & Metadata", move |s: &mut App| {
            s.lookup_picker = (s.lookup_picker.as_ref() != Some(&target)).then(|| target.clone());
        })
        .expanded(lookup_open);
        if lookup_open {
            trigger = trigger.fill(k.c.layer_04);
        }
        actions = actions.child(trigger);
    }
    if let Some(file) = entry.main_file() {
        let (play, reveal) = (file.to_owned(), file.to_owned());
        actions = actions
            .child(widgets::icon_button(k, &format!("play-entry-{index}"), &icons::PLAY, "Play", move |s: &mut App| s.open(&play)).disabled(missing))
            .child(widgets::icon_button(k, &format!("reveal-entry-{index}"), &icons::FOLDER, "Show in Folder", move |s: &mut App| {
                s.reveal(&reveal)
            }));
    }
    actions = actions.child(widgets::danger_icon_button(k, &format!("remove-entry-{index}"), &icons::X, "Remove from Library", move |s: &mut App| {
        s.remove_entry(index)
    }));
    let mut artwork = cover(k, s.image(entry.cover.as_deref()), 44.0);
    if let Some(path) = entry.cover.as_deref()
        && s.image(Some(path)).is_none()
    {
        let path = path.to_owned();
        artwork = artwork.on_visible(move |s: &mut App, visible, _| {
            if visible {
                s.request_history_cover(&path);
            }
        });
    }
    let head = row()
        .key(format!("entry-{index}"))
        .w_full()
        .px(4.0)
        .py(10.0)
        .gap(14.0)
        .center_y()
        .child(artwork)
        .child(info)
        .child(meta)
        .child(text(widgets::ago(entry.date, now)).font(k.ty.text_xsmall).color(k.c.text_faint).w(76.0).truncate())
        .child(actions);
    if lookup_open { col().w_full().child(head).child(lookup_providers(s, k, index)) } else { head }
}

fn source_icon(s: &App, k: &Kit, entry: &Entry) -> El {
    let name = entry.service.trim();
    let service = services::by_name(name);
    let label = service.map_or(if name.is_empty() { "Audio Source" } else { name }, |service| service.name);
    service.and_then(|service| widgets::provider_logo(s, service, 16.0))
        .unwrap_or_else(|| icon(&icons::LINK, 16.0).key("source-link").color(k.c.icon_muted))
        .role("img")
        .label(label.to_owned())
        .help(label.to_owned())
}

fn lookup_providers(s: &App, k: &Kit, index: usize) -> El {
    let mut lyrics = Vec::new();
    let mut tags = Vec::new();
    let mut json = Vec::new();
    let mut metadata = Vec::new();
    let mut updates = Vec::new();
    for item in s.installed.iter().filter(|item| item.manifest.supports_lookup()) {
        let id = item.manifest.id.clone();
        let key = format!("lookup-{index}-{id}");
        let label = item.manifest.name.strip_suffix(" Web").unwrap_or(&item.manifest.name);
        let (group, help) = if item.manifest.kind == Kind::Lyrics {
            (&mut lyrics, format!("Get lyrics from {label}"))
        } else if let Some(message) = item.manifest.tag_writer_update() {
            (&mut updates, message)
        } else if item.manifest.can_write_tags() {
            (&mut tags, format!("Write {label} metadata to this FLAC"))
        } else if id == "musicbrainz" {
            (&mut json, "Export MusicBrainz metadata as a JSON file".into())
        } else {
            (&mut metadata, format!("Export {label} metadata"))
        };
        let disabled = s.importing > 0 || s.removing.contains(&id);
        group.push(widgets::button(k, &key, label, None, ButtonVariant::Outline,
            move |s: &mut App| s.enqueue_lookup(index, &id),
        ).disabled(disabled).help(help));
    }

    let mut providers = col().w_full().gap(8.0);
    let mut first = true;
    for (title, glyph, buttons) in [
        ("Get Lyrics", &icons::MIC_VOCAL, lyrics),
        ("Tag FLAC", &icons::TAG, tags),
        ("Export JSON", &icons::FILE_UP, json),
        ("Export Metadata", &icons::FILE_UP, metadata),
        ("Update Required", &icons::TRIANGLE_ALERT, updates),
    ] {
        if buttons.is_empty() {
            continue;
        }
        if !first {
            providers = providers.child(Divider::horizontal().build(k));
        }
        first = false;
        let label = row().w(112.0).h(32.0).gap(6.0).center_y()
            .child(icon(glyph, 13.0).color(k.c.icon_muted))
            .child(text(title).font(k.ty.small_medium).color(k.c.text_muted));
        providers = providers.child(row().role("group").label(title).w_full().gap(8.0).items_start()
            .child(label).child(row().gap(6.0).center_y().flex_wrap().grow(1.0).min_w(0.0).children(buttons)));
    }
    let disclosure = Collapsible::new("Lyrics & Metadata", true)
        .on_toggle_tx(move |s: &mut App, tx: &mut Tx| {
            s.lookup_picker = None;
            tx.focus_when_shown(format!("lookup-{index}"), false);
        })
        .child(providers.pt(6.0))
        .build(k).key(format!("lookup-disclosure-{index}"));
    let panel = col()
        .key(format!("lookup-providers-{index}"))
        .w_full()
        .padding(10.0)
        .radius(k.radius.md)
        .fill(k.c.bg_deep)
        .border(1.0, k.c.border_base)
        .child(disclosure);
    col().w_full().px(4.0).pb(10.0).child(panel)
}
