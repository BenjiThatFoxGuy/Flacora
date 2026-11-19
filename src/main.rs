//! Flacora: a Rune GUI for music downloaders that install as extensions.
//! `cargo run` opens the window; `-- --snap out.png --scene queue` renders a scene headless
//! (`--list-scenes` lists them). Headless runs show fixed data; set
//! `FLACORA_HEADLESS_LIVE=1` to load locally installed extensions instead.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod catalog;
mod extensions;
mod fixtures;
mod icons;
mod jobs;
mod library;
mod output;
mod package_picker;
mod protocol;
mod runner;
mod services;
mod store;
mod track_links;
mod ui;
mod updates;
#[cfg(windows)]
mod window;

use app::App;
use rune::AppSpec;

const WINDOW_SIZE: (f32, f32) = (1280.0, 720.0);

/// Flacora's icon (1024 px PNG): the window and taskbar icon and the sidebar logo.
pub const LOGO: &[u8] = include_bytes!("../assets/appicon.png");

fn setup(s: &mut App, waker: rune::Waker) {
    s.images(waker.clone());
    let headless = rune::tooling::headless_run().is_some();
    if headless {
        // Decode embedded artwork before the first snapshot.
        for logo in s.logo.iter().chain(s.project_logos.iter().flatten()).chain(s.provider_logos.values()) {
            let _ = logo.wait();
        }
    }
    if !headless || std::env::var("FLACORA_HEADLESS_LIVE").is_ok_and(|v| v == "1") {
        s.connect(waker);
    }
}

fn poll(s: &mut App, clock: f64, tx: &mut rune::Tx) -> bool {
    let received = s.drain();
    s.tick_notifications(clock, tx) || received
}

pub(crate) fn on_key(s: &mut App, ev: &rune::KeyEvent, tx: &mut rune::Tx) -> bool {
    use app::Page;
    use rune::Key;
    match &ev.key {
        Key::Command('1') => {
            s.page = Page::Downloads;
            s.confirm_action = None;
            true
        }
        Key::Command('2') => {
            s.enter_catalog();
            s.confirm_action = None;
            true
        }
        Key::Command('3') => {
            s.page = Page::Library;
            s.confirm_action = None;
            s.refresh_library_files();
            true
        }
        Key::Command('4') => {
            s.page = Page::Extensions;
            s.confirm_action = None;
            true
        }
        Key::Command('5') => {
            s.page = Page::Settings;
            s.confirm_action = None;
            true
        }
        Key::Command('l') => {
            s.page = Page::Downloads;
            s.confirm_action = None;
            tx.focus_when_shown("url", false);
            true
        }
        Key::Command('f') => {
            s.confirm_action = None;
            if s.page == Page::Library && !s.history.is_empty() {
                tx.focus_when_shown("search", false);
            } else {
                s.enter_catalog();
                tx.focus_when_shown("catalog-input", false);
            }
            true
        }
        Key::Escape => {
            if s.confirm_action.take().is_some() {
                return true;
            }
            if s.lookup_picker.take().is_some() {
                return true;
            }
            if s.extension_settings.take().is_some() {
                return true;
            }
            if s.page == Page::Catalog && !s.catalog.selected.is_empty() {
                s.catalog.selected.clear();
                return true;
            }
            false
        }
        _ => false,
    }
}

fn main() {
    let _ = rune::set_app_icon(LOGO);
    let app = rune::App::new(AppSpec::<App> {
        min_size: Some(WINDOW_SIZE),
        fonts: &[ui::FONT],
        families: ("Figtree", "ui-monospace"),
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        scenes: &[
            ("home", fixtures::home),
            ("queue", fixtures::queue),
            ("queue-indeterminate", fixtures::queue_indeterminate),
            ("choose", fixtures::choose),
            ("resolvers", fixtures::resolvers),
            ("light", fixtures::light),
            ("missing", fixtures::missing),
            ("spotify", fixtures::spotify),
            ("fresh", fixtures::fresh),
            ("library", fixtures::library),
            ("library-sources", fixtures::library_sources),
            ("library-single", fixtures::library_single),
            ("library-overflow", fixtures::library_overflow),
            ("lookup-picker", fixtures::lookup_picker),
            ("catalog", fixtures::catalog_results),
            ("catalog-feed", fixtures::catalog_feed),
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
            ("extensions-drop", fixtures::extensions_dropping),
            ("extensions-installing", fixtures::extensions_installing),
            ("imported", fixtures::imported),
            ("notifications", fixtures::notifications),
            ("settings", fixtures::settings),
            ("settings-update-available", fixtures::settings_update_available),
            ("output-settings", fixtures::output_settings),
            ("settings-without-ffmpeg", fixtures::settings_without_ffmpeg),
            ("settings-without-browser", fixtures::settings_without_browser),
            ("drop", fixtures::dropping),
            ("loading", fixtures::loading),
        ],
        ..AppSpec::new("Flacora", WINDOW_SIZE, App::default, ui::view)
    });
    let app_handle = app.handle();
    let app = app.on_window(move |handle| {
        #[cfg(windows)]
        window::center(handle);
        let handle = handle.clone();
        app_handle.update_if(move |s, _| {
            s.window = Some(handle);
            false
        });
    });
    let args = ["--scroll-bars", "always"].map(str::to_owned).into_iter().chain(std::env::args().skip(1));
    app.args(args).run();
}
