//! Window-level helpers shared by the GUI binary and the UI tests.
//!
//! These live in the library because `ui::tests` reaches them as `crate::` items.

use crate::app::{App, Page};
use rune::{Chord, ChordKey, ChordMod, KeyEvent, Tx};

/// The smallest window the GUI opens at.
pub const MIN_WINDOW_SIZE: (f32, f32) = (1024.0, 600.0);

/// Flacora's icon (1024 px PNG): the window and taskbar icon and the sidebar logo.
pub const LOGO: &[u8] = include_bytes!("../assets/appicon.png");

/// Drains background messages once per frame and advances notification timers.
pub fn poll(s: &mut App, clock: f64, tx: &mut Tx) -> bool {
    let received = s.drain_with(clock, tx);
    s.tick_notifications(clock, tx) || received
}

/// The GUI's global keyboard shortcuts.
pub fn on_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    const MOD_1: Chord = Chord::new(ChordKey::Char('1')).with(ChordMod::Primary);
    const MOD_2: Chord = Chord::new(ChordKey::Char('2')).with(ChordMod::Primary);
    const MOD_3: Chord = Chord::new(ChordKey::Char('3')).with(ChordMod::Primary);
    const MOD_4: Chord = Chord::new(ChordKey::Char('4')).with(ChordMod::Primary);
    const MOD_5: Chord = Chord::new(ChordKey::Char('5')).with(ChordMod::Primary);
    const MOD_L: Chord = Chord::new(ChordKey::Char('l')).with(ChordMod::Primary);
    const MOD_F: Chord = Chord::new(ChordKey::Char('f')).with(ChordMod::Primary);
    const ESC: Chord = Chord::new(ChordKey::Escape);

    if MOD_1.matches_tx(ev, tx) {
        s.page = Page::Downloads;
        s.dismiss_page_overlays();
        true
    } else if MOD_2.matches_tx(ev, tx) {
        s.page = Page::Catalog;
        s.dismiss_page_overlays();
        true
    } else if MOD_3.matches_tx(ev, tx) {
        s.page = Page::Library;
        s.dismiss_page_overlays();
        s.refresh_library_files();
        true
    } else if MOD_4.matches_tx(ev, tx) {
        s.page = Page::Extensions;
        s.dismiss_page_overlays();
        true
    } else if MOD_5.matches_tx(ev, tx) {
        s.page = Page::Settings;
        s.dismiss_page_overlays();
        true
    } else if MOD_L.matches_tx(ev, tx) {
        s.page = Page::Downloads;
        s.dismiss_page_overlays();
        tx.focus_when_shown("url", false);
        true
    } else if MOD_F.matches_tx(ev, tx) {
        s.dismiss_page_overlays();
        if s.page == Page::Library && !s.history.is_empty() {
            tx.focus_when_shown("search", false);
        } else {
            s.page = Page::Catalog;
            tx.focus_when_shown("catalog-input", false);
        }
        true
    } else if ESC.matches_tx(ev, tx) {
        if std::mem::take(&mut s.queue_menu_open) || s.confirm_action.take().is_some() {
            return true;
        }
        if let Some((date, file)) = s.lookup_picker.take() {
            if s.page == Page::Library
                && let Some(index) = s.history.iter().position(|entry| {
                    entry.date == date && entry.flac_file() == Some(file.as_str())
                })
            {
                tx.focus_when_shown(format!("lookup-{index}"), false);
            }
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
    } else {
        false
    }
}
