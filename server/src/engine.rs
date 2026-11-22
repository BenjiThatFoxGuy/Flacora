//! The headless engine: Flacora's `App` driven on its own thread.
//!
//! `App` is the GUI's state object, but its download engine only needs a `Tx`
//! and a `Waker`, both of which are constructible without a window. The HTTP
//! layer sends [`Command`]s here and receives a [`State`] snapshot after every
//! change.

use flacora::app::{App, NotificationKind};
use flacora::{Tx, Waker};
use flacora::extensions;
use flacora::jobs::{Choice as JobChoice, Job, Status};
use flacora::library::Entry;
use flacora::output::Layout;
use flacora::services;
use flacora::store::{ExtensionStatus, Installed, Settings};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A request from the HTTP layer to the engine.
#[derive(Debug)]
pub enum Command {
    Enqueue {
        urls: Vec<String>,
        service: Option<String>,
        options: Vec<(String, String, usize)>,
    },
    CancelJob(u64),
    RetryJob(u64),
    RemoveJob(u64),
    CatalogMore,
    ClearFinished,
    CancelAll,
    RetryFailed,
    ToggleLog(u64),
    SetQueuePaused(bool),
    Choose {
        job: u64,
        choice: usize,
    },
    Install(PathBuf),
    RemoveExtension(String),
    RemoveAllExtensions,
    CheckAll,
    CheckExtension(String),
    RemoveEntry(usize),
    ClearHistory,
    Lookup {
        index: usize,
        extension: String,
    },
    SetOutput(PathBuf),
    SetLayout(Layout),
    SetAutoTag(bool),
    SetPreferred {
        service: String,
        extension: Option<String>,
    },
    SetPick {
        extension: String,
        option: String,
        index: usize,
    },
}

// ---------------------------------------------------------------- state DTOs

#[derive(Clone, Debug, Serialize)]
pub struct State {
    pub loaded: bool,
    pub importing: usize,
    pub queue_paused: bool,
    pub jobs: Vec<JobDto>,
    pub installed: Vec<ExtensionDto>,
    pub settings: SettingsDto,
    pub history: Vec<EntryDto>,
    pub library: BTreeMap<String, LibraryDto>,
    pub tools: ToolsDto,
    pub update: Option<UpdateDto>,
    pub notices: Vec<NoticeDto>,
    pub catalog: Option<serde_json::Value>,
    pub services: Vec<ServiceDto>,
}

impl Default for State {
    fn default() -> Self {
        State {
            loaded: false,
            importing: 0,
            queue_paused: false,
            jobs: Vec::new(),
            installed: Vec::new(),
            settings: SettingsDto::default(),
            history: Vec::new(),
            library: BTreeMap::new(),
            tools: ToolsDto::default(),
            update: None,
            notices: Vec::new(),
            catalog: None,
            services: services::SERVICES
                .iter()
                .map(|service| ServiceDto { key: service.key.to_string(), name: service.name.to_string() })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct JobDto {
    pub id: u64,
    pub url: String,
    pub service: Option<String>,
    pub service_key: Option<String>,
    pub extension: ExtensionDto,
    pub status: String,
    pub error: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub quality: Option<String>,
    pub stage: Option<String>,
    pub progress: Option<f32>,
    pub cover: Option<String>,
    pub log: Vec<String>,
    pub saved: Vec<String>,
    pub show_log: bool,
    pub cancelling: bool,
    pub finished: Option<u64>,
    pub links: Vec<(String, String)>,
    pub choices: Vec<ChoiceDto>,
    pub resolved: Option<(String, Option<String>)>,
    pub lookup_file: Option<String>,
    pub output: String,
    pub auto_tag: bool,
    pub picks: Vec<usize>,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct ChoiceDto {
    pub service: String,
    pub service_key: String,
    pub url: String,
    pub downloader: String,
    pub downloader_name: String,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct ExtensionDto {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: String,
    pub description: String,
    pub services: Vec<String>,
    pub hosts: Vec<String>,
    pub options: Vec<OptionDto>,
    pub status: String,
    pub status_detail: Option<String>,
    pub checking: bool,
    pub browser: bool,
    pub writes_tags: bool,
    pub requires: Vec<String>,
    pub dir: String,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct OptionDto {
    pub key: String,
    pub label: String,
    pub choices: Vec<ChoiceValueDto>,
    pub default: usize,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct ChoiceValueDto {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct SettingsDto {
    pub output: String,
    pub filename: String,
    pub folders: String,
    pub auto_tag: bool,
    pub preferred: BTreeMap<String, String>,
    pub options: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct EntryDto {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub service: String,
    pub extension: String,
    pub quality: String,
    pub url: String,
    pub files: Vec<String>,
    pub cover: Option<String>,
    pub date: u64,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct LibraryDto {
    pub exists: bool,
    pub sidecars: Vec<(String, String)>,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct ToolsDto {
    pub ffmpeg: Option<String>,
    pub browser: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpdateDto {
    pub version: String,
    pub url: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NoticeDto {
    pub kind: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ServiceDto {
    pub key: String,
    pub name: String,
}

// ---------------------------------------------------------------------- engine

pub fn run(cmd_rx: Receiver<Command>, state_tx: Sender<State>) {
    let wake = Arc::new(AtomicBool::new(false));
    let waker: Waker = Arc::new(move || {
        wake.store(true, Ordering::SeqCst);
    });

    let mut app = App::default();
    app.connect(waker);
    let output_env = std::env::var_os("FLACORA_OUTPUT").filter(|value| !value.is_empty());
    let mut applied_output = false;
    let mut tx = Tx::default();
    let mut clock = 0.0f64;
    let mut last = Instant::now();

    loop {
        let changed = app.drain_with(clock, &mut tx);
        if !applied_output && app.loaded {
            applied_output = true;
            if let Some(output) = &output_env
                && app.settings.output == flacora::store::default_output()
            {
                app.settings.output = PathBuf::from(output);
            }
        }
        let mut commanded = false;
        while let Ok(command) = cmd_rx.try_recv() {
            commanded |= handle(&mut app, command);
        }
        if changed || commanded {
            let _ = state_tx.send(snapshot(&app));
        }
        clock += last.elapsed().as_secs_f64().max(0.016);
        last = Instant::now();
        std::thread::sleep(Duration::from_millis(60));
    }
}

fn handle(app: &mut App, command: Command) -> bool {
    match command {
        Command::Enqueue { urls, service, options } => {
            app.url.text = urls.join("\n");
            for (extension, option, index) in options {
                app.set_pick(&extension, &option, index);
            }
            if let Some(name) = service {
                let found = services::by_name(&name).or_else(|| {
                    services::SERVICES.iter().find(|service| service.key == name)
                });
                app.set_batch_service(found);
            } else {
                app.set_batch_service(None);
            }
            app.enqueue();
            true
        }
        Command::CancelJob(id) => {
            app.cancel_job(id);
            true
        }
        Command::RetryJob(id) => {
            app.retry_job(id);
            true
        }
        Command::RemoveJob(id) => {
            app.remove_job(id);
            true
        }
        Command::CatalogMore => {
            app.catalog_more();
            true
        }
        Command::ClearFinished => {
            app.clear_finished();
            true
        }
        Command::CancelAll => {
            app.cancel_all_jobs();
            true
        }
        Command::RetryFailed => {
            app.retry_failed_jobs();
            true
        }
        Command::ToggleLog(id) => {
            app.toggle_log(id);
            true
        }
        Command::SetQueuePaused(paused) => {
            if app.queue_paused != paused {
                app.toggle_queue_pause();
            }
            true
        }
        Command::Choose { job, choice } => {
            app.choose(job, choice);
            true
        }
        Command::Install(path) => {
            app.install_files(&[path]);
            true
        }
        Command::RemoveExtension(id) => {
            app.remove(&id);
            true
        }
        Command::RemoveAllExtensions => {
            app.remove_all_extensions();
            true
        }
        Command::CheckAll => {
            app.check_all();
            true
        }
        Command::CheckExtension(id) => {
            app.check_extension(&id);
            true
        }
        Command::RemoveEntry(index) => {
            app.remove_entry(index);
            true
        }
        Command::ClearHistory => {
            app.clear_history();
            true
        }
        Command::Lookup { index, extension } => {
            app.enqueue_lookup(index, &extension);
            true
        }
        Command::SetOutput(path) => {
            app.set_output(path);
            true
        }
        Command::SetLayout(layout) => {
            app.set_output_layout(layout);
            true
        }
        Command::SetAutoTag(value) => {
            if app.settings.auto_tag != value {
                app.toggle_auto_tag();
            }
            true
        }
        Command::SetPreferred { service, extension } => {
            app.set_preferred(&service, extension);
            true
        }
        Command::SetPick { extension, option, index } => {
            app.set_pick(&extension, &option, index);
            true
        }
    }
}

// ------------------------------------------------------------------ snapshots

fn snapshot(app: &App) -> State {
    State {
        loaded: app.loaded,
        importing: app.importing,
        queue_paused: app.queue_paused,
        jobs: app.jobs.iter().map(job_dto).collect(),
        installed: app.installed.iter().map(|installed| extension_dto(app, installed)).collect(),
        settings: settings_dto(&app.settings),
        history: app.history.iter().map(entry_dto).collect(),
        library: app
            .library_files
            .iter()
            .map(|(path, info)| {
                (
                    path.clone(),
                    LibraryDto { exists: info.exists, sidecars: info.sidecars.clone() },
                )
            })
            .collect(),
        tools: ToolsDto {
            ffmpeg: app.tools.as_ref().and_then(|tools| tools.ffmpeg.clone()),
            browser: app.tools.as_ref().and_then(|tools| tools.browser.clone()),
        },
        update: app.available_update.as_ref().map(|release| UpdateDto {
            version: release.version.to_string(),
            url: release.url.to_string(),
        }),
        notices: notices_dto(app),
        catalog: (!app.catalog.input.text.is_empty()).then(|| serde_json::json!({
            "url": app.catalog.input.text,
            "busy": app.catalog.busy(),
            "error": app.catalog.error,
            "result": app.catalog.result.as_ref().map(|result| serde_json::json!({
                "title": result.title,
                "note": result.note,
                "more": result.next.is_some(),
                "items": result.items.iter().map(|item| serde_json::json!({
                    "title": item.title, "detail": item.detail, "url": item.url,
                    "track": item.kind == flacora::catalog::Kind::Track,
                })).collect::<Vec<_>>(),
            })),
        })),
        services: services::SERVICES
            .iter()
            .map(|service| ServiceDto { key: service.key.to_string(), name: service.name.to_string() })
            .collect(),
    }
}

fn job_dto(job: &Job) -> JobDto {
    let (status, error) = status_dto(&job.status);
    JobDto {
        id: job.id,
        url: job.url.clone(),
        service: job.service.map(|service| service.name.to_string()),
        service_key: job.service.map(|service| service.key.to_string()),
        extension: extension_dto_manifest(&job.extension),
        status,
        error,
        title: job.title.clone(),
        artist: job.artist.clone(),
        album: job.album.clone(),
        quality: job.quality.clone(),
        stage: job.stage.clone(),
        progress: job.progress,
        cover: job.cover.clone(),
        log: job.log.clone(),
        saved: job.saved.clone(),
        show_log: job.show_log,
        cancelling: job.cancelling,
        finished: job.finished,
        links: job.links.clone(),
        choices: job.choices.iter().map(choice_dto).collect(),
        resolved: job
            .resolved
            .as_ref()
            .map(|(name, service)| (name.clone(), service.map(|service| service.name.to_string()))),
        lookup_file: job.lookup_file.as_ref().map(|path| path.to_string_lossy().into_owned()),
        output: job.output.to_string_lossy().into_owned(),
        auto_tag: job.auto_tag,
        picks: job.picks.clone(),
    }
}

fn choice_dto(choice: &JobChoice) -> ChoiceDto {
    ChoiceDto {
        service: choice.service.name.to_string(),
        service_key: choice.service.key.to_string(),
        url: choice.url.clone(),
        downloader: choice.downloader.clone(),
        downloader_name: choice.downloader_name.clone(),
    }
}

fn status_dto(status: &Status) -> (String, Option<String>) {
    match status {
        Status::Queued => ("queued".to_string(), None),
        Status::Running => ("running".to_string(), None),
        Status::Choosing => ("choosing".to_string(), None),
        Status::Done => ("done".to_string(), None),
        Status::Failed(error) => ("failed".to_string(), Some(error.clone())),
        Status::Cancelled => ("cancelled".to_string(), None),
    }
}

fn extension_dto(app: &App, installed: &Installed) -> ExtensionDto {
    let id = &installed.manifest.id;
    let (status, status_detail) = extension_status_dto(app.extension_status.get(id));
    ExtensionDto {
        id: id.clone(),
        name: installed.manifest.name.clone(),
        version: installed.manifest.version.clone(),
        kind: installed.manifest.kind.label().to_string(),
        description: installed.manifest.description.clone(),
        services: app.services_of(id, &installed.manifest.services).to_vec(),
        hosts: installed.manifest.hosts.clone(),
        options: installed.manifest.options.iter().map(option_dto).collect(),
        status,
        status_detail,
        checking: app.is_checking(id),
        browser: installed.manifest.browser,
        writes_tags: installed.manifest.writes_tags,
        requires: installed.manifest.requires.clone(),
        dir: installed.dir.to_string_lossy().into_owned(),
    }
}

fn extension_status_dto(status: Option<&ExtensionStatus>) -> (String, Option<String>) {
    match status {
        Some(ExtensionStatus::Online) => ("online".to_string(), None),
        Some(ExtensionStatus::Degraded) => ("degraded".to_string(), None),
        Some(ExtensionStatus::Offline(reason)) => ("offline".to_string(), Some(reason.clone())),
        Some(ExtensionStatus::Unknown(reason)) => ("unknown".to_string(), Some(reason.clone())),
        Some(ExtensionStatus::Services(_)) => ("online".to_string(), None),
        None => ("unknown".to_string(), None),
    }
}

fn extension_dto_manifest(installed: &Installed) -> ExtensionDto {
    ExtensionDto {
        id: installed.manifest.id.clone(),
        name: installed.manifest.name.clone(),
        version: installed.manifest.version.clone(),
        kind: installed.manifest.kind.label().to_string(),
        description: installed.manifest.description.clone(),
        services: installed.manifest.services.clone(),
        hosts: installed.manifest.hosts.clone(),
        options: installed.manifest.options.iter().map(option_dto).collect(),
        status: String::new(),
        status_detail: None,
        checking: false,
        browser: installed.manifest.browser,
        writes_tags: installed.manifest.writes_tags,
        requires: installed.manifest.requires.clone(),
        dir: installed.dir.to_string_lossy().into_owned(),
    }
}

fn option_dto(option: &extensions::ExtOption) -> OptionDto {
    OptionDto {
        key: option.key.clone(),
        label: option.label.clone(),
        choices: option
            .choices
            .iter()
            .map(|choice| ChoiceValueDto { value: choice.value.clone(), label: choice.label.clone() })
            .collect(),
        default: option.default_index(),
    }
}

fn settings_dto(settings: &Settings) -> SettingsDto {
    SettingsDto {
        output: settings.output.to_string_lossy().into_owned(),
        filename: filename_key(settings.layout.filename).to_string(),
        folders: folders_key(settings.layout.folders).to_string(),
        auto_tag: settings.auto_tag,
        preferred: settings.preferred.clone(),
        options: settings.options.clone(),
    }
}

fn filename_key(filename: flacora::output::Filename) -> &'static str {
    match filename {
        flacora::output::Filename::ExtensionDefault => "extension-default",
        flacora::output::Filename::ArtistTitle => "artist-title",
        flacora::output::Filename::TitleArtist => "title-artist",
        flacora::output::Filename::Title => "title",
    }
}

fn folders_key(folders: flacora::output::Folders) -> &'static str {
    match folders {
        flacora::output::Folders::Single => "single",
        flacora::output::Folders::Artist => "artist",
        flacora::output::Folders::ArtistAlbum => "artist-album",
    }
}

fn entry_dto(entry: &Entry) -> EntryDto {
    EntryDto {
        title: entry.title.clone(),
        artist: entry.artist.clone(),
        album: entry.album.clone(),
        service: entry.service.clone(),
        extension: entry.extension.clone(),
        quality: entry.quality.clone(),
        url: entry.url.clone(),
        files: entry.files.clone(),
        cover: entry.cover.clone(),
        date: entry.date,
    }
}

fn notices_dto(app: &App) -> Vec<NoticeDto> {
    let mut notices = Vec::new();
    for notification in [&app.notice, &app.info].into_iter().flatten() {
        notices.push(NoticeDto {
            kind: notice_kind(notification.kind).to_string(),
            message: notification.message.clone(),
        });
    }
    notices
}

fn notice_kind(kind: NotificationKind) -> &'static str {
    match kind {
        NotificationKind::Success => "success",
        NotificationKind::Warning => "warning",
        NotificationKind::Error => "error",
    }
}
