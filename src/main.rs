//! Flacora: a Rune GUI for music downloaders that install as extensions.
//! `cargo run` opens the window; `-- --snap out.png --scene queue` renders a scene headless
//! (`--list-scenes` lists them). Headless runs show fixed data; set
//! `FLACORA_HEADLESS_LIVE=1` to load locally installed extensions instead.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use flacora::app::App;
#[cfg(windows)]
use flacora::window;
use flacora::{fixtures, gui, ui};
use rune::AppSpec;

fn setup(s: &mut App, waker: rune::Waker) {
    s.images(waker.clone());
    s.waker = Some(waker.clone());
    let headless = rune::tooling::headless_run().is_some();
    if headless {
        // Decode embedded artwork before the first snapshot.
        for logo in s.logo.iter().chain(s.project_logos.iter().flatten()).chain(s.provider_logos.values()) {
            let _ = logo.wait();
        }
        if std::env::var("FLACORA_HEADLESS_LIVE").is_ok_and(|v| v == "1") {
            s.connect(waker);
        }
    }
}

fn main() {
    rune::prewarm_gpu();
    let _ = rune::set_app_icon(gui::LOGO);
    let app = rune::App::new(AppSpec::<App> {
        min_size: Some(gui::MIN_WINDOW_SIZE),
        fonts: ui::FONTS,
        families: ("Figtree", "ui-monospace"),
        setup: Some(setup),
        poll: Some(gui::poll),
        on_key: Some(gui::on_key),
        scenes: &[
            ("home", fixtures::home),
            ("downloads-batch-input", fixtures::downloads_batch_input),
            ("queue", fixtures::queue),
            ("queue-actions", fixtures::queue_actions),
            ("queue-cancel-confirmation", fixtures::queue_cancel_confirmation),
            ("queue-indeterminate", fixtures::queue_indeterminate),
            ("choose", fixtures::choose),
            ("resolvers", fixtures::resolvers),
            ("light", fixtures::light),
            ("missing", fixtures::missing),
            ("spotify", fixtures::spotify),
            ("fresh", fixtures::fresh),
            ("library", fixtures::library),
            ("library-search", fixtures::library_search),
            ("library-sources", fixtures::library_sources),
            ("library-single", fixtures::library_single),
            ("library-overflow", fixtures::library_overflow),
            ("lookup-picker", fixtures::lookup_picker),
            ("library-outdated-metadata", fixtures::library_outdated_metadata),
            ("catalog", fixtures::catalog_results),
            ("catalog-feed", fixtures::catalog_feed),
            ("catalog-feed-hidden", fixtures::catalog_feed_hidden),
            ("catalog-idle", fixtures::catalog_idle),
            ("catalog-resolver-sources", fixtures::catalog_with_resolver_sources),
            ("catalog-batch", fixtures::catalog_batch),
            ("catalog-empty", fixtures::catalog_empty),
            ("catalog-link", fixtures::catalog_link),
            ("catalog-overflow", fixtures::catalog_overflow),
            ("catalog-missing-extension", fixtures::catalog_missing_extension),
            ("lookup-queue", fixtures::lookup_queue),
            ("extensions", fixtures::extensions_page),
            ("extension-badges", fixtures::extension_badges),
            ("extension-status", fixtures::extension_status),
            ("extension-options", fixtures::extension_options),
            ("extensions-empty", fixtures::extensions_empty),
            ("extensions-remove-confirmation", fixtures::extensions_remove_confirmation),
            ("extensions-drop", fixtures::extensions_dropping),
            ("extensions-installing", fixtures::extensions_installing),
            ("imported", fixtures::imported),
            ("notifications", fixtures::notifications),
            ("settings", fixtures::settings),
            ("settings-matcha", fixtures::settings_matcha),
            ("settings-update-available", fixtures::settings_update_available),
            ("output-settings", fixtures::output_settings),
            ("settings-auto-tag-available", fixtures::auto_tag_available_settings),
            ("settings-auto-tag", fixtures::auto_tag_settings),
            ("settings-without-ffmpeg", fixtures::settings_without_ffmpeg),
            ("settings-without-browser", fixtures::settings_without_browser),
            ("drop", fixtures::dropping),
            ("loading", fixtures::loading),
        ],
        ..AppSpec::new("Flacora", MIN_WINDOW_SIZE, App::default, ui::view)
    });
    let app_handle = app.handle();
    let ready_handle = app.handle();
    let app = app
        .on_window(move |handle| {
            #[cfg(windows)]
            window::center(handle);
            let handle = handle.clone();
            app_handle.update_if(move |s, _| {
                s.window = Some(handle);
                false
            });
        })
        .on_ready(move |_| {
            ready_handle.update_if(|s, _| {
                if s.link.is_none()
                    && let Some(waker) = s.waker.clone()
                {
                    s.connect(waker);
                }
                false
            });
        });
    let args = ["--scroll-bars", "always"].map(str::to_owned).into_iter().chain(std::env::args().skip(1));
    app.args(args).run();
}
