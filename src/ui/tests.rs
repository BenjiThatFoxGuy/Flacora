use super::*;
use crate::{fixtures, jobs::Status};
use rune::{Env, Runtime};
use std::path::PathBuf;

fn runtime(setup: fn(&mut App)) -> Runtime<App> {
    runtime_at(setup, (1080.0, 720.0))
}

fn runtime_at(setup: fn(&mut App), size: (f32, f32)) -> Runtime<App> {
    let mut state = App::default();
    setup(&mut state);
    let env = Env { system_dark: true, scale: 1.0 };
    let mut rt = Runtime::new(state, view, size, env);
    rt.frame(0.0);
    rt
}

fn tap(rt: &mut Runtime<App>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("{key} is on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..3 {
        rt.frame(1.0 / 60.0);
    }
}

#[test]
fn the_sidebar_switches_pages() {
    let mut rt = runtime(fixtures::home);
    for (key, page, marker) in [
        ("nav-catalog", Page::Catalog, "catalog-empty"),
        ("nav-library", Page::Library, "library-list"),
        ("nav-extensions", Page::Extensions, "install-extension"),
        ("nav-settings", Page::Settings, "setting-folder"),
        ("nav-downloads", Page::Downloads, "composer"),
    ] {
        tap(&mut rt, key);
        assert_eq!(rt.state.page, page);
        assert!(rt.find(marker).is_some(), "{marker} shows on {page:?}");
    }
    tap(&mut rt, "destination");
    assert_eq!(rt.state.page, Page::Settings);
}

#[test]
fn the_sidebar_counts_installed_extensions_after_loading() {
    use crate::app::Msg;

    let loading = runtime(fixtures::loading);
    assert!(loading.find("nav-extensions/badge").is_none());

    let mut rt = runtime(fixtures::fresh);
    tap(&mut rt, "nav-extensions");
    assert_eq!(rt.state.page, Page::Extensions);
    let check_count = |rt: &Runtime<App>, expected: &str| {
        let elements = rt.elements();
        assert!(elements.iter().any(|el| el.path.contains("/nav-extensions/badge/")
            && el.text.as_deref() == Some(expected)));
        let label = format!("Extensions, {expected} installed");
        assert!(elements.iter().any(|el| el.key.as_deref() == Some("nav-extensions")
            && el.label.as_deref() == Some(&label)));
    };
    check_count(&rt, "0");

    rt.state.apply(Msg::Imported(PathBuf::from("odesli.fext"), Ok(Box::new(fixtures::installed("odesli")))));
    rt.invalidate();
    rt.frame(0.0);
    check_count(&rt, "1");

    rt.state.apply(Msg::Removed("odesli".into(), Ok(())));
    rt.invalidate();
    rt.frame(0.0);
    check_count(&rt, "0");
}

#[test]
fn pages_stay_centered_and_bounded_when_the_window_resizes() {
    let mut rt = runtime_at(fixtures::queue, (1280.0, 720.0));
    for size in [(1280.0, 720.0), (1920.0, 1080.0), (2560.0, 1440.0), (1280.0, 720.0)] {
        rt.set_viewport(size, 1.0);
        rt.frame(0.0);
        rt.frame(0.6);
        let ([pane_x, _, pane_width, _], _) = rt.presented("content").unwrap();
        let expected = pane_width.min(theme::PAGE_CONTENT_MAX_WIDTH + 2.0 * theme::PAGE_INSET);
        for (navigation, page, component) in [
            ("nav-downloads", "downloads-page", "composer"),
            ("nav-catalog", "catalog-page", "catalog-empty"),
            ("nav-library", "library-page", "library-list"),
            ("nav-extensions", "extensions-page", "extension-filters"),
            ("nav-settings", "settings-page", "group-downloads"),
        ] {
            tap(&mut rt, navigation);
            rt.frame(0.6);
            let ([x, _, width, height], _) = rt.presented(page).unwrap();
            let ([component_x, _, component_width, _], _) = rt.presented(component).unwrap();
            assert!((width - expected).abs() < 1.0, "{page} at {size:?}: {width}");
            assert!((x + width / 2.0 - pane_x - pane_width / 2.0).abs() < 1.0, "{page} is centered");
            assert!((height - size.1).abs() < 1.0, "{page} uses the available height");
            assert!((component_x - x - theme::PAGE_INSET).abs() < 1.0, "{component} horizontal inset");
            assert!((component_width - (expected - 2.0 * theme::PAGE_INSET)).abs() < 1.0, "{component} width");
        }
    }
}

#[test]
fn sidebar_footer_links_open_support_and_project_pages() {
    let mut rt = runtime_at(fixtures::home, (1280.0, 720.0));
    assert!(rt.find("sidebar-update").is_none());
    let footer_y = |key| rt.presented(key).unwrap().0[1];
    assert!(footer_y("buy-me-a-coffee") < footer_y("project-spotbye"));
    assert!(footer_y("buy-me-a-coffee") < footer_y("project-stream-hoarder"));
    let ([x, _, width, height], _) = rt.presented("buy-me-a-coffee").unwrap();
    let ([settings_x, _, settings_width, settings_height], _) = rt.presented("nav-settings").unwrap();
    assert_eq!((x, width, height), (settings_x, settings_width, settings_height));
    let status_left = |key| rt.presented(key).unwrap().0[0];
    assert_eq!(status_left("project-spotbye-status"), status_left("project-stream-hoarder-status"));
    for (key, url) in [
        ("buy-me-a-coffee", "https://afkarxyz.gumroad.com/coffee"),
        ("project-spotbye", "https://spotbye.vip/"),
        ("project-spotbye-status", "https://status.spotbye.vip/"),
        ("project-stream-hoarder", "https://streamhoarder.app/"),
        ("project-stream-hoarder-status", "https://status.streamhoarder.app/"),
    ] {
        tap(&mut rt, key);
        assert_eq!(rt.take_url_requests(), [url]);
        assert_eq!(rt.state.page, Page::Downloads);
    }
}

#[test]
fn the_sidebar_update_badge_sits_above_coffee_and_opens_github_release() {
    let mut rt = runtime(|s| {
        fixtures::settings_update_available(s);
        s.page = Page::Downloads;
    });
    let (badge, _) = rt.presented("sidebar-update").unwrap();
    let (coffee, _) = rt.presented("buy-me-a-coffee").unwrap();
    assert!(badge[1] + badge[3] <= coffee[1]);
    let release = rt.state.available_update.as_ref().unwrap();
    let expected = format!("https://github.com/spotbye/Flacora/releases/tag/v{}", release.version);
    tap(&mut rt, "sidebar-update");
    assert_eq!(rt.take_url_requests(), [expected]);
    assert_eq!(rt.state.page, Page::Downloads);

    rt.state.apply(crate::app::Msg::UpdateChecked(None));
    rt.invalidate();
    rt.frame(0.0);
    assert!(rt.find("sidebar-update").is_none());
}

#[test]
fn extension_counts_and_ffmpeg_details_stay_on_their_pages() {
    let mut rt = runtime(fixtures::home);
    assert!(!rt.elements().iter().any(|el| {
        el.path.contains("/sidebar-footer/") && el.text.as_deref().is_some_and(|text| {
            text.contains("FFmpeg") || text.ends_with(" Extensions")
        })
    }));

    tap(&mut rt, "nav-extensions");
    assert!(rt.find("extension-category-downloaders").is_some());
    tap(&mut rt, "nav-settings");
    assert!(rt.find("setting-ffmpeg").is_some());
}

#[test]
fn notifications_in_the_toast_region_can_be_dismissed_independently() {
    let mut rt = runtime_at(fixtures::notifications, (1280.0, 720.0));
    assert!(rt.find("toast-region").is_some());
    // Toasts stay clear of right-aligned card action columns.
    let ([toast_x, _, toast_w, _], _) = rt.presented("info").unwrap();
    let ([status_x, _, _, _], _) = rt.presented("online-status-lucida").unwrap();
    assert!(toast_x + toast_w < status_x, "toast ({}) should stay clear of card actions ({status_x})", toast_x + toast_w);
    tap(&mut rt, "info/top/close");
    assert!(rt.state.info.is_none());
    assert!(rt.state.notice.is_some());
    tap(&mut rt, "notice/top/close");
    assert!(rt.state.notice.is_none());
}

#[test]
fn status_toasts_use_their_theme_colors_and_expire_independently() {
    let mut rt = runtime(fixtures::notifications);
    rt.poll = Some(crate::poll);
    rt.frame(1.0 / 60.0);
    let k = theme::kit(true);
    let elements = rt.elements();
    for (key, label, fill) in [("info", "Success", k.c.success.bg), ("notice", "Error", k.c.danger.bg)] {
        let toast = elements.iter().find(|el| el.key.as_deref() == Some(key)).unwrap();
        assert_eq!(toast.label.as_deref(), Some(label));
        assert_eq!(toast.fill, fill);
    }

    rt.frame(2.4);
    assert!(rt.find("info").is_some());
    let before_exit = rt.presented("info").unwrap().0;
    rt.frame(0.2);
    assert!(rt.find("info").is_none());
    assert!(rt.find("notice").is_some());
    let exiting = rt.elements().into_iter().find(|el| el.key.as_deref() == Some("info")).unwrap();
    assert!(exiting.exiting && exiting.opacity > 0.0);
    assert_eq!(&exiting.rect[..2], &before_exit[..2]);
    rt.frame(0.08);
    let fading = rt.elements().into_iter().find(|el| el.key.as_deref() == Some("info")).unwrap();
    assert!(fading.opacity < exiting.opacity);

    rt.frame(1.25);
    assert!(rt.find("notice").is_some());
    rt.frame(0.1);
    assert!(rt.find("notice").is_none());
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("notice") && el.exiting));
    rt.frame(0.5);
    assert!(rt.elements().iter().all(|el| el.key.as_deref() != Some("notice")));
}

#[test]
fn a_link_without_an_installed_extension_opens_the_install_controls() {
    let mut rt = runtime(fixtures::missing);
    assert!(rt.find("install-odesli").is_none());
    tap(&mut rt, "open-extensions");
    assert_eq!(rt.state.page, Page::Extensions);
    assert!(rt.find("install-extension").is_some());
}

#[test]
fn a_spotify_link_goes_through_the_resolver() {
    let mut rt = runtime(fixtures::spotify);
    assert!(rt.find("resolver").is_some(), "the route names Odesli");
    tap(&mut rt, "download");
    assert_eq!(rt.state.jobs.last().map(|j| j.extension.manifest.id.as_str()), Some("odesli"));
}

#[test]
fn with_two_resolvers_the_composer_lets_you_pick_one() {
    let mut rt = runtime(fixtures::resolvers);
    assert!(rt.find("resolver-pick").is_some() && rt.find("resolver").is_none());
    tap(&mut rt, "resolver-pick/segment1");
    assert_eq!(rt.state.settings.preferred.get(crate::app::RESOLVER_KEY).map(String::as_str), Some("squigly"));
    tap(&mut rt, "download");
    assert_eq!(rt.state.jobs.last().map(|j| j.extension.manifest.id.as_str()), Some("squigly"));
}

#[test]
fn picking_an_extension_is_remembered_for_the_service() {
    let mut rt = runtime(fixtures::queue);
    assert_eq!(rt.state.selected().unwrap().manifest.id, "flacdownloader");
    assert!(rt.find("option-quality").is_none(), "FlacDownloader has no quality option");
    tap(&mut rt, "extension-pick/segment1");
    assert_eq!(rt.state.settings.preferred.get("tidal").map(String::as_str), Some("odskyler"));
    rt.frame(1.0 / 60.0);
    assert!(rt.find("option-quality").is_some(), "Rhythmax shows its quality option");
}

#[test]
fn queue_cards_offer_the_actions_their_state_allows() {
    let mut rt = runtime(fixtures::queue);
    assert!(rt.find("cancel-1").is_some() && rt.find("progress-1").is_some(), "running");
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("progress-1") && el.label.as_deref() == Some("Downloading Audio")));
    assert!(rt.find("cancel-2").is_some() && rt.find("cancel-3").is_some(), "queued");
    assert!(rt.find("play-4").is_some() && rt.find("reveal-4").is_some(), "done");
    assert!(rt.find("retry-5").is_some() && rt.find("log-view-5").is_some(), "failed, details open");
    tap(&mut rt, "cancel-2");
    assert_eq!(rt.state.jobs[1].status, Status::Cancelled);
    tap(&mut rt, "clear-finished");
    let left: Vec<u64> = rt.state.jobs.iter().map(|j| j.id).collect();
    assert_eq!(left, [1, 3], "the running and the waiting job stay");
}

#[test]
fn dropping_a_package_anywhere_installs_it_and_other_files_are_refused() {
    let mut rt = runtime(fixtures::queue);
    let package = PathBuf::from("C:/Downloads/lucida-0.2.0-windows-x64.flacora");
    rt.drag_files(vec![package.clone()], Some((600.0, 400.0)));
    rt.frame(1.0 / 60.0);
    assert!(rt.state.drop_over && rt.find("drop-overlay").is_some());
    rt.drop_files(vec![package], Some((600.0, 400.0)));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.importing, 1);
    assert!(!rt.state.drop_over);

    rt.drop_files(vec![PathBuf::from("C:/Music/Golden.flac")], Some((600.0, 400.0)));
    rt.frame(1.0 / 60.0);
    let notice = &rt.state.notice.as_ref().expect("rejected file shows a notice").message;
    assert!(notice.contains("Golden.flac is not an extension"), "{notice}");
    assert!(notice.contains("Drop a .fext file"), "{notice}");
}

#[test]
fn extensions_lists_local_installs_and_keeps_the_header_actions_visible() {
    let mut empty = runtime(fixtures::extensions_empty);
    assert!(empty.find("drop-zone").is_some());
    assert!(empty.find("ext-lucida").is_none());
    tap(&mut empty, "nav-downloads");
    tap(&mut empty, "go-extensions");
    assert_eq!(empty.state.page, Page::Extensions);

    let mut rt = runtime(fixtures::extensions_page);
    assert!(rt.find("drop-zone").is_none());
    for id in rt.state.installed.iter().map(|extension| &extension.manifest.id) {
        assert!(rt.find(&format!("ext-{id}")).is_some(), "{id} is listed");
        assert!(rt.find(&format!("site-{id}")).is_some(), "{id} shows its website");
    }
    for key in ["install-file", "refresh", "ext-doubledouble", "install-odskyler"] {
        assert!(rt.find(key).is_none(), "{key} is absent");
    }
    assert!(rt.find("kind-odesli").is_some() && rt.find("kind-lucida").is_some(), "each card says what the extension is");
    let install = rt.find("install-extension").unwrap();
    let check = rt.find("check-extension-status").unwrap();
    assert!(install.0 > check.0 && install.1 == check.1);
    let ([_, head_y, _, head_h], _) = rt.presented("page-head").unwrap();
    assert_eq!(rt.presented("extension-filters").unwrap().0[1], head_y + head_h + 16.0);
    rt.wheel(760.0, 520.0, -2000.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.scroll_at_bottom("extension-list"), Some(true));
    assert_eq!(rt.find("install-extension"), Some(install));
}

#[test]
fn extension_cards_are_alphabetical_within_each_category() {
    let mut rt = runtime(fixtures::extensions_page);
    tap(&mut rt, "nav-downloads");
    tap(&mut rt, "nav-extensions");

    let order = [
        "extension-category-downloaders", "ext-cyan-dl", "ext-flacdownloader", "ext-lucida", "ext-monochrome", "ext-odskyler",
        "extension-category-resolvers", "ext-odesli", "ext-squigly",
        "extension-category-metadata", "ext-musicbrainz",
        "extension-category-lyrics", "ext-betterlyrics", "ext-lrclib", "ext-lyricsplus", "ext-musixmatch",
    ];
    let positions = order.map(|key| rt.presented(key).unwrap_or_else(|| panic!("{key} is listed")).0[1]);
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{positions:?}");

    let partial = runtime(fixtures::extension_options);
    assert!(partial.find("extension-category-metadata").is_none());
    assert!(partial.find("extension-category-lyrics").is_none());
}

#[test]
fn extension_settings_use_manifest_choices_and_update_the_composer() {
    let mut rt = runtime(|s| {
        fixtures::extension_options(s);
        s.extension_settings = None;
    });
    assert!(rt.find("configure-odesli").is_none());
    assert!(rt.find("extension-options-odskyler").is_none());
    tap(&mut rt, "configure-odskyler");
    for _ in 0..30 { rt.frame(1.0 / 60.0); }
    tap(&mut rt, "setting-odskyler-quality/segment1");
    assert_eq!(rt.state.settings.options.get("odskyler/quality").map(String::as_str), Some("hires"));

    tap(&mut rt, "configure-doubledouble");
    assert!(rt.find("extension-options-odskyler").is_none());
    for _ in 0..30 { rt.frame(1.0 / 60.0); }
    tap(&mut rt, "setting-doubledouble-region/segment1");
    assert_eq!(rt.state.settings.options.get("doubledouble/region").map(String::as_str), Some("eu"));

    tap(&mut rt, "nav-downloads");
    assert!(rt.find("option-quality").is_some());
    let quality = rt.state.selected().unwrap().manifest.options.iter().find(|o| o.key == "quality").unwrap();
    assert_eq!(rt.state.pick_index("odskyler", quality), 1);
}

#[test]
fn extension_settings_fold_without_moving_the_card_header() {
    let mut rt = runtime_at(|s| {
        fixtures::extension_options(s);
        s.extension_settings = None;
    }, (1280.0, 720.0));
    let height = |rt: &Runtime<App>| rt.presented("ext-cyan-dl").unwrap().0[3];
    let next_top = |rt: &Runtime<App>| rt.presented("ext-doubledouble").unwrap().0[1];
    let trigger = rt.presented("configure-cyan-dl").unwrap().0;
    let closed = height(&rt);
    let next_closed = next_top(&rt);
    let settle = |rt: &mut Runtime<App>| {
        for _ in 0..45 { rt.frame(1.0 / 60.0); }
    };

    tap(&mut rt, "configure-cyan-dl");
    let opening = height(&rt);
    assert_eq!(rt.presented("configure-cyan-dl").unwrap().0, trigger);
    assert!((next_top(&rt) - next_closed - (opening - closed)).abs() < 1.0);
    settle(&mut rt);
    let opened = height(&rt);
    assert!(closed < opening && opening < opened, "heights while opening: {closed}, {opening}, {opened}");

    tap(&mut rt, "configure-cyan-dl");
    let closing = height(&rt);
    assert!(closed < closing && closing < opened, "height while closing: {closing}");
    assert_eq!(rt.presented("configure-cyan-dl").unwrap().0, trigger);
    settle(&mut rt);
    assert_eq!(height(&rt), closed);
    assert_eq!(next_top(&rt), next_closed);
    assert!(rt.find("setting-cyan-dl-quality").is_none());
}

#[test]
fn the_empty_extension_drop_zone_highlights_and_yields_to_the_install_button() {
    let mut rt = runtime(fixtures::extensions_empty);
    let (empty, _) = rt.presented("drop-zone").unwrap();
    assert_eq!(empty[3], 224.0);
    let content = rt.presented("extensions-page").unwrap().0;
    assert!((empty[0] + empty[2] / 2.0 - content[0] - content[2] / 2.0).abs() < 1.0);

    let package = PathBuf::from("C:/Downloads/odesli.fext");
    let at = Some((900.0, 620.0));
    rt.drag_files(vec![package.clone()], at);
    rt.frame(1.0 / 60.0);
    assert!(rt.state.drop_over);
    assert!(rt.find("drop-overlay").is_none());
    assert!(rt.elements().iter().any(|element| element.text.as_deref() == Some("Drop to Install")));

    rt.drop_files(vec![package.clone()], at);
    rt.frame(1.0 / 60.0);
    assert!(rt.elements().iter().any(|element| element.text.as_deref() == Some("Installing Packages…")));

    rt.state.apply(crate::app::Msg::Imported(package, Ok(Box::new(fixtures::installed("odesli")))));
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    assert!(rt.find("drop-zone").is_none());
    assert!(rt.find("install-extension").is_some());
    assert!(rt.find("ext-odesli").is_some());
}

#[test]
fn both_extension_install_controls_can_browse_and_import_a_local_package() {
    for (scene, key) in [(fixtures::extensions_empty as fn(&mut App), "drop-zone"), (fixtures::extensions_page, "install-extension")] {
        let mut rt = runtime(scene);
        tap(&mut rt, key);
        let (kind, options) = rt.file_dialog().expect("the install control requests a file picker");
        assert_eq!(kind, rune::FileDialogKind::Open);
        assert_eq!(options.filters, [("Extension Packages (*.fext)".into(), vec!["fext".into()])]);

        rt.resolve_file_dialog(None).unwrap();
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.importing, 0);

        tap(&mut rt, key);
        rt.resolve_file_dialog(Some(PathBuf::from("C:/Downloads/legacy.zip"))).unwrap();
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.importing, 0);
        assert_eq!(rt.state.notice.as_ref().unwrap().message, "Select only .fext extension packages.");

        tap(&mut rt, key);
        rt.resolve_file_dialog(Some(PathBuf::from("C:/Downloads/doubledouble.fext"))).unwrap();
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.importing, 1);
    }
}

#[test]
fn populated_extensions_shows_a_drop_overlay_only_while_dragging_packages() {
    let mut rt = runtime(fixtures::extensions_page);
    let at = Some((120.0, 400.0));
    rt.drag_files(vec![PathBuf::from("C:/Downloads/song.flac")], at);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("drop-overlay").is_none());

    let package = PathBuf::from("C:/Downloads/doubledouble.fext");
    rt.drag_files(vec![package.clone()], at);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("drop-overlay").is_some());
    rt.drop_files(vec![package], at);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("drop-overlay").is_none());
    assert_eq!(rt.state.importing, 1);
    assert!(rt.elements().iter().any(|element| element.text.as_deref() == Some("Installing Packages…")));
}

#[test]
fn extension_cards_show_colored_kind_tiles_and_only_music_service_badges() {
    let mut rt = runtime(|s| {
        fixtures::extension_badges(s);
        s.images(std::sync::Arc::new(|| {}));
        for image in s.provider_logos.values() {
            image.wait().expect("embedded provider logo decodes");
        }
    });
    tap(&mut rt, "nav-downloads");
    tap(&mut rt, "nav-extensions");

    for key in [
        "kind-betterlyrics/lucide-mic-vocal",
        "kind-flacdownloader/lucide-cloud-download",
        "kind-musicbrainz/lucide-tag",
        "kind-odesli/lucide-share-2",
        "services-flacdownloader/provider-icon-qobuz",
        "services-flacdownloader/provider-icon-tidal",
        "services-flacdownloader/provider-icon-deezer",
        "services-monochrome/provider-icon-tidal",
        "services-odesli/provider-icon-spotify",
        "services-odesli/provider-icon-youtube-music",
        "services-odesli/provider-icon-amazon",
        "services-odesli/provider-icon-apple",
    ] {
        assert!(rt.find(key).is_some(), "{key} is present");
    }
    assert_eq!(fixtures::manifest("monochrome").services, ["Tidal"]);
    for id in ["betterlyrics", "flacdownloader", "musicbrainz", "odesli"] {
        assert!(rt.find(&format!("kind-label-{id}")).is_none(), "section header already shows the category");
    }
    for id in ["betterlyrics", "musicbrainz"] {
        assert!(rt.find(&format!("services-{id}")).is_none());
    }
    let elements = rt.elements();
    for logo in elements.iter().filter(|el| el.key.as_deref().is_some_and(|key| key.starts_with("provider-icon-"))) {
        let badge = &elements[logo.parent.expect("provider icon has a wrapper")];
        assert_eq!(badge.fill, rune::Color::TRANSPARENT);
    }
    for (id, service) in [("flacdownloader", "qobuz"), ("odesli", "spotify")] {
        let group = format!("/services-{id}/");
        let key = format!("provider-icon-{service}");
        let logo = elements.iter().find(|el| el.path.contains(&group) && el.key.as_deref() == Some(&key)).unwrap();
        let (website, _) = rt.presented(&format!("site-{id}")).unwrap();
        assert_eq!(logo.rect[0], website[0]);
    }
    let mut fills = Vec::new();
    for (id, label) in [("betterlyrics", "Lyrics"), ("flacdownloader", "Downloader"), ("musicbrainz", "Metadata"), ("odesli", "Link Resolver")] {
        let key = format!("kind-{id}");
        let tile = elements.iter().find(|el| el.key.as_deref() == Some(&key)).expect("kind tile exists");
        assert_eq!(tile.label.as_deref(), Some(label));
        assert!(!fills.contains(&tile.fill), "{label} has a distinct tile color");
        fills.push(tile.fill);
        assert!(tile.rect[0] < rt.presented(&format!("site-{id}")).unwrap().0[0], "{label} is on the left");
    }
    let ([status_x, status_y, status_w, status_h], _) = rt.presented("online-status-odesli").unwrap();
    let ([remove_x, remove_y, _, remove_h], _) = rt.presented("remove-odesli").unwrap();
    assert!((remove_x - status_x - status_w - 8.0).abs() < 1.0);
    assert!((status_y + status_h / 2.0 - remove_y - remove_h / 2.0).abs() < 1.0);
    assert!(status_x > rt.presented("site-odesli").unwrap().0[0]);
}

#[test]
fn extension_status_refresh_checks_every_installed_category() {
    let mut rt = runtime(fixtures::extension_status);
    let elements = rt.elements();
    for (id, label) in [("cyan-dl", "Online"), ("lucida", "Offline"), ("odesli", "Status Unknown"),
        ("musicbrainz", "Online"), ("betterlyrics", "Degraded")]
    {
        let key = format!("online-status-{id}");
        assert!(elements.iter().any(|el| el.key.as_deref() == Some(&key) && el.label.as_deref() == Some(label)));
    }

    // Unknown legacy extensions without a domain finish without issuing a network request.
    for installed in &mut rt.state.installed {
        installed.manifest.id = format!("test-{}", installed.manifest.id);
        installed.manifest.status.clear();
        installed.manifest.description.clear();
    }
    rt.state.link = Some(crate::app::Link::new(std::sync::Arc::new(|| {})));
    rt.frame(0.0);
    tap(&mut rt, "check-extension-status");
    assert!(rt.state.checking.len() <= 4, "status checks are concurrency-bounded");
    assert_eq!(rt.state.checking.len() + rt.state.status_queue.len(), rt.state.installed.len());
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("online-status-test-betterlyrics")
        && el.label.as_deref() == Some("Checking…")));
}

#[test]
fn extension_filter_moves_remaining_rows_to_the_top_in_one_motion() {
    let mut rt = runtime(fixtures::extension_badges);
    let top = |rt: &Runtime<App>| rt.presented("extension-category-metadata").unwrap().0[1];
    let list_top = rt.presented("extension-list").unwrap().0[1];
    let mut positions = vec![top(&rt)];
    tap(&mut rt, "ext-filter-metadata");
    positions.push(top(&rt));
    for _ in 0..36 {
        rt.frame(1.0 / 60.0);
        positions.push(top(&rt));
    }
    assert!((positions[5] - list_top) < (positions[0] - list_top) * 0.36, "{positions:?}");
    assert!(positions.windows(2).all(|pair| pair[1] <= pair[0] + 0.5), "{positions:?}");
    assert!(positions[18..].windows(2).all(|pair| pair[0] - pair[1] < 3.0), "{positions:?}");
    assert!((positions.last().unwrap() - list_top).abs() < 1.0, "{positions:?}");
}

#[test]
fn changing_the_extension_filter_scrolls_to_the_top_immediately() {
    let mut rt = runtime_at(fixtures::extensions_page, (1080.0, 520.0));
    let top = |rt: &Runtime<App>| rt.presented("extension-category-downloaders").unwrap().0[1];
    let list_top = rt.presented("extension-list").unwrap().0[1];
    rt.wheel(760.0, 410.0, -120.0);
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
    let scrolled = top(&rt);
    assert!(scrolled < list_top - 60.0, "{scrolled} vs {list_top}");

    tap(&mut rt, "ext-filter-downloaders");
    assert!(top(&rt) > scrolled + 3.0 && top(&rt) < list_top - 3.0);
    for _ in 0..36 {
        rt.frame(1.0 / 60.0);
    }
    assert!((top(&rt) - list_top).abs() < 1.0);
}

#[test]
fn extensions_can_be_filtered_by_category_and_rechecked_individually() {
    let mut rt = runtime(fixtures::extension_status);
    assert!(rt.find("status-detail-odesli").is_some(), "unknown status reason displays inline");
    assert!(rt.find("ext-filter-lyrics").is_some());
    tap(&mut rt, "ext-filter-lyrics");
    assert!(rt.find("ext-betterlyrics").is_some());
    assert!(rt.find("ext-cyan-dl").is_none());
    tap(&mut rt, "ext-filter-all");
    assert!(rt.find("ext-cyan-dl").is_some());

    // Clicking one extension's status badge re-checks only that extension.
    for installed in &mut rt.state.installed {
        installed.manifest.id = format!("one-{}", installed.manifest.id);
        installed.manifest.status.clear();
        installed.manifest.description.clear();
    }
    rt.state.link = Some(crate::app::Link::new(std::sync::Arc::new(|| {})));
    rt.invalidate();
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
    tap(&mut rt, "online-status-one-cyan-dl");
    assert_eq!(rt.state.checking.len(), 1);
    assert!(rt.state.checking.contains_key("one-cyan-dl"));
}

#[test]
fn extensions_opens_rentry_in_both_states() {
    for scene in [fixtures::extensions_empty as fn(&mut App), fixtures::extensions_page] {
        let mut rt = runtime(scene);
        assert!(rt.elements().iter().any(|el| {
            el.key.as_deref() == Some("download-extensions") && el.text.as_deref() == Some("Get Extension on Rentry")
        }));
        tap(&mut rt, "download-extensions");
        assert_eq!(rt.take_url_requests(), ["https://rentry.org/flacora"]);
    }
}

#[test]
fn an_available_update_stays_in_the_sidebar_on_settings() {
    let rt = runtime(fixtures::settings_update_available);
    assert!(rt.find("sidebar-update").is_some());
    assert!(rt.find("check-for-updates").is_none());
    assert!(rt.find("app-update").is_none());
    assert!(rt.find("view-update-release").is_none());
}

#[test]
fn extension_website_is_left_aligned_and_opens_as_a_link() {
    let mut rt = runtime(fixtures::extension_badges);
    let elements = rt.elements();
    let title = elements.iter()
        .find(|el| el.path.contains("ext-odesli/") && el.text.as_deref() == Some("Odesli"))
        .unwrap();
    let website = elements.iter().find(|el| el.key.as_deref() == Some("site-odesli")).unwrap();
    assert_eq!(website.rect[0], title.rect[0]);
    assert_eq!(website.fill, rune::Color::TRANSPARENT);
    tap(&mut rt, "site-odesli");
    assert_eq!(rt.take_url_requests(), ["https://odesli.co"]);
}

#[test]
fn the_library_shows_its_recorded_source_and_leaves_older_links_generic() {
    let mut rt = runtime(|s| {
        fixtures::library_sources(s);
        s.images(std::sync::Arc::new(|| {}));
        for image in s.provider_logos.values() {
            image.wait().expect("embedded provider logo decodes");
        }
    });
    tap(&mut rt, "nav-downloads");
    tap(&mut rt, "nav-library");

    let elements = rt.elements();
    let mut source_positions = Vec::new();
    for (row, key, name) in [("entry-0", "provider-icon-tidal", "Tidal"), ("entry-3", "source-link", "Audio Source")] {
        let source = elements.iter().find(|el| el.path.contains(row) && el.key.as_deref() == Some(key)).unwrap();
        assert_eq!(source.role, "img");
        assert_eq!(source.help.as_deref(), Some(name));
        assert_eq!(source.fill, rune::Color::TRANSPARENT);
        assert!(!elements.iter().any(|el| el.path.contains(row) && el.text.as_deref() == Some(name)));
        source_positions.push(source.rect[0]);
    }
    assert_eq!(source_positions[0], source_positions[1]);
}

#[test]
fn download_progress_keeps_rounded_caps_when_switching_to_a_percentage() {
    let mut rt = runtime(fixtures::queue_indeterminate);
    let track_and_fill = |rt: &Runtime<App>, fill_key: &str| {
        let elements = rt.elements();
        let track = elements.iter().find(|el| el.path.ends_with("/progress-1/track")).unwrap();
        let fill = elements.iter().find(|el| el.key.as_deref() == Some(fill_key) && el.path.contains("/progress-1/track/")).unwrap();
        assert_eq!(track.radius, 4.0);
        assert_eq!(fill.radius, 4.0);
        (fill.loop_phase, fill.target[2], track.target[2])
    };
    assert!(track_and_fill(&rt, "moving-fill").0.is_some());
    rt.state.jobs[0].progress = Some(0.5);
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    let (loop_phase, fill_width, track_width) = track_and_fill(&rt, "fill");
    assert!(loop_phase.is_none());
    assert!((fill_width / track_width - 0.5).abs() < 0.01);
    assert!(rt.elements().iter().any(|el| el.path.contains("/progress-1/") && el.text.as_deref() == Some("50%")));
}

#[test]
fn destructive_buttons_use_the_danger_palette() {
    let danger = theme::kit(true).c.danger;
    for (scene, key, filled) in [
        (fixtures::queue as fn(&mut App), "clear-finished", true),
        (fixtures::queue, "cancel-1", false),
        (fixtures::queue, "remove-4", false),
        (fixtures::library, "clear-history", true),
        (fixtures::library, "remove-entry-0", false),
        (fixtures::extensions_page, "remove-odesli", false),
        (fixtures::settings, "settings-clear-history", true),
    ] {
        let rt = runtime(scene);
        let elements = rt.elements();
        let (index, button) = elements.iter().enumerate().find(|(_, el)| el.key.as_deref() == Some(key)).unwrap();
        assert_eq!(button.fill, if filled { danger.bg } else { rune::Color::TRANSPARENT }, "{key}");
        let icon = elements.iter().find(|el| el.parent == Some(index) && el.key.as_deref() == Some("icon")).unwrap();
        assert_eq!(icon.text_color, Some(danger.fg), "{key}");
    }
}

#[test]
fn library_hover_highlights_actions_on_a_single_track() {
    let mut rt = runtime(fixtures::library_single);
    let (x, y) = rt.find("entry-0").unwrap();
    rt.pointer_move(x, y);
    for _ in 0..12 {
        rt.frame(1.0 / 60.0);
    }
    let elements = rt.elements();
    let entry = elements.iter().find(|el| el.key.as_deref() == Some("entry-0")).unwrap();
    assert_eq!(entry.hover, 0.0);
    assert_eq!(entry.fill, rune::Color::TRANSPARENT);

    let (x, y) = rt.find("remove-entry-0").unwrap();
    rt.pointer_move(x, y);
    for _ in 0..12 {
        rt.frame(1.0 / 60.0);
    }
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("remove-entry-0") && el.hover > 0.5));
    tap(&mut rt, "remove-entry-0");
    assert!(rt.state.history.is_empty());
}

#[test]
fn the_library_scrollbar_stays_visible_and_can_be_dragged_when_tracks_overflow() {
    let mut rt = runtime_at(fixtures::library_overflow, (1280.0, 720.0));
    rt.set_system_scroll_bars(rune::ScrollBars::Always);
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
    let ([x, y, w, h], opacity) = rt.presented("thumb").unwrap();
    assert!(w > 0.0 && h > 0.0 && opacity > 0.5);
    let ([card_x, _, card_w, _], _) = rt.presented("library-list").unwrap();
    assert!((card_x + card_w - 1256.0).abs() < 1.0);
    assert!(x >= card_x + card_w);

    let (x, y) = (x + w / 2.0, y + h / 2.0);
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x, 690.0);
    rt.frame(1.0 / 60.0);
    rt.pointer_up(x, 690.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.scroll_at_bottom("library-scroll"), Some(true));

    let mut single = runtime(fixtures::library_single);
    single.set_system_scroll_bars(rune::ScrollBars::Always);
    single.frame(1.0 / 60.0);
    let (thumb, opacity) = single.presented("thumb").unwrap();
    assert!(thumb[3] == 0.0 || opacity == 0.0);
}

#[test]
fn the_library_queues_the_chosen_lookup_and_keeps_one_history_entry_per_track() {
    let mut rt = runtime(fixtures::library_lookups);
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("lookup-0") && el.label.as_deref() == Some("Lyrics & Metadata")));
    tap(&mut rt, "lookup-0");
    let elements = rt.elements();
    let card = elements.iter().find(|el| el.key.as_deref() == Some("library-list")).unwrap();
    let panel = elements.iter().find(|el| el.key.as_deref() == Some("lookup-providers-0")).unwrap();
    assert_ne!(card.fill, panel.fill, "lookup panel recesses inside library card");
    for provider in [
        "betterlyrics", "lrclib", "lrc-red", "lyricsplus", "musixmatch", "musicbrainz",
        "spotify-web", "tidal-web", "deezer-web", "qobuz-web", "amazon-web",
    ] {
        assert!(rt.find(&format!("lookup-0-{provider}")).is_some(), "{provider} is a choice");
    }
    assert!(rt.state.jobs.is_empty(), "opening the picker does not start a lookup");
    tap(&mut rt, "lookup-0-musixmatch");
    assert_eq!(rt.state.page, Page::Downloads);
    let job = &mut rt.state.jobs[0];
    assert_eq!(job.extension.manifest.id, "musixmatch");
    assert_eq!(job.status, Status::Queued);
    assert!(job.lookup_file.as_ref().is_some_and(|p| p.extension().is_some_and(|ext| ext == "flac")));
    let id = job.id;
    job.status = Status::Running;
    job.handle = Some(crate::runner::JobHandle::detached());
    let count = rt.state.history.len();
    let saved = r#"@flacora {"event":"saved","path":"C:/Music/Golden.lrc"}"#;
    rt.state.job_event(id, crate::runner::JobEvent::Line(saved.into()));
    rt.state.job_event(id, crate::runner::JobEvent::Exit(Ok(0)));
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.jobs[0].status, Status::Done);
    assert_eq!(rt.state.history.len(), count);
    assert!(rt.find("lookup-source-1").is_some() && rt.find("reveal-1").is_some());
}

#[test]
fn a_catalog_lookup_receives_the_library_source_url() {
    let mut rt = runtime(fixtures::library_lookups);
    tap(&mut rt, "lookup-0");
    tap(&mut rt, "lookup-0-tidal-web");
    let [job] = rt.state.jobs.as_slice() else { panic!("expected one metadata lookup") };
    assert_eq!(job.extension.manifest.id, "tidal-web");
    assert_eq!(job.url, rt.state.history[0].url);
    assert!(job.lookup_file.is_some());
}

#[test]
fn catalog_home_feed_scrolls_sideways_and_sends_selected_tracks_to_downloads() {
    let mut rt = runtime(fixtures::catalog_feed);
    let viewport = rt.presented("catalog-shelf-0").unwrap().0;
    let card = "catalog-feed-item-0-0";
    let first = rt.presented(card).unwrap().0;
    assert!(rt.find("catalog-feed").is_some() && rt.find("catalog-results").is_none());
    assert!(rt.presented("catalog-shelf-1").unwrap().0[1] > viewport[1] + viewport[3]);
    assert!(rt.presented("catalog-feed-item-0-8").unwrap().0[0] > viewport[0] + viewport[2]);

    tap(&mut rt, card);
    assert!(rt.state.catalog.selected.contains("https://www.deezer.com/track/3412534581"));
    tap(&mut rt, "catalog-shelf-0-next");
    for _ in 0..28 {
        rt.frame(1.0 / 60.0);
    }
    assert!(rt.presented(card).unwrap().0[0] < first[0] - 100.0);
    tap(&mut rt, "catalog-shelf-0-prev");
    for _ in 0..28 {
        rt.frame(1.0 / 60.0);
    }
    assert!((rt.presented(card).unwrap().0[0] - first[0]).abs() < 1.0);

    tap(&mut rt, "catalog-use");
    assert_eq!(rt.state.page, Page::Downloads);
    assert!(rt.state.url.text.contains("https://www.deezer.com/track/3412534581"));
}

#[test]
fn catalog_selection_prepares_tracks_without_enqueuing_a_collection() {
    let mut rt = runtime(fixtures::catalog_link);
    tap(&mut rt, "download");
    assert_eq!(rt.state.page, Page::Catalog);
    assert!(rt.state.jobs.is_empty());
    assert!(rt.state.url.text.is_empty());

    let mut rt = runtime(fixtures::catalog_results);
    assert!(rt.find("catalog-item-1").is_some());
    assert!(rt.find("catalog-artwork-0").is_some() && rt.find("catalog-artwork-2").is_some());
    tap(&mut rt, "catalog-item-0");
    tap(&mut rt, "catalog-use");
    assert_eq!(rt.state.page, Page::Downloads);
    assert_eq!(rt.state.url.text, "https://www.deezer.com/track/3412534581");
    assert!(rt.state.jobs.is_empty());
    tap(&mut rt, "nav-catalog");
    tap(&mut rt, "catalog-provider-spotify-web");
    assert!(rt.state.catalog.result.is_none() && rt.state.catalog.selected.is_empty());
}

#[test]
fn catalog_scrollbar_reaches_the_centered_load_more_button() {
    let mut rt = runtime_at(fixtures::catalog_overflow, (1280.0, 720.0));
    rt.set_system_scroll_bars(rune::ScrollBars::Always);
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
    let ([bar_x, bar_y, bar_w, bar_h], opacity) = rt.presented("thumb").unwrap();
    assert!(bar_w > 0.0 && bar_h > 0.0 && opacity > 0.5);
    let ([controls_x, _, controls_w, _], _) = rt.presented("catalog-controls").unwrap();
    assert!(bar_x >= controls_x + controls_w);

    let (x, y) = (bar_x + bar_w / 2.0, bar_y + bar_h / 2.0);
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x, 690.0);
    rt.frame(1.0 / 60.0);
    rt.pointer_up(x, 690.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.scroll_at_bottom("catalog-results"), Some(true));

    let ([more_x, _, more_w, _], _) = rt.presented("catalog-more").unwrap();
    assert!(more_w < controls_w / 4.0);
    assert!((more_x + more_w / 2.0 - (controls_x + controls_w / 2.0)).abs() < 1.0);
    tap(&mut rt, "catalog-item-11");
    assert!(rt.state.catalog.selected.contains("https://www.deezer.com/track/12"));
    assert!(rt.find("catalog-more").is_some());
}

#[test]
fn catalog_reports_when_no_installed_extension_supports_a_link() {
    let mut rt = runtime(|s| {
        fixtures::catalog_missing_extension(s);
        s.catalog.error = None;
    });
    tap(&mut rt, "catalog-submit");
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("catalog-error")
        && el.text.as_deref() == Some(crate::catalog::MISSING_CATALOG_EXTENSION)));
    assert!(rt.state.catalog.result.is_none(), "stale results clear when opening an unsupported URL");
    assert!(rt.find("catalog-open-extensions").is_some());
    tap(&mut rt, "catalog-open-extensions");
    assert_eq!(rt.state.page, Page::Extensions);
}

#[test]
fn fresh_downloads_hides_composer_and_extension_cards_show_catalog_badge_and_active_gear() {
    let fresh = runtime(fixtures::fresh);
    assert!(fresh.find("composer").is_none(), "fresh onboarding hides disabled composer");
    assert!(fresh.find("go-extensions").is_some());

    let mut opts = runtime(fixtures::extension_options);
    opts.state.installed.push(fixtures::installed("deezer-web"));
    opts.invalidate();
    opts.frame(0.0);
    assert!(opts.find("catalog-badge-deezer-web").is_some());
    let elements = opts.elements();
    let open_gear = elements.iter().find(|el| el.key.as_deref() == Some("configure-odskyler")).unwrap();
    let closed_gear = elements.iter().find(|el| el.key.as_deref() == Some("configure-cyan-dl")).unwrap();
    assert_ne!(open_gear.fill, closed_gear.fill, "expanded settings gear has an active fill");
}

#[test]
fn a_resolved_link_offers_the_services_it_was_found_on() {
    let mut rt = runtime(fixtures::choose);
    let id = rt.state.jobs[0].id;
    assert_eq!(rt.state.jobs[0].status, Status::Choosing);
    for i in 0..3 {
        assert!(rt.find(&format!("choose-{id}-{i}")).is_some(), "choice {i} is offered");
    }
    assert!(rt.find(&format!("choose-{id}-3")).is_none(), "Pandora has no downloader, Spotify is the source");
    tap(&mut rt, &format!("choose-{id}-1"));
    let job = &rt.state.jobs[0];
    assert_eq!((job.status.clone(), job.service.map(|s| s.key)), (Status::Queued, Some("deezer")));
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

    // Bulk Cancel All (two-step confirm) and Retry Failed buttons appear and act on jobs.
    assert!(rt.find("cancel-all").is_some());
    assert!(rt.find("retry-failed").is_some());
    tap(&mut rt, "cancel-all");
    assert_eq!(rt.state.confirm_action, Some(crate::app::ConfirmAction::CancelAll));
    assert!(rt.state.jobs.iter().any(|j| j.status == Status::Queued), "first click arms confirmation");
    tap(&mut rt, "cancel-all");
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
    tap(&mut q, "clear-finished");
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
    let mut rt = runtime(fixtures::settings);
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
    assert!(rt.find("copy-failed").is_some());
    tap(&mut rt, "copy-failed");
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
