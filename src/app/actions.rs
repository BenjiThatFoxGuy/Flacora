//! Routing, settings persistence, extension management, and library actions.

use super::{App, LibrarySort, Msg, RESOLVER_KEY, SaveState, UndoAction};
use crate::{
    extensions::{self, Kind},
    library::{self, Entry},
    package_picker, runner,
    services::{self, Service},
    store::{self, ExtensionStatus, Installed, Theme, ThemePreset},
};
use rune::{FileDialogOptions, Tx};
use std::path::PathBuf;

impl App {
    pub fn dismiss_page_overlays(&mut self) {
        self.queue_menu_open = false;
        self.confirm_action = None;
    }

    // ------------------------------------------------------------ routing

    /// Host of the first link being typed.
    pub fn host(&self) -> Option<String> {
        self.url.text.split_whitespace().find_map(extensions::url_host)
    }

    pub fn service(&self) -> Option<&'static Service> {
        services::detect(&self.host()?)
    }

    pub fn batch_has_downloader(&self, service: &Service) -> bool {
        self.installed_downloaders_for(service.primary_host())
            .iter().any(|extension| !self.removing.contains(&extension.manifest.id))
    }

    pub fn batch_service(&self) -> Option<&'static Service> {
        let (draft, service) = self.batch_destination.as_ref()?;
        (draft == &self.url.text && self.batch_has_downloader(service)).then_some(*service)
    }

    pub fn set_batch_service(&mut self, service: Option<&'static Service>) {
        self.batch_destination = service.map(|service| (self.url.text.clone(), service));
    }

    /// Installed downloaders that accept links on `host`, best first, including those
    /// currently reporting a temporary outage (used in Settings).
    pub fn installed_downloaders_for(&self, host: &str) -> Vec<&Installed> {
        if services::is_excluded_host(host) {
            return Vec::new();
        }
        let mut found: Vec<&Installed> = self
            .installed
            .iter()
            .filter(|i| i.manifest.kind == Kind::Downloader && extensions::host_matches(&i.manifest.hosts, host))
            .collect();
        found.sort_by_key(|i| (extensions::rank(&i.manifest.id), i.manifest.name.to_lowercase()));
        found
    }

    /// Installed downloaders that accept links on `host`, best first. A downloader whose status
    /// check says the link's service is down right now is left out.
    pub fn candidates_for(&self, host: &str) -> Vec<&Installed> {
        let service = services::detect(host);
        let mut found: Vec<&Installed> = self
            .installed_downloaders_for(host)
            .into_iter()
            .filter(|i| !self.removing.contains(&i.manifest.id))
            .filter(|i| match (self.extension_status.get(&i.manifest.id), service) {
                (Some(ExtensionStatus::Services(live)), Some(service)) => live.iter().any(|s| s == service.name),
                _ => true,
            })
            .collect();
        found.sort_by_key(|i| (extensions::rank(&i.manifest.id), i.manifest.name.to_lowercase()));
        found
    }

    /// Installed resolvers that take links on `host`, best first.
    pub fn resolvers_for(&self, host: &str) -> Vec<&Installed> {
        if services::is_excluded_host(host) {
            return Vec::new();
        }
        let mut found: Vec<&Installed> = self.installed.iter()
            .filter(|i| i.manifest.kind == Kind::Resolver && !self.removing.contains(&i.manifest.id)
                && extensions::host_matches(&i.manifest.hosts, host))
            .collect();
        found.sort_by_key(|i| (extensions::rank(&i.manifest.id), i.manifest.name.to_lowercase()));
        found
    }

    /// The resolver that takes links on `host` when no downloader does: the preferred one
    /// (stored under [`RESOLVER_KEY`]) or the best.
    pub fn resolver_for(&self, host: &str) -> Option<&Installed> {
        if !self.candidates_for(host).is_empty() {
            return None;
        }
        let found = self.resolvers_for(host);
        let preferred = self.settings.preferred.get(RESOLVER_KEY);
        found.iter().find(|i| Some(&i.manifest.id) == preferred).or(found.first()).copied()
    }

    pub fn resolver(&self) -> Option<&Installed> {
        self.resolver_for(&self.host()?)
    }

    pub fn resolvers(&self) -> Vec<&Installed> {
        self.host().map(|h| self.resolvers_for(&h)).unwrap_or_default()
    }

    pub fn candidates(&self) -> Vec<&Installed> {
        self.host().map(|h| self.candidates_for(&h)).unwrap_or_default()
    }

    /// Key under which the preferred extension for a host is stored.
    pub fn service_key(host: &str) -> String {
        services::detect(host).map_or_else(|| host.to_owned(), |s| s.key.to_owned())
    }

    pub fn selected_for(&self, host: &str) -> Option<&Installed> {
        let candidates = self.candidates_for(host);
        let preferred = self.settings.preferred.get(&Self::service_key(host));
        candidates.iter().find(|i| Some(&i.manifest.id) == preferred).or(candidates.first()).copied()
    }

    pub fn selected(&self) -> Option<&Installed> {
        self.selected_for(&self.host()?)
    }

    pub fn pick_index(&self, extension: &str, option: &extensions::ExtOption) -> usize {
        self.settings
            .options
            .get(&format!("{extension}/{}", option.key))
            .and_then(|value| option.choices.iter().position(|c| &c.value == value))
            .unwrap_or_else(|| option.default_index())
    }

    pub fn set_pick(&mut self, extension: &str, option: &str, index: usize) {
        let value = self
            .installed
            .iter()
            .find(|i| i.manifest.id == extension)
            .and_then(|i| i.manifest.options.iter().find(|o| o.key == option))
            .and_then(|o| o.choices.get(index))
            .map(|c| c.value.clone());
        if let Some(value) = value {
            self.settings.options.insert(format!("{extension}/{option}"), value);
            self.save_settings();
        }
    }

    pub fn set_preferred(&mut self, service_key: &str, extension: Option<String>) {
        match extension {
            Some(id) => self.settings.preferred.insert(service_key.to_owned(), id),
            None => self.settings.preferred.remove(service_key),
        };
        self.save_settings();
    }

    // ------------------------------------------------------------ settings

    pub fn set_output(&mut self, output: PathBuf) {
        self.settings.output = output;
        self.save_settings();
    }

    pub fn set_output_layout(&mut self, layout: crate::output::Layout) {
        self.settings.layout = layout;
        self.save_settings();
    }

    pub fn toggle_auto_tag(&mut self) {
        self.settings.auto_tag = !self.settings.auto_tag;
        self.save_settings();
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.settings.theme = theme;
        self.save_settings();
    }

    pub fn set_theme_preset(&mut self, preset: ThemePreset) {
        if self.settings.theme_preset != preset {
            self.settings.theme_preset = preset;
            self.save_settings();
        }
    }

    pub(super) fn save_settings(&mut self) {
        if self.link.is_none() {
            return;
        }
        if !self.settings_save.request() {
            return;
        }
        let (paths, settings) = (self.paths.clone(), self.settings.clone());
        self.background(move || Msg::SettingsSaved(store::save_settings(&paths, &settings)));
    }

    pub fn refresh_tools(&self) {
        let paths = self.paths.clone();
        self.background(move || Msg::Tools(store::tools(&paths)));
    }

    pub fn refresh_tools_if_missing(&self) {
        let missing = self
            .tools
            .as_ref()
            .is_none_or(|t| t.ffmpeg.is_none() || t.browser.is_none());
        if missing {
            self.refresh_tools();
        }
    }

    // ------------------------------------------------------------ extensions

    pub fn remove(&mut self, id: &str) {
        if self.link.is_none()
            || self.importing > 0
            || self.installed.iter().any(|i| i.manifest.id == id && self.status_command_running(i))
        {
            return;
        }
        if self.running_extension(id) {
            self.warn("Wait for this extension's active jobs to finish before removing it.");
            return;
        }
        if !self.removing.insert(id.to_owned()) {
            return;
        }
        let (paths, id) = (self.paths.clone(), id.to_owned());
        self.background(move || Msg::Removed(id.clone(), store::remove(&paths, &id)));
    }

    pub fn remove_all_extensions_blocker(&self) -> Option<&'static str> {
        if self.importing > 0 {
            Some("Wait for the current installation to finish.")
        } else if !self.removing.is_empty() {
            Some("Wait for the current removal to finish.")
        } else if self.installed.iter().any(|i| self.running_extension(&i.manifest.id) || self.status_command_running(i)) {
            Some("Wait for active jobs and extension checks to finish.")
        } else {
            None
        }
    }

    pub fn remove_all_extensions(&mut self) {
        if self.link.is_none() || self.installed.is_empty() {
            return;
        }
        if let Some(reason) = self.remove_all_extensions_blocker() {
            self.warn(reason);
            return;
        }
        self.confirm_action = None;
        let ids: Vec<_> = self.installed.iter().map(|i| i.manifest.id.clone()).collect();
        self.removing.extend(ids.iter().cloned());
        let paths = self.paths.clone();
        self.background(move || Msg::ExtensionsRemoved(ids.into_iter().map(|id| {
            let result = store::remove(&paths, &id);
            (id, result)
        }).collect()));
    }

    pub fn browse_packages(&mut self, tx: &mut Tx) {
        if self.browsing_packages {
            return;
        }
        if let Some(window) = &self.window {
            if self.link.is_none() {
                return;
            }
            match package_picker::open(window) {
                Ok(pick) => {
                    self.browsing_packages = true;
                    self.background(move || Msg::PackagesPicked(pick()));
                }
                Err(error) => self.error(format!("Could not open package picker: {error}")),
            }
            return;
        }

        // Rune's scripted dialog accepts one path when no native window is present.
        let options = FileDialogOptions { title: package_picker::TITLE.into(), ..Default::default() }
            .filter(package_picker::FILTER, &package_picker::EXTENSIONS);
        tx.open_file(options, |s: &mut App, result, _| match result {
            Ok(Some(path)) => s.apply(Msg::PackagesPicked(vec![path])),
            Ok(None) => {}
            Err(error) => s.error(format!("Could not open package picker: {error}")),
        });
    }

    /// Queues local extension packages.
    pub fn install_files(&mut self, files: &[PathBuf]) {
        self.drop_over = false;
        let was_idle = self.importing == 0;
        for file in files {
            let accepted = file.extension().is_some_and(|e| store::PACKAGE_EXTENSIONS.iter().any(|ext| e.eq_ignore_ascii_case(ext)));
            if !accepted {
                self.reject_files(std::slice::from_ref(file));
                continue;
            }
            self.importing += 1;
            self.info = None;
            self.pending_imports.push_back(file.clone());
        }
        if self.loaded && was_idle {
            self.start_import();
        }
    }

    /// Installs one package at a time so two drops cannot race to replace the same folder.
    pub(super) fn start_import(&mut self) {
        let Some(file) = self.pending_imports.pop_front() else { return };
        let mut in_use = self.removing.clone();
        in_use.extend(self.installed.iter().filter(|i| self.status_command_running(i)).map(|i| i.manifest.id.clone()));
        in_use.extend(self.catalog.active_extension().map(str::to_owned));
        in_use.extend(self.jobs.iter().filter(|j| !j.is_finished()).map(|j| j.extension.manifest.id.clone()));
        let paths = self.paths.clone();
        self.background(move || {
            let result = store::install_file(&paths, &file, |id| !in_use.contains(id)).map(Box::new);
            Msg::Imported(file, result)
        });
    }

    pub fn wait_for_import(&mut self) -> bool {
        if self.importing == 0 {
            return false;
        }
        self.warn("Wait for the extension packages to finish installing.");
        true
    }

    pub fn reject_files(&mut self, files: &[PathBuf]) {
        self.drop_over = false;
        let names: Vec<String> = files.iter().filter_map(|f| f.file_name()).map(|n| n.to_string_lossy().into_owned()).collect();
        self.warn(format!("{} is not an extension. Drop a .{} file to install one.", names.join(", "), store::PACKAGE));
    }

    pub fn running_extension(&self, id: &str) -> bool {
        self.catalog.running_extension(id)
            || self.jobs.iter().any(|j| j.extension.manifest.id == id && !j.is_finished())
    }

    // ------------------------------------------------------------ library

    pub fn remove_entry(&mut self, index: usize) {
        if index < self.history.len() {
            let saved = self.history.clone();
            let title = self.history[index].title.clone();
            self.lookup_picker = None;
            self.history.remove(index);
            self.save_history();
            self.success_undo(format!("Removed \"{title}\" from Library"), UndoAction::History(saved));
        }
    }

    pub fn clear_history(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let saved = std::mem::take(&mut self.history);
        let count = saved.len();
        self.lookup_picker = None;
        self.cover_loading.clear();
        self.save_history();
        self.success_undo(
            format!("Cleared {count} library item{}", if count == 1 { "" } else { "s" }),
            UndoAction::History(saved),
        );
    }

    pub fn save_history(&mut self) {
        if self.link.is_none() {
            return;
        }
        if !self.history_save.request() {
            return;
        }
        let (paths, history) = (self.paths.clone(), self.history.clone());
        self.background(move || Msg::HistorySaved(library::save(&paths, &history)));
    }

    pub fn active_library_service(&self) -> Option<&str> {
        self.library_service.as_deref().filter(|service| {
            self.history.iter().any(|entry| entry.service.eq_ignore_ascii_case(service))
        })
    }

    /// History entries matching the service filter and search box, sorted by `library_sort`.
    pub fn filtered_history(&self) -> Vec<(usize, &Entry)> {
        let query = self.search.text.trim();
        let service_filter = self.active_library_service();
        let ascii = query.is_ascii();
        let lower = (!query.is_empty() && !ascii).then(|| query.to_lowercase());
        let mut entries: Vec<(usize, &Entry)> = self
            .history
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                if let Some(svc) = service_filter
                    && !e.service.eq_ignore_ascii_case(svc)
                {
                    return false;
                }
                if query.is_empty() {
                    return true;
                }
                [&e.title, &e.artist, &e.album, &e.service, &e.extension, &e.quality].iter().any(|field| {
                    if ascii {
                        contains_ignore_ascii_case(field, query)
                    } else {
                        field.to_lowercase().contains(lower.as_deref().unwrap_or(""))
                    }
                })
            })
            .collect();
        match self.library_sort {
            LibrarySort::Newest => {}
            LibrarySort::Title => entries.sort_by_cached_key(|(_, e)| (e.title.to_lowercase(), e.artist.to_lowercase())),
            LibrarySort::Artist => entries.sort_by_cached_key(|(_, e)| (e.artist.to_lowercase(), e.title.to_lowercase())),
        }
        entries
    }

    pub fn refresh_library_files(&self) {
        if self.link.is_none() || self.history.is_empty() {
            return;
        }
        let paths: Vec<String> = self.history.iter().filter_map(|e| e.main_file().map(str::to_owned)).collect();
        self.background(move || Msg::LibraryFiles(library::inspect_files(&paths)));
    }

    pub fn open(&self, path: &str) {
        let path = PathBuf::from(path);
        self.background_opt(move || runner::open_path(&path).err().map(Msg::Error));
    }

    pub fn reveal(&self, path: &str) {
        let path = PathBuf::from(path);
        self.background_opt(move || runner::reveal(&path).err().map(Msg::Error));
    }

    pub fn open_output(&self) {
        let output = self.settings.output.clone();
        self.background_opt(move || {
            if let Err(error) = std::fs::create_dir_all(&output) {
                return Some(Msg::Error(format!("Could not prepare download folder: {error}")));
            }
            runner::open_path(&output).err().map(Msg::Error)
        });
    }
}

impl SaveState {
    pub(super) fn request(&mut self) -> bool {
        match self {
            SaveState::Idle => {
                *self = SaveState::Saving;
                true
            }
            SaveState::Saving | SaveState::Dirty => {
                *self = SaveState::Dirty;
                false
            }
        }
    }

    pub(super) fn finish(&mut self) -> bool {
        let needs_rerun = matches!(self, SaveState::Dirty);
        *self = SaveState::Idle;
        needs_rerun
    }
}

fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    let n = needle.as_bytes();
    if n.is_empty() {
        return true;
    }
    haystack.as_bytes().windows(n.len()).any(|w| w.eq_ignore_ascii_case(n))
}
