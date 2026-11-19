use super::*;
use crate::{fixtures, jobs::Status};
use std::path::PathBuf;

#[test]
fn automatic_tagging_is_off_until_enabled() {
    let mut rt = runtime(fixtures::auto_tag_available_settings);
    let check_size = |rt: &Runtime<App>, thumb_size| {
        let elements = rt.elements();
        let track = elements.iter().find(|el| el.path.ends_with("/auto-tag/track")).unwrap();
        let thumb = elements.iter().find(|el| el.path.ends_with("/auto-tag/track/thumb")).unwrap();
        assert_eq!((track.rect[2], track.rect[3], track.radius), (40.0, 24.0, 12.0));
        assert_eq!((thumb.target[2], thumb.target[3]), (thumb_size, thumb_size));
    };
    assert!(!rt.state.settings.auto_tag);
    check_size(&rt, 16.0);
    tap(&mut rt, "auto-tag");
    assert!(rt.state.settings.auto_tag);
    check_size(&rt, 20.0);
    tap(&mut rt, "auto-tag");
    assert!(!rt.state.settings.auto_tag);
}

#[test]
fn the_tagging_switch_appears_only_when_a_compatible_extension_is_available() {
    let mut rt = runtime(fixtures::settings);
    assert!(rt.find("setting-auto-tag").is_none());

    let mut old = fixtures::installed("tidal-web");
    old.manifest.version = "1.0.0".into();
    rt.state.installed.push(old);
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    assert!(rt.find("setting-auto-tag").is_none());

    *rt.state.installed.last_mut().unwrap() = fixtures::installed("tidal-web");
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    assert!(rt.elements().iter().any(|el| el.path.contains("/setting-auto-tag/")
        && el.text.as_deref() == Some("Update tags and cover art on new FLACs with a matching Web extension.")));
    tap(&mut rt, "auto-tag");
    assert!(rt.state.settings.auto_tag);

    rt.state.removing.insert("tidal-web".into());
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    assert!(rt.find("setting-auto-tag").is_none());
    assert!(rt.state.settings.auto_tag);
}

#[test]
fn settings_change_file_naming_and_folder_organization() {
    let mut rt = runtime_at(fixtures::settings, (1280.0, 720.0));
    tap(&mut rt, "filename/segment2");
    tap(&mut rt, "folders/segment2");
    assert_eq!(rt.state.settings.layout, crate::output::Layout {
        filename: crate::output::Filename::TitleArtist, folders: crate::output::Folders::ArtistAlbum,
    });

    tap(&mut rt, "change-output");
    assert_eq!(rt.file_dialog().unwrap().0, rune::FileDialogKind::Folder);
    let folder = PathBuf::from("C:/Music/Organized");
    rt.resolve_file_dialog(Some(folder.clone())).unwrap();
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.settings.output, folder);
}

#[test]
fn settings_change_the_theme() {
    let mut rt = runtime(fixtures::settings);
    assert!(rt.elements().iter().any(|el| el.text.as_deref() == Some("Default Extension")));
    assert!(rt.elements().iter().any(|el| el.text.as_deref() == Some("Link Resolver")));
    let ([outer_x, outer_y, outer_w, outer_h], _) = rt.presented("theme-track").unwrap();
    let ([inner_x, inner_y, inner_w, inner_h], _) = rt.presented("theme").unwrap();
    assert_eq!((inner_x - outer_x, inner_y - outer_y, outer_w - inner_w, outer_h - inner_h), (4.0, 4.0, 8.0, 8.0));
    let (x, y) = rt.find("theme/segment1").unwrap();
    rt.pointer_move(x, y);
    for _ in 0..12 {
        rt.frame(1.0 / 60.0);
    }
    let elements = rt.elements();
    let indicator = elements.iter().find(|el| el.path.ends_with("/theme/indicator")).unwrap();
    let selected = elements.iter().find(|el| el.path.ends_with("/theme/segment0")).unwrap();
    let hovered = elements.iter().find(|el| el.path.ends_with("/theme/segment1")).unwrap();
    assert_eq!(selected.rect, indicator.rect);
    assert!(hovered.hover > 0.5);
    assert!((hovered.rect[0] - indicator.rect[0] - indicator.rect[2] - 4.0).abs() < 1.0);
    tap(&mut rt, "theme/segment2");
    assert_eq!(rt.state.settings.theme, crate::store::Theme::Dark);
}

#[test]
fn settings_explains_how_to_get_ffmpeg_when_it_is_missing() {
    let mut rt = runtime(fixtures::settings_without_ffmpeg);
    tap(&mut rt, "nav-downloads");
    tap(&mut rt, "nav-settings");
    assert!(rt.find("setting-ffmpeg").is_some());
    let expected = if store::CAN_DOWNLOAD_FFMPEG { "On Demand" } else { "Missing" };
    assert!(rt.elements().iter().any(|element| element.text.as_deref() == Some(expected)));
}

#[test]
fn installed_helpers_show_the_version_and_ready_status() {
    let mut rt = runtime(fixtures::home);
    tap(&mut rt, "nav-settings");
    let elements = rt.elements();
    for (path, label, version) in [
        ("/setting-ffmpeg/", "FFmpeg", "9.0"),
        ("/setting-browser/", "Browser for Protected Sites", "145.0.7632.159.7"),
    ] {
        let visible_text: Vec<&str> = elements.iter().filter(|el| el.path.contains(path)).filter_map(|el| el.text.as_deref()).collect();
        assert_eq!(visible_text, [label, version, "Ready"]);
    }
}

#[test]
fn settings_shows_browser_status_without_a_remove_action() {
    for (scene, status) in [
        (fixtures::settings as fn(&mut App), "Ready"),
        (fixtures::settings_without_browser, "On Demand"),
    ] {
        let mut rt = runtime(scene);
        tap(&mut rt, "nav-downloads");
        tap(&mut rt, "nav-settings");
        assert!(rt.elements().iter().any(|el| {
            el.path.contains("/setting-browser/") && el.text.as_deref() == Some(status)
        }));
        assert!(rt.find("remove-browser").is_none());
    }
}

#[test]
fn settings_has_a_bottom_gutter_at_the_end_of_the_list() {
    let mut rt = runtime(fixtures::settings);
    rt.wheel(760.0, 520.0, -2000.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.scroll_at_bottom("settings-list"), Some(true));
    let ([_, y, _, h], _) = rt.presented("group-storage").unwrap();
    assert!((720.0 - (y + h) - 24.0).abs() < 1.0, "bottom edge was {}", y + h);
}

#[test]
fn long_settings_controls_wrap_below_their_labels_at_minimum_width() {
    let rt = runtime_at(fixtures::settings, (860.0, 580.0));
    let ([row_x, row_y, row_w, _], _) = rt.presented("setting-default-qobuz").unwrap();
    let ([track_x, track_y, track_w, _], _) = rt.presented("default-qobuz-track").unwrap();
    assert!(track_y > row_y + 40.0, "the label should sit above the track");
    assert!(track_x >= row_x && track_x + track_w <= row_x + row_w);
}

#[test]
fn light_theme_log_box_has_distinct_surface_and_bulk_queue_controls_work() {
    let mut rt = runtime(fixtures::light);
    let log_id = rt.state.jobs.iter().find(|j| !j.log.is_empty()).unwrap().id;
    rt.state.toggle_log(log_id);
    rt.invalidate();
    rt.frame(0.0);
    let elements = rt.elements();
    let card = elements.iter().find(|el| el.key.as_deref() == Some(&format!("job-{log_id}"))).unwrap();
    let log_box = elements.iter().find(|el| el.key.as_deref() == Some(&format!("log-view-{log_id}"))).unwrap();
    assert_ne!(card.fill, log_box.fill, "light-mode log box must contrast with its card");

    // The dialog defaults to keeping the queue running.
    assert!(rt.find("cancel-all").is_some());
    assert!(rt.find("retry-failed").is_some());
    tap(&mut rt, "cancel-all");
    assert_eq!(rt.state.confirm_action, Some(crate::app::ConfirmAction::CancelAll));
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("keep-downloading") && el.focused));
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("cancel-all-counts")
        && el.text.as_deref() == Some("3 jobs will be canceled (1 active, 2 queued).")));
    rt.key(rune::KeyEvent { key: rune::Key::Enter, mods: rune::Mods::default() });
    rt.frame(1.0 / 60.0);
    assert!(rt.state.confirm_action.is_none());
    assert_eq!(rt.state.jobs.iter().filter(|j| j.status == Status::Queued).count(), 2);
    tap(&mut rt, "cancel-all");
    tap(&mut rt, "confirm-cancel-all");
    assert!(rt.state.confirm_action.is_none());
    assert!(rt.state.jobs.iter().all(|j| j.status != Status::Queued && j.status != Status::Choosing));
    tap(&mut rt, "retry-failed");
    assert!(rt.state.jobs.iter().any(|j| j.status == Status::Queued));
}

#[test]
fn undo_toast_restores_cleared_history_and_finished_jobs_and_remove_requires_confirm() {
    let mut rt = runtime(fixtures::library);
    let original_len = rt.state.history.len();
    tap(&mut rt, "clear-history");
    assert!(rt.state.history.is_empty());
    assert!(rt.state.undo.is_some());
    assert!(rt.find("info").is_some());
    rt.state.undo_last();
    rt.invalidate();
    rt.frame(0.0);
    assert_eq!(rt.state.history.len(), original_len);

    let mut q = runtime(fixtures::queue);
    let original_jobs = q.state.jobs.len();
    tap(&mut q, "queue-more");
    tap(&mut q, "queue-actions-item1");
    assert!(q.state.jobs.len() < original_jobs);
    assert!(q.state.undo.is_some());
    q.state.undo_last();
    assert_eq!(q.state.jobs.len(), original_jobs);

    let mut exts = runtime(fixtures::extensions_page);
    exts.state.link = Some(crate::app::Link::new(std::sync::Arc::new(|| {})));
    exts.invalidate();
    exts.frame(0.0);
    tap(&mut exts, "remove-cyan-dl");
    assert_eq!(exts.state.confirm_action, Some(crate::app::ConfirmAction::RemoveExtension("cyan-dl".into())));
    assert!(!exts.state.removing.contains("cyan-dl"), "first click arms removal without deleting");
    tap(&mut exts, "remove-cyan-dl");
    assert!(exts.state.removing.contains("cyan-dl"));
}

#[test]
fn settings_defaults_stay_visible_during_transient_outage_and_show_layout_preview() {
    let mut rt = runtime_at(fixtures::settings, (1080.0, 820.0));
    rt.state.extension_status.insert("flacdownloader".into(), store::ExtensionStatus::Services(Vec::new()));
    rt.state.extension_status.insert("lucida".into(), store::ExtensionStatus::Services(Vec::new()));
    rt.invalidate();
    rt.frame(0.0);
    assert!(rt.find("setting-default-tidal").is_some(), "offline status must not hide default pickers");
    assert!(rt.find("layout-preview").is_some());
    assert!(rt.find("refresh-tools").is_some());

    // Wrapping default picker selects preferred extension and reset-output restores the default folder.
    tap(&mut rt, "default-tidal/segment2");
    assert_eq!(rt.state.settings.preferred.get("tidal").map(String::as_str), Some("odskyler"));
    assert!(rt.find("reset-output").is_some());
    tap(&mut rt, "reset-output");
    assert_eq!(rt.state.settings.output, store::default_output());
    assert!(rt.find("reset-output").is_none());
}

#[test]
fn dark_theme_cards_lift_above_the_page_and_nested_panels_recess() {
    let rt = runtime(fixtures::choose);
    let elements = rt.elements();
    let window = elements.iter().find(|el| el.key.as_deref() == Some("window")).unwrap();
    let card = elements.iter().find(|el| el.key.as_deref() == Some("job-1")).unwrap();
    let panel = elements.iter().find(|el| el.key.as_deref() == Some("choices-panel-1")).unwrap();
    assert_ne!(window.fill, card.fill, "dark-mode cards must be distinct from the page");
    assert_ne!(card.fill, panel.fill, "nested choice panel must be distinct from its parent card");
    assert_eq!(panel.fill, window.fill, "nested choice panel recesses to page depth");
}

#[test]
fn global_keyboard_shortcuts_switch_pages_and_escape_dismisses_panels() {
    use rune::{Key, KeyEvent, Mods};
    let mut rt = runtime(fixtures::home);
    rt.on_key = Some(crate::on_key);
    let send = |rt: &mut Runtime<App>, key: Key| {
        let cmd = matches!(key, Key::Command(_));
        rt.key(KeyEvent { key, mods: Mods { cmd, ..Mods::default() } });
        rt.frame(1.0 / 60.0);
    };
    send(&mut rt, Key::Command('2'));
    assert_eq!(rt.state.page, Page::Catalog);
    send(&mut rt, Key::Command('3'));
    assert_eq!(rt.state.page, Page::Library);
    send(&mut rt, Key::Command('4'));
    assert_eq!(rt.state.page, Page::Extensions);
    send(&mut rt, Key::Command('5'));
    assert_eq!(rt.state.page, Page::Settings);
    send(&mut rt, Key::Command('l'));
    assert_eq!(rt.state.page, Page::Downloads);

    // Move off the focused text input, then verify Escape dismisses armed confirmations and panels.
    send(&mut rt, Key::Command('4'));
    rt.state.confirm_action = Some(crate::app::ConfirmAction::CancelAll);
    send(&mut rt, Key::Escape);
    assert!(rt.state.confirm_action.is_none());

    rt.state.extension_settings = Some("odskyler".into());
    send(&mut rt, Key::Escape);
    assert!(rt.state.extension_settings.is_none());

    let mut queue = runtime(fixtures::queue);
    queue.on_key = Some(crate::on_key);
    tap(&mut queue, "queue-more");
    assert!(queue.state.queue_menu_open);
    send(&mut queue, Key::Escape);
    assert!(!queue.state.queue_menu_open);
    assert!(queue.elements().iter().any(|el| el.key.as_deref() == Some("queue-more") && el.focused));
}

#[test]
fn library_filters_by_service_sorts_tracks_and_displays_sidecars() {
    let mut rt = runtime(|s| {
        fixtures::library(s);
        s.history[0].title = "Zebra".into();
        s.history[1].title = "Alpha".into();
        s.history[0].artist = "Beta".into();
        s.history[1].artist = "Aaron".into();
        s.history[0].files = vec!["C:/Music/Zebra.flac".into()];
        s.history[1].files = vec!["C:/Music/Alpha.flac".into()];
        let first_file = s.history[0].main_file().unwrap().to_owned();
        let second_file = s.history[1].main_file().unwrap().to_owned();
        s.library_files.insert(first_file, crate::library::SidecarInfo {
            exists: true,
            sidecars: vec![("LRC".into(), "C:/Music/Zebra.lrc".into()), ("Meta".into(), "C:/Music/Zebra.json".into())],
        });
        s.library_files.insert(second_file, crate::library::SidecarInfo {
            exists: false,
            sidecars: Vec::new(),
        });
    });
    assert!(rt.find("sidecar-0-lrc").is_some());
    assert!(rt.find("sidecar-0-meta").is_some());
    assert!(rt.find("missing-entry-1").is_some());

    // Service filter narrows the visible entries.
    tap(&mut rt, "library-filter-tidal");
    assert_eq!(rt.state.filtered_history().len(), 1);
    tap(&mut rt, "library-filter-all");
    assert_eq!(rt.state.filtered_history().len(), 4);

    // Sort by Title puts "Alpha" first.
    tap(&mut rt, "library-sort/segment1");
    assert_eq!(rt.state.library_sort, crate::app::LibrarySort::Title);
    assert_eq!(rt.state.filtered_history()[0].1.title, "Alpha");

    // Sort by Artist puts "Aaron" first.
    tap(&mut rt, "library-sort/segment2");
    assert_eq!(rt.state.library_sort, crate::app::LibrarySort::Artist);
    assert_eq!(rt.state.filtered_history()[0].1.artist, "Aaron");
}

#[test]
fn queue_pause_copy_failed_batch_summary_and_duplicate_hint() {
    let mut rt = runtime(fixtures::queue);
    // Pause / Resume queue toggles state and shows indicator badge.
    assert!(rt.find("toggle-pause").is_some());
    tap(&mut rt, "toggle-pause");
    assert!(rt.state.queue_paused);
    assert!(rt.find("queue-paused-badge").is_some());
    tap(&mut rt, "toggle-pause");
    assert!(!rt.state.queue_paused);
    assert!(rt.find("queue-paused-badge").is_none());

    // Copy Failed Links copies failed job URLs and shows a success toast.
    tap(&mut rt, "queue-more");
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("queue-actions-item0")
        && el.label.as_deref() == Some("Copy Failed Links")));
    tap(&mut rt, "queue-actions-item0");
    assert!(rt.state.info.as_ref().is_some_and(|n| n.message.contains("Copied 1 failed link")));

    // Multi-link batch summary + non-blocking duplicate hint.
    rt.state.history[0].url = "https://tidal.com/track/441821360".into();
    rt.state.url = rune::TextEdit::new("https://tidal.com/track/441821360 https://www.deezer.com/track/3412534581");
    rt.invalidate();
    rt.frame(0.0);
    assert!(rt.find("batch-summary").is_some());
    assert!(rt.find("duplicate-hint").is_some());
}

#[test]
fn fixture_manifests_have_unique_extension_ids() {
    let ids: std::collections::BTreeSet<_> = fixtures::MANIFESTS
        .iter()
        .map(|json| crate::extensions::parse_manifest(&json.to_string()).unwrap().id)
        .collect();
    assert_eq!(ids.len(), fixtures::MANIFESTS.len());
}

#[test]
fn custom_pickers_and_selectable_cards_expose_astryx_accessibility_and_target_sizes() {
    fn find_a11y<'a>(node: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
        if node.get("key").and_then(|v| v.as_str()) == Some(key) {
            return Some(node);
        }
        node.get("children")?.as_array()?.iter().find_map(|child| find_a11y(child, key))
    }

    let mut settings = runtime_at(fixtures::settings, (1080.0, 820.0));
    let tree = settings.accessibility_json();
    assert_eq!(find_a11y(&tree, "theme").and_then(|n| n.get("name")).and_then(|v| v.as_str()), Some("Theme"));
    assert_eq!(find_a11y(&tree, "default-tidal").and_then(|n| n.get("name")).and_then(|v| v.as_str()), Some("Tidal"));

    // Wrapping default_picker exposes radio checked state and supports arrow-key roving.
    tap(&mut settings, "default-tidal/segment2");
    let tree = settings.accessibility_json();
    let tidal = find_a11y(&tree, "default-tidal").expect("tidal radiogroup");
    let picked = find_a11y(tidal, "segment2").expect("selected radio pill");
    assert_eq!(picked.get("name").and_then(|v| v.as_str()), Some("Rhythmax"));
    assert!(picked.get("states").and_then(|v| v.as_array()).is_some_and(|s| s.iter().any(|v| v.as_str() == Some("checked"))));

    // Selectable track card in Catalog Feed exposes checked state and 28px scroll buttons.
    let mut feed = runtime(fixtures::catalog_feed);
    let ([_, _, prev_w, prev_h], _) = feed.presented("catalog-shelf-0-prev").unwrap();
    assert_eq!((prev_w, prev_h), (28.0, 28.0));
    tap(&mut feed, "catalog-feed-item-0-0");
    let feed_tree = feed.accessibility_json();
    let card = find_a11y(&feed_tree, "catalog-feed-item-0-0").expect("selected feed card");
    assert!(card.get("states").and_then(|v| v.as_array()).is_some_and(|s| s.iter().any(|v| v.as_str() == Some("checked"))));

    // Small filter buttons use Astryx sm height (28px).
    let library = runtime(fixtures::library);
    let ([_, _, _, filter_h], _) = library.presented("library-filter-all").unwrap();
    assert_eq!(filter_h, 28.0);
}
