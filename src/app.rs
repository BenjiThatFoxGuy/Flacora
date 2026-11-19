//! App state, messages from background threads, and user actions.
//! The download queue lives in `jobs`.

use crate::{
    extensions::{self, Kind},
    jobs::Job,
    library::{self, Entry},
    package_picker,
    runner::{self, JobEvent},
    services::{self, Service},
    store::{self, ExtensionStatus, Installed, Paths, Settings, Theme, Tools},
    updates,
};
use rune::{FileDialogOptions, Image, ImageCache, TextEdit, Tx, Waker};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, channel},
    },
    thread,
};

/// Key in `settings.preferred` of the resolver to use when several are installed.
pub const RESOLVER_KEY: &str = "resolver";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Downloads,
    Catalog,
    Library,
    Extensions,
    Settings,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum LibrarySort {
    #[default]
    Newest,
    Title,
    Artist,
}

impl LibrarySort {
    pub const ALL: [LibrarySort; 3] = [LibrarySort::Newest, LibrarySort::Title, LibrarySort::Artist];

    pub fn label(self) -> &'static str {
        match self {
            LibrarySort::Newest => "Newest",
            LibrarySort::Title => "Title",
            LibrarySort::Artist => "Artist",
        }
    }
}

#[derive(Clone, Copy)]
pub enum NotificationKind {
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfirmAction {
    CancelAll,
    RemoveExtension(String),
}

pub enum UndoAction {
    History(Vec<Entry>),
    FinishedJobs(Vec<Job>),
}

pub struct Notification {
    pub kind: NotificationKind,
    pub message: String,
    expires_at: Option<f64>,
}

pub enum Msg {
    Loaded { installed: Vec<Installed>, settings: Settings, history: Vec<Entry> },
    PackagesPicked(Vec<PathBuf>),
    /// A package file was installed (or refused).
    Imported(PathBuf, Result<Box<Installed>, String>),
    Removed(String, Result<(), String>),
    Tools(Tools),
    UpdateChecked(Option<updates::Release>),
    ExtensionStatus { id: String, check: u64, status: ExtensionStatus },
    SettingsSaved(Result<(), String>),
    HistorySaved(Result<(), String>),
    Job(u64, JobEvent),
    JobStaging(u64, tempfile::TempDir),
    Catalog(u64, JobEvent),
    CatalogArtwork(String, Option<Vec<u8>>),
    DownloadPlaced(u64, Result<Vec<String>, String>),
    /// Cover art for a job: the cached file and its bytes.
    JobCover(u64, PathBuf, Vec<u8>),
    /// Bytes of a cached cover, for the history.
    Image(String, Vec<u8>),
    LibraryFiles(BTreeMap<String, library::SidecarInfo>),
    Error(String),
}

/// Channel from background threads to `poll`, and the waker that runs it.
pub struct Link {
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    waker: Waker,
}

impl Link {
    pub fn new(waker: Waker) -> Link {
        let (tx, rx) = channel();
        Link { tx, rx, waker }
    }

    pub fn sender(&self) -> impl Fn(Msg) + Send + Sync + Clone + 'static {
        let tx = Arc::new(Mutex::new(self.tx.clone()));
        let waker = self.waker.clone();
        move |msg| {
            if let Ok(tx) = tx.lock() {
                let _ = tx.send(msg);
            }
            waker();
        }
    }
}

pub struct App {
    pub paths: Paths,
    pub page: Page,
    pub url: TextEdit,
    pub search: TextEdit,
    pub catalog: crate::catalog::Catalog,
    pub loaded: bool,
    pub settings: Settings,
    pub installed: Vec<Installed>,
    pub removing: BTreeSet<String>,
    pub jobs: Vec<Job>,
    pub next_job: u64,
    pub queue_paused: bool,
    pub history: Vec<Entry>,
    pub library_sort: LibrarySort,
    pub library_service: Option<String>,
    pub library_files: BTreeMap<String, library::SidecarInfo>,
    pub lookup_picker: Option<(u64, String)>,
    pub extension_settings: Option<String>,
    pub extension_filter: Option<Kind>,
    pub(super) saving_settings: bool,
    pub(super) pending_settings_save: bool,
    pub(super) saving_history: bool,
    pub(super) pending_history_save: bool,
    /// Decoded covers by cached file path.
    pub images: BTreeMap<String, Image>,
    pub image_cache: Option<ImageCache>,
    pub tools: Option<Tools>,
    pub available_update: Option<updates::Release>,
    pub extension_status: BTreeMap<String, ExtensionStatus>,
    /// Extensions whose status check is running.
    pub checking: BTreeMap<String, u64>,
    pub(super) status_queue: VecDeque<String>,
    pub(super) status_serial: u64,
    pub(super) cover_loading: BTreeSet<String>,
    /// Flacora's own icon, shown in the sidebar.
    pub logo: Option<Image>,
    pub project_logos: Option<[Image; 2]>,
    pub provider_logos: BTreeMap<&'static str, Image>,
    pub notice: Option<Notification>,
    pub info: Option<Notification>,
    pub undo: Option<UndoAction>,
    pub confirm_action: Option<ConfirmAction>,
    /// Package files being installed.
    pub importing: usize,
    pub(super) window: Option<rune::WindowHandle>,
    pub(super) browsing_packages: bool,
    pub(super) pending_imports: VecDeque<PathBuf>,
    pub(super) pending_checks: BTreeSet<String>,
    /// Files are being dragged over the window.
    pub drop_over: bool,
    pub link: Option<Link>,
}

impl Default for App {
    fn default() -> Self {
        App {
            paths: Paths::detect(),
            page: Page::Downloads,
            url: TextEdit::default(),
            search: TextEdit::default(),
            catalog: crate::catalog::Catalog::default(),
            loaded: false,
            settings: Settings::default(),
            installed: Vec::new(),
            removing: BTreeSet::new(),
            jobs: Vec::new(),
            next_job: 1,
            queue_paused: false,
            history: Vec::new(),
            library_sort: LibrarySort::Newest,
            library_service: None,
            library_files: BTreeMap::new(),
            lookup_picker: None,
            extension_settings: None,
            extension_filter: None,
            saving_settings: false,
            pending_settings_save: false,
            saving_history: false,
            pending_history_save: false,
            images: BTreeMap::new(),
            image_cache: None,
            tools: None,
            available_update: None,
            extension_status: BTreeMap::new(),
            checking: BTreeMap::new(),
            status_queue: VecDeque::new(),
            status_serial: 0,
            cover_loading: BTreeSet::new(),
            logo: None,
            project_logos: None,
            provider_logos: BTreeMap::new(),
            notice: None,
            info: None,
            undo: None,
            confirm_action: None,
            importing: 0,
            window: None,
            browsing_packages: false,
            pending_imports: VecDeque::new(),
            pending_checks: BTreeSet::new(),
            drop_over: false,
            link: None,
        }
    }
}

impl App {
    pub fn success(&mut self, message: impl Into<String>) {
        self.discard_undo();
        self.notify(NotificationKind::Success, message.into());
    }

    pub fn success_undo(&mut self, message: impl Into<String>, undo: UndoAction) {
        self.discard_undo();
        self.undo = Some(undo);
        self.notify(NotificationKind::Success, message.into());
    }

    pub fn dismiss_info(&mut self) {
        self.info = None;
        self.discard_undo();
    }

    pub fn discard_undo(&mut self) {
        if let Some(UndoAction::History(_)) = self.undo.take() {
            let keep: Vec<String> = self.history.iter().filter_map(|e| e.cover.clone()).collect();
            self.images.retain(|path, _| keep.contains(path));
            let paths = self.paths.clone();
            self.background_opt(move || {
                library::prune_unused_covers(&paths, &keep);
                None
            });
        }
    }

    pub fn undo_last(&mut self) {
        self.info = None;
        match self.undo.take() {
            Some(UndoAction::History(entries)) => {
                self.history = entries;
                self.save_history();
                self.load_history_covers();
            }
            Some(UndoAction::FinishedJobs(jobs)) => {
                self.jobs.extend(jobs);
                self.jobs.sort_by_key(|j| j.id);
            }
            None => {}
        }
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.notify(NotificationKind::Warning, message.into());
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.notify(NotificationKind::Error, message.into());
    }

    fn notify(&mut self, kind: NotificationKind, message: String) {
        let toast = Some(Notification { kind, message, expires_at: None });
        match kind {
            NotificationKind::Success => self.info = toast,
            NotificationKind::Warning | NotificationKind::Error => self.notice = toast,
        }
        // An input handler can create a notification after the frame's poll.
        if let Some(link) = &self.link {
            (link.waker)();
        }
    }

    pub fn tick_notifications(&mut self, clock: f64, tx: &mut Tx) -> bool {
        let mut expired = false;
        let info_lifetime = if self.undo.is_some() { 6.0 } else { 2.5 };
        let mut info_expired = false;
        if let Some(toast) = &mut self.info {
            if toast.expires_at.is_none() {
                tx.animated();
            }
            let deadline = *toast.expires_at.get_or_insert(clock + info_lifetime);
            if clock >= deadline {
                self.info = None;
                expired = true;
                info_expired = true;
                tx.animated();
            } else {
                tx.wake_at(deadline);
            }
        }
        if info_expired {
            self.discard_undo();
        }
        if let Some(toast) = &mut self.notice {
            if toast.expires_at.is_none() {
                tx.animated();
            }
            let deadline = *toast.expires_at.get_or_insert(clock + 4.0);
            if clock >= deadline {
                self.notice = None;
                expired = true;
                tx.animated();
            } else {
                tx.wake_at(deadline);
            }
        }
        expired
    }

    /// Sets up image decoding for covers and embedded artwork.
    pub fn images(&mut self, waker: Waker) {
        let cache = ImageCache::new(waker);
        self.logo = Some(cache.load(crate::LOGO));
        self.project_logos = Some(crate::ui::project_logos(&cache));
        self.provider_logos = services::LOGOS.into_iter().map(|(service, bytes)| (service, cache.load(bytes))).collect();
        self.image_cache = Some(cache);
    }

    /// Connects background work to the window and starts loading.
    pub fn connect(&mut self, waker: Waker) {
        if self.image_cache.is_none() {
            self.images(waker.clone());
        }
        self.link = Some(Link::new(waker));
        let paths = self.paths.clone();
        self.background(move || Msg::Loaded {
            installed: store::load_installed(&paths),
            settings: store::load_settings(&paths),
            history: library::load(&paths),
        });
        self.refresh_tools();
        self.background(|| Msg::UpdateChecked(updates::check()));
    }

    pub fn background(&self, work: impl FnOnce() -> Msg + Send + 'static) {
        if let Some(link) = &self.link {
            let send = link.sender();
            thread::spawn(move || send(work()));
        }
    }

    pub fn background_opt(&self, work: impl FnOnce() -> Option<Msg> + Send + 'static) {
        if let Some(link) = &self.link {
            let send = link.sender();
            thread::spawn(move || {
                if let Some(msg) = work() {
                    send(msg);
                }
            });
        }
    }

    /// Applies every message that arrived; true when something changed.
    pub fn drain(&mut self) -> bool {
        let mut messages = Vec::new();
        if let Some(link) = &self.link {
            while let Ok(msg) = link.rx.try_recv() {
                messages.push(msg);
            }
        }
        let changed = !messages.is_empty();
        for msg in messages {
            self.apply(msg);
        }
        changed
    }

    pub fn apply(&mut self, msg: Msg) {
        match msg {
            Msg::Loaded { installed, settings, history } => {
                self.installed = installed;
                self.settings = settings;
                self.history = history;
                self.loaded = true;
                self.load_history_covers();
                self.refresh_library_files();
                if self.importing == 0 {
                    self.check_all();
                } else {
                    self.pending_checks.extend(
                        self.installed.iter().map(|i| i.manifest.id.clone()),
                    );
                    self.start_import();
                }
                if self.page == Page::Catalog {
                    self.enter_catalog();
                }
            }
            Msg::PackagesPicked(mut files) => {
                self.browsing_packages = false;
                let count = files.len();
                files.retain(|file| file.extension().is_some_and(|ext| ext.eq_ignore_ascii_case(store::PACKAGE)));
                if files.len() != count {
                    self.warn(format!("Select only .{} extension packages.", store::PACKAGE));
                }
                if !files.is_empty() {
                    self.install_files(&files);
                }
            }
            Msg::Imported(file, result) => {
                self.importing = self.importing.saturating_sub(1);
                let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                match result {
                    Ok(installed) => {
                        self.success(format!("Installed {} {} from {name}", installed.manifest.name, installed.manifest.version));
                        self.add_installed(*installed);
                    }
                    Err(e) => self.error(format!("{name}: {e}")),
                }
                if self.importing == 0 {
                    for id in std::mem::take(&mut self.pending_checks) {
                        self.check_status(&id);
                    }
                } else {
                    self.start_import();
                }
            }
            Msg::Removed(id, result) => {
                self.removing.remove(&id);
                match result {
                    Ok(()) => {
                        let name = self
                            .installed
                            .iter()
                            .find(|i| i.manifest.id == id)
                            .map(|i| i.manifest.name.clone())
                            .unwrap_or_else(|| id.clone());
                        self.installed.retain(|i| i.manifest.id != id);
                        self.forget_status(&id);
                        if self.extension_settings.as_deref() == Some(id.as_str()) {
                            self.extension_settings = None;
                        }
                        if let Some(kind) = self.extension_filter
                            && !self.installed.iter().any(|i| i.manifest.kind == kind)
                        {
                            self.extension_filter = None;
                        }
                        self.success(format!("Removed {name}"));
                    }
                    Err(e) => self.error(e),
                }
            }
            Msg::Tools(tools) => self.tools = Some(tools),
            Msg::UpdateChecked(release) => self.available_update = release,
            Msg::ExtensionStatus { id, check, status } => {
                if self.checking.get(&id) == Some(&check) {
                    self.checking.remove(&id);
                    self.extension_status.insert(id, status);
                    self.pump_status_checks();
                }
            }
            Msg::SettingsSaved(result) => {
                self.saving_settings = false;
                if let Err(error) = result {
                    self.error(format!("Could not save settings: {error}"));
                }
                if std::mem::take(&mut self.pending_settings_save) {
                    self.save_settings();
                }
            }
            Msg::HistorySaved(result) => {
                self.saving_history = false;
                if let Err(error) = result {
                    self.error(format!("Could not save the history: {error}"));
                }
                if std::mem::take(&mut self.pending_history_save) {
                    self.save_history();
                }
            }
            Msg::Job(id, event) => self.job_event(id, event),
            Msg::JobStaging(id, staging) => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id && j.is_running()) {
                    job.staging = Some(staging);
                }
            }
            Msg::Catalog(id, event) => self.catalog.event(id, event),
            Msg::CatalogArtwork(url, bytes) => self.catalog_artwork_loaded(url, bytes),
            Msg::DownloadPlaced(id, result) => self.download_placed(id, result),
            Msg::JobCover(id, path, bytes) => self.job_cover(id, path, bytes),
            Msg::Image(path, bytes) => {
                self.cover_loading.remove(&path);
                self.load_image(path, bytes);
            }
            Msg::LibraryFiles(files) => self.library_files = files,
            Msg::Error(e) if e.is_empty() => {}
            Msg::Error(e) => self.error(e),
        }
    }

    fn add_installed(&mut self, installed: Installed) {
        let id = installed.manifest.id.clone();
        let alias = extensions::legacy_web_id(&id);
        self.installed.retain(|i| i.manifest.id != id && alias != Some(i.manifest.id.as_str()));
        if alias.is_some() && self.catalog.provider.as_deref() == alias {
            self.catalog.select_provider(id.clone());
        }
        self.installed.push(installed);
        self.installed.sort_by_key(|i| i.manifest.name.to_lowercase());
        if let Some(alias) = alias {
            self.forget_status(alias);
        }
        self.forget_status(&id);
        self.pending_checks.insert(id);
    }

    fn forget_status(&mut self, id: &str) {
        self.checking.remove(id);
        self.extension_status.remove(id);
        self.pending_checks.remove(id);
        self.status_queue.retain(|queued| queued != id);
    }

    /// Refreshes the online status of every installed extension.
    pub fn check_all(&mut self) {
        if self.importing > 0 {
            return;
        }
        let ids: Vec<_> = self.installed.iter().map(|i| i.manifest.id.clone()).collect();
        for id in ids {
            self.check_status(&id);
        }
    }

    /// Refreshes the online status of one installed extension.
    pub fn check_extension(&mut self, id: &str) {
        if self.importing > 0 {
            return;
        }
        self.check_status(id);
    }

    fn check_status(&mut self, id: &str) {
        if self.link.is_none()
            || self.checking.contains_key(id)
            || self.removing.contains(id)
            || self.status_queue.iter().any(|queued| queued == id)
        {
            return;
        }
        if !self.installed.iter().any(|i| i.manifest.id == id) {
            return;
        }
        self.status_queue.push_back(id.to_owned());
        self.pump_status_checks();
    }

    pub fn pump_status_checks(&mut self) {
        const MAX_CONCURRENT_CHECKS: usize = 4;
        if self.link.is_none() || self.importing > 0 {
            return;
        }
        let mut browser_busy = self.jobs.iter().any(|j| j.is_running() && j.extension.manifest.browser)
            || self.installed.iter().any(|i| {
                i.manifest.browser && !i.manifest.status.is_empty() && self.checking.contains_key(&i.manifest.id)
            });
        let mut scanned = 0;
        let total = self.status_queue.len();
        while self.checking.len() < MAX_CONCURRENT_CHECKS && scanned < total {
            let Some(id) = self.status_queue.pop_front() else { break };
            scanned += 1;
            if self.checking.contains_key(&id) || self.removing.contains(&id) {
                continue;
            }
            let Some(installed) = self.installed.iter().find(|i| i.manifest.id == id).cloned() else {
                continue;
            };
            let needs_browser = installed.manifest.browser && !installed.manifest.status.is_empty();
            if needs_browser && browser_busy {
                self.status_queue.push_back(id);
                continue;
            }
            if needs_browser {
                browser_busy = true;
            }
            self.status_serial += 1;
            let check = self.status_serial;
            self.checking.insert(id.clone(), check);
            let paths = self.paths.clone();
            self.background(move || Msg::ExtensionStatus {
                id,
                check,
                status: store::check_status(&paths, &installed),
            });
        }
    }

    pub fn is_checking(&self, id: &str) -> bool {
        self.checking.contains_key(id) || self.status_queue.iter().any(|queued| queued == id)
    }

    pub fn any_checking(&self) -> bool {
        !self.checking.is_empty() || !self.status_queue.is_empty()
    }

    pub fn status_command_running(&self, installed: &Installed) -> bool {
        !installed.manifest.status.is_empty() && self.is_checking(&installed.manifest.id)
    }

    /// The services to show for an extension: what works now when it checked, else what it declares.
    pub fn services_of<'a>(&'a self, id: &str, declared: &'a [String]) -> &'a [String] {
        match self.extension_status.get(id) {
            Some(ExtensionStatus::Services(services)) => services,
            _ => declared,
        }
    }

    pub fn load_image(&mut self, path: String, bytes: Vec<u8>) {
        if let Some(cache) = &self.image_cache {
            self.images.entry(path).or_insert_with(|| cache.load(bytes));
        }
    }

    pub fn image(&self, path: Option<&str>) -> Option<&Image> {
        self.images.get(path?)
    }

    fn load_history_covers(&mut self) {
        const INITIAL_COVERS: usize = 64;
        let mut seen = BTreeSet::new();
        let covers: Vec<String> = self
            .history
            .iter()
            .filter_map(|e| e.cover.as_ref())
            .filter(|path| !self.images.contains_key(*path) && seen.insert((*path).clone()))
            .take(INITIAL_COVERS)
            .cloned()
            .collect();
        if covers.is_empty() {
            return;
        }
        for path in &covers {
            self.cover_loading.insert(path.clone());
        }
        if let Some(link) = &self.link {
            let send = link.sender();
            let paths = self.paths.clone();
            thread::spawn(move || {
                for path in covers {
                    if let Some(bytes) = library::read_cover(&paths, &path) {
                        send(Msg::Image(path, bytes));
                    }
                }
            });
        }
    }

    pub fn request_history_cover(&mut self, path: &str) {
        if self.link.is_none() || self.images.contains_key(path) || !self.cover_loading.insert(path.to_owned()) {
            return;
        }
        let paths = self.paths.clone();
        let path = path.to_owned();
        self.background_opt(move || library::read_cover(&paths, &path).map(|bytes| Msg::Image(path, bytes)));
    }

    pub fn dark(&self, system_dark: bool) -> bool {
        match self.settings.theme {
            Theme::System => system_dark,
            Theme::Light => false,
            Theme::Dark => true,
        }
    }

    // ------------------------------------------------------------ routing

    /// Host of the first link being typed.
    pub fn host(&self) -> Option<String> {
        self.url.text.split_whitespace().find_map(extensions::url_host)
    }

    pub fn service(&self) -> Option<&'static Service> {
        services::detect(&self.host()?)
    }

    /// Installed downloaders that accept links on `host`, best first, including those
    /// currently reporting a temporary outage (used in Settings).
    pub fn installed_downloaders_for(&self, host: &str) -> Vec<&Installed> {
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
        let mut found: Vec<&Installed> =
            self.installed.iter().filter(|i| i.manifest.kind == Kind::Resolver && extensions::host_matches(&i.manifest.hosts, host)).collect();
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

    pub fn set_theme(&mut self, theme: Theme) {
        self.settings.theme = theme;
        self.save_settings();
    }

    fn save_settings(&mut self) {
        if self.link.is_none() {
            return;
        }
        if self.saving_settings {
            self.pending_settings_save = true;
            return;
        }
        self.saving_settings = true;
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
        if self.link.is_none() || self.importing > 0
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
    fn start_import(&mut self) {
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
        if self.saving_history {
            self.pending_history_save = true;
            return;
        }
        self.saving_history = true;
        let (paths, history) = (self.paths.clone(), self.history.clone());
        self.background(move || Msg::HistorySaved(library::save(&paths, &history)));
    }

    /// History entries matching the service filter and search box, sorted by `library_sort`.
    pub fn filtered_history(&self) -> Vec<(usize, &Entry)> {
        let query = self.search.text.trim();
        let service_filter = self.library_service.as_deref();
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

fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    let n = needle.as_bytes();
    if n.is_empty() {
        return true;
    }
    haystack.as_bytes().windows(n.len()).any(|w| w.eq_ignore_ascii_case(n))
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
