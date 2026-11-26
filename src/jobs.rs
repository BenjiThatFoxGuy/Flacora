//! The download queue. Jobs run one at a time, in the order they were added,
//! so extensions never fight over the shared browser.

use crate::{
    app::{App, Msg, Page},
    extensions::{self, Fill, Kind},
    library::{self, Entry},
    output::{Layout, TrackNames},
    protocol::{self, Event},
    runner::{self, JobEvent, JobHandle},
    services::{self, Service},
    store::{Installed, Settings},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Log lines kept per job.
const LOG_LIMIT: usize = 400;

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Queued,
    Running,
    /// A resolver found the track; waiting for the user to pick one of `Job::choices`.
    Choosing,
    Done,
    Failed(String),
    Cancelled,
}

/// A service a resolver found the track on, with the downloader that would take it.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub service: &'static Service,
    pub url: String,
    pub downloader: String,
    pub downloader_name: String,
}

pub struct Job {
    pub id: u64,
    pub url: String,
    /// A downloaded FLAC passed to a lyrics or metadata extension.
    pub lookup_file: Option<PathBuf>,
    pub service: Option<&'static Service>,
    pub extension: Installed,
    /// Option choice per option of `extension`, fixed when the job was added.
    pub picks: Vec<usize>,
    pub output: PathBuf,
    pub layout: Layout,
    pub auto_tag: bool,
    pub staging: Option<tempfile::TempDir>,
    pub status: Status,
    /// When this job first started running, for the watchdog in `timeout_stalled_jobs`.
    pub started: Option<Instant>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub quality: Option<String>,
    pub stage: Option<String>,
    /// 0..=1 while the extension reports progress.
    pub progress: Option<f32>,
    pub cover: Option<String>,
    pub log: Vec<String>,
    pub saved: Vec<String>,
    pub show_log: bool,
    pub handle: Option<JobHandle>,
    pub cancelling: bool,
    /// Unix seconds when it finished, matching its history entry.
    pub finished: Option<u64>,
    /// A resolver's result: the track on other services.
    pub links: Vec<(String, String)>,
    /// The services from `links` an installed downloader takes, best first.
    pub choices: Vec<Choice>,
    /// The destination explicitly chosen for this resolver's batch.
    pub batch_service: Option<&'static Service>,
    /// Set once a resolver handed the job to a downloader: the resolver's name and the
    /// service of the link that was pasted.
    pub resolved: Option<(String, Option<&'static Service>)>,
}

impl Drop for Job {
    fn drop(&mut self) {
        // Kill the running process tree before `staging` (TempDir) is dropped.
        if let Some(handle) = self.handle.take() {
            handle.cancel();
        }
    }
}

impl Job {
    fn queued(id: u64, url: String, service: Option<&'static Service>, extension: Installed, picks: Vec<usize>, settings: &Settings) -> Self {
        Self {
            id,
            url,
            service,
            extension,
            picks,
            output: settings.output.clone(),
            layout: settings.layout,
            auto_tag: settings.auto_tag,
            staging: None,
            lookup_file: None,
            status: Status::Queued,
            started: None,
            title: None,
            artist: None,
            album: None,
            quality: None,
            stage: None,
            progress: None,
            cover: None,
            log: Vec::new(),
            saved: Vec::new(),
            show_log: false,
            handle: None,
            cancelling: false,
            finished: None,
            links: Vec::new(),
            choices: Vec::new(),
            batch_service: None,
            resolved: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.status == Status::Running
    }

    pub fn is_finished(&self) -> bool {
        matches!(self.status, Status::Done | Status::Failed(_) | Status::Cancelled)
    }

    /// The main saved audio file.
    pub fn main_file(&self) -> Option<&str> {
        self.saved.iter().find(|f| library::is_audio_file(f)).or(self.saved.first()).map(String::as_str)
    }
}

/// Validates saved file paths reported by an extension. When `enforce_containment` is true
/// (live runs), each file must exist as a regular file inside `base_dir`.
pub fn validate_saved_files(
    base_dir: Option<&std::path::Path>,
    files: &[String],
    enforce_containment: bool,
) -> Result<Vec<String>, String> {
    let base = if enforce_containment {
        base_dir.and_then(|dir| dir.canonicalize().ok())
    } else {
        None
    };
    let mut validated = Vec::with_capacity(files.len());
    for file in files {
        let trimmed = file.trim();
        if trimmed.is_empty() || trimmed.contains('\0') {
            continue;
        }
        let path = std::path::Path::new(trimmed);
        if !runner::is_safe_open_file(path) {
            return Err("The extension reported an unsupported executable file".into());
        }
        if let Some(base) = &base {
            let resolved = if path.is_absolute() {
                path.canonicalize()
            } else {
                base.join(path).canonicalize()
            }
            .map_err(|_| "The extension reported a missing saved file".to_string())?;
            if !resolved.is_file() || !resolved.starts_with(base) {
                return Err("The extension reported a file outside its output folder".into());
            }
            let display = resolved.to_string_lossy().into_owned();
            let cleaned = display.strip_prefix(r"\\?\").unwrap_or(&display).to_owned();
            if !validated.contains(&cleaned) {
                validated.push(cleaned);
            }
        } else if !validated.iter().any(|existing| existing == trimmed) {
            validated.push(trimmed.to_owned());
        }
    }
    if validated.is_empty() {
        return Err("The extension did not report a saved file".into());
    }
    Ok(validated)
}

/// Applies one output line to a running job; returns a cover URL to fetch.
fn job_line(job: &mut Job, line: String) -> Option<String> {
    match protocol::parse(&line) {
        Some(Event::Track { title, artist, album, cover }) => {
            job.title = Some(title).filter(|t| !t.is_empty());
            job.artist = Some(artist).filter(|a| !a.is_empty());
            job.album = Some(album).filter(|a| !a.is_empty());
            return cover.filter(|_| job.cover.is_none());
        }
        Some(Event::Quality(text)) => job.quality = Some(text),
        Some(Event::Stage(text)) => {
            job.stage = Some(text);
            job.progress = None;
        }
        Some(Event::Progress(value)) => job.progress = Some(value),
        Some(Event::Saved(path)) => {
            if !job.saved.contains(&path) {
                job.saved.push(path);
            }
        }
        Some(Event::Links(links)) => job.links = links,
        Some(Event::Services(_)) => {}
        None if line.starts_with(protocol::PREFIX) => {}
        None if job.extension.manifest.kind == Kind::Resolver
            && line.split_once(':').is_some_and(|(name, url)| {
                services::is_excluded_service(name)
                    || extensions::url_host(url.trim()).is_some_and(|host| services::is_excluded_host(&host))
            }) => {}
        None => {
            if let Some(path) = protocol::saved_line(&line)
                && !job.saved.contains(&path)
            {
                job.saved.push(path);
            }
            job.log.push(line);
            if job.log.len() > LOG_LIMIT {
                job.log.drain(..job.log.len() - LOG_LIMIT);
            }
        }
    }
    None
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl App {
    /// Queues every link in the box; links no installed extension accepts are reported.
    pub fn enqueue(&mut self) {
        if self.wait_for_import() {
            return;
        }
        let links: Vec<String> =
            self.url.text.split_whitespace().filter(|w| extensions::url_host(w).is_some()).map(str::to_owned).collect();
        if links.is_empty() {
            return;
        }
        if let [url] = links.as_slice() && crate::catalog::is_catalog_url(url) {
            self.url.clear();
            self.batch_destination = None;
            self.open_catalog(url.clone(), false);
            return;
        }
        let mut unsupported = Vec::new();
        let mut added = 0;
        let batch_service = (links.len() > 1).then(|| self.batch_service()).flatten();
        for url in links {
            if crate::catalog::is_catalog_url(&url) {
                self.warn("Open a catalog link on its own to choose tracks from it.");
                continue;
            }
            let host = extensions::url_host(&url).unwrap_or_default();
            // A downloader takes the link; otherwise a resolver finds it elsewhere first.
            let Some(extension) = self.selected_for(&host).or_else(|| self.resolver_for(&host)).cloned() else {
                unsupported.push(host);
                continue;
            };
            let picks = extension.manifest.options.iter().map(|o| self.pick_index(&extension.manifest.id, o)).collect();
            let id = self.next_job;
            self.next_job += 1;
            let mut job = Job::queued(id, url, services::detect(&host), extension, picks, &self.settings);
            if job.extension.manifest.kind == Kind::Resolver {
                job.batch_service = batch_service;
            }
            self.jobs.push(job);
            added += 1;
        }
        if added > 0 {
            self.url.clear();
            self.batch_destination = None;
        }
        if !unsupported.is_empty() {
            unsupported.dedup();
            self.warn(format!("No installed extension handles {}.", unsupported.join(", ")));
        }
        self.pump();
    }

    pub fn enqueue_lookup(&mut self, index: usize, extension_id: &str) {
        if self.wait_for_import() {
            return;
        }
        let Some(entry) = self.history.get(index).cloned() else { return };
        let Some(extension) = self.installed.iter().find(|i| i.manifest.id == extension_id && i.manifest.supports_lookup()).cloned() else {
            return;
        };
        if self.removing.contains(extension_id) {
            return;
        }
        if let Some(message) = extension.manifest.tag_writer_update() {
            self.warn(message);
            return;
        }
        let Some(job) = self.lookup_job(&entry, extension) else { return };
        self.jobs.push(job);
        self.lookup_picker = None;
        self.page = Page::Downloads;
        self.pump();
    }

    fn lookup_job(&mut self, entry: &Entry, extension: Installed) -> Option<Job> {
        let file = entry.flac_file()?;
        let Ok(file) = std::path::absolute(file) else {
            self.error("Could not resolve the track's file path.");
            return None;
        };
        let id = &extension.manifest.id;
        if self.jobs.iter().any(|j| !j.is_finished() && &j.extension.manifest.id == id && j.lookup_file.as_ref() == Some(&file)) {
            self.warn("This lookup is already in the queue.");
            return None;
        }
        let picks = extension.manifest.options.iter().map(|o| self.pick_index(id, o)).collect();
        let mut job = Job::queued(self.next_job, entry.url.clone(), None, extension, picks, &self.settings);
        job.lookup_file = Some(file);
        job.title = Some(entry.title.clone());
        job.artist = Some(entry.artist.clone()).filter(|s| !s.is_empty());
        job.album = Some(entry.album.clone()).filter(|s| !s.is_empty());
        job.cover = entry.cover.clone();
        self.next_job += 1;
        Some(job)
    }

    /// Starts the next queued job when nothing is running.
    pub fn pump(&mut self) {
        if self.queue_paused || self.link.is_none() || self.jobs.iter().any(Job::is_running) {
            return;
        }
        let Some(index) = self.jobs.iter().position(|j| j.status == Status::Queued) else { return };
        let job = &self.jobs[index];
        let id = &job.extension.manifest.id;
        if self.removing.contains(id) || !self.installed.iter().any(|i| i.manifest.id == *id) {
            let name = job.extension.manifest.name.clone();
            self.jobs[index].status = Status::Failed(format!("Install {name} to run this job."));
            return self.pump();
        }
        let Some(program) = job.extension.executable() else {
            let name = job.extension.manifest.name.clone();
            self.jobs[index].status = Status::Failed(format!("{name} has no build for this computer"));
            return self.pump();
        };
        let manifest = job.extension.manifest.clone();
        let (data, browser) = (self.paths.data(&manifest.id), self.paths.browser());
        let flacora_home = std::path::absolute(&self.paths.root).unwrap_or_else(|_| self.paths.root.clone());
        let output_dir = job.output.clone();
        let layout = job.layout;
        let url = job.url.clone();
        let lookup_file = job.lookup_file.clone();
        let picks = job.picks.clone();
        let has_monochrome = manifest.kind == Kind::Resolver && !self.candidates_for("tracks.monochrome.st").is_empty();
        let job_id = job.id;
        let Some(link) = &self.link else { return };
        let msg_send = link.sender();
        let handle = JobHandle::detached();
        let job = &mut self.jobs[index];
        job.status = Status::Running;
        job.started.get_or_insert_with(Instant::now);
        job.stage = Some("Starting".into());
        job.handle = Some(handle.clone());
        std::thread::spawn(move || {
            let staging = if manifest.kind == Kind::Downloader {
                match layout.staging(&output_dir) {
                    Ok(staging) => staging,
                    Err(error) => {
                        msg_send(Msg::Job(
                            job_id,
                            JobEvent::Exit(Err(format!("Could not prepare the download folder: {error}"))),
                        ));
                        return;
                    }
                }
            } else {
                None
            };
            let output_text = staging
                .as_ref()
                .map_or(output_dir.as_path(), |dir| dir.path())
                .to_string_lossy()
                .into_owned();
            if let Some(staging) = staging {
                msg_send(Msg::JobStaging(job_id, staging));
            }
            let (data_text, browser_text) = (
                data.to_string_lossy().into_owned(),
                browser.to_string_lossy().into_owned(),
            );
            let file_text = lookup_file.as_deref().map(|p| p.to_string_lossy()).unwrap_or_default();
            let fill = Fill {
                url: &url,
                file: &file_text,
                output: &output_text,
                data: &data_text,
                browser: &browser_text,
            };
            let mut env = extensions::command_env(&manifest, &fill);
            if manifest.requires.iter().any(|required| required == "ffmpeg") {
                env.retain(|(key, _)| !key.eq_ignore_ascii_case("FLACORA_HOME"));
                env.push(("FLACORA_HOME".into(), flacora_home.to_string_lossy().into_owned()));
            }
            if has_monochrome {
                env.push(("FLACORA_MONOCHROME".into(), "1".into()));
            }
            let spec = runner::Spec {
                program,
                args: extensions::command_args(&manifest, &fill, &picks),
                env,
                cwd: data,
                browser_root: manifest.browser.then(|| flacora_home.clone()),
            };
            let event_send = Arc::new(move |event| msg_send(Msg::Job(job_id, event)));
            if let Err(error) = runner::spawn_into(handle, spec, event_send.clone()) {
                event_send(JobEvent::Exit(Err(error)));
            }
        });
    }

    pub fn job_event(&mut self, job_id: u64, event: JobEvent) {
        let Some(index) = self.jobs.iter().position(|j| j.id == job_id && j.is_running()) else { return };
        match event {
            JobEvent::Line(line) => {
                if self.jobs[index].cancelling {
                    return;
                }
                if let Some(url) = job_line(&mut self.jobs[index], line) {
                    let paths = self.paths.clone();
                    self.background_opt(move || {
                        library::cover_from_url(&paths, &url).map(|(path, bytes)| Msg::JobCover(job_id, path, bytes))
                    });
                }
            }
            JobEvent::Exit(exit) => {
                let job = &mut self.jobs[index];
                let cancelled = job.cancelling || job.handle.take().is_none();
                job.handle = None;
                job.cancelling = false;
                job.progress = None;
                job.stage = None;
                job.status = match exit {
                    _ if cancelled => Status::Cancelled,
                    Ok(0) => Status::Done,
                    Ok(code) => Status::Failed(
                        job.log
                            .iter()
                            .rev()
                            .find_map(|l| protocol::error_line(l))
                            .unwrap_or(format!("The extension exited with code {code}")),
                    ),
                    Err(e) => Status::Failed(e),
                };
                let staging = job.staging.take();
                let enforce_containment = self.link.is_some();
                if job.status == Status::Done {
                    match (job.extension.manifest.kind, staging) {
                        (Kind::Resolver, _) => self.offer_choices(job_id),
                        _ if job.saved.is_empty() => {
                            job.status = Status::Failed("The extension did not report a saved file".into());
                        }
                        (Kind::Downloader, Some(staging)) => {
                            let layout = job.layout;
                            let output = job.output.clone();
                            let files = std::mem::take(&mut job.saved);
                            let names = TrackNames { title: job.title.clone(), artist: job.artist.clone(), album: job.album.clone() };
                            job.status = Status::Running;
                            job.stage = Some("Organizing Download".into());
                            self.background(move || Msg::DownloadPlaced(job_id, layout.place(staging, &output, &files, names)));
                            return;
                        }
                        (Kind::Downloader, None) => {
                            if enforce_containment {
                                let output = job.output.clone();
                                let files = std::mem::take(&mut job.saved);
                                job.status = Status::Running;
                                job.stage = Some("Verifying Download".into());
                                self.background(move || Msg::JobValidated(job_id, validate_saved_files(Some(&output), &files, true)));
                                return;
                            }
                            match validate_saved_files(Some(&job.output), &job.saved, false) {
                                Ok(valid) => {
                                    job.saved = valid;
                                    self.record(job_id);
                                }
                                Err(error) => job.status = Status::Failed(error),
                            }
                        }
                        _ => {
                            let file = job.lookup_file.clone();
                            let parent = file.as_deref().and_then(Path::parent).map(PathBuf::from);
                            if enforce_containment {
                                let files = std::mem::take(&mut job.saved);
                                job.status = Status::Running;
                                job.stage = Some("Verifying Output".into());
                                if job.extension.manifest.can_write_tags() && let Some(file) = file {
                                    let paths = self.paths.clone();
                                    self.background(move || {
                                        let result = (|| {
                                            let mut validated = validate_saved_files(parent.as_deref(), &files, true)?;
                                            let expected = file.canonicalize().map_err(|_| "The FLAC file is missing")?;
                                            if validated.len() != 1 || Path::new(&validated[0]).canonicalize().ok().as_ref() != Some(&expected) {
                                                return Err("The extension did not report the original FLAC as tagged".into());
                                            }
                                            library::read_tagged_track(&paths, validated.remove(0))
                                        })();
                                        Msg::TagsWritten(job_id, result)
                                    });
                                } else {
                                    self.background(move || Msg::JobValidated(job_id, validate_saved_files(parent.as_deref(), &files, true)));
                                }
                                return;
                            }
                            match validate_saved_files(parent.as_deref(), &job.saved, false) {
                                Ok(valid) => {
                                    job.saved = valid;
                                    self.refresh_library_files();
                                }
                                Err(error) => job.status = Status::Failed(error),
                            }
                        }
                    }
                }
                self.refresh_tools_if_missing();
                self.pump();
                self.pump_status_checks();
            }
        }
    }

    pub fn job_validated(&mut self, job_id: u64, result: Result<Vec<String>, String>) {
        let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id && j.is_running()) else { return };
        let cancelled = job.cancelling;
        let is_downloader = job.extension.manifest.kind == Kind::Downloader;
        job.cancelling = false;
        job.stage = None;
        if cancelled {
            job.status = Status::Cancelled;
            self.refresh_tools_if_missing();
            self.pump();
            self.pump_status_checks();
            return;
        }
        match result {
            Ok(valid) => {
                job.saved = valid;
                job.status = Status::Done;
                if is_downloader {
                    self.record(job_id);
                } else {
                    self.refresh_library_files();
                }
            }
            Err(error) => job.status = Status::Failed(error),
        }
        self.refresh_tools_if_missing();
        self.pump();
        self.pump_status_checks();
    }

    pub fn tags_written(&mut self, job_id: u64, result: Result<library::TaggedTrack, String>) {
        let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id && j.is_running()) else { return };
        let track = match result {
            Ok(track) => track,
            Err(error) => return self.job_validated(job_id, Err(error)),
        };
        let Some(file) = job.lookup_file.clone() else {
            return self.job_validated(job_id, Err("The job has no FLAC file to tag".into()));
        };
        let set = |field: &mut Option<String>, value: &String| {
            if !value.is_empty() {
                *field = Some(value.clone());
            }
        };
        set(&mut job.title, &track.title);
        set(&mut job.artist, &track.artist);
        set(&mut job.album, &track.album);
        let cover = track.cover.map(|(path, bytes)| {
            let path = path.to_string_lossy().into_owned();
            job.cover = Some(path.clone());
            (path, bytes)
        });
        let mut updated = false;
        for entry in &mut self.history {
            if entry.flac_file().and_then(|path| std::path::absolute(path).ok()).as_ref() != Some(&file) {
                continue;
            }
            for (field, value) in [(&mut entry.title, &track.title), (&mut entry.artist, &track.artist), (&mut entry.album, &track.album)] {
                if !value.is_empty() {
                    field.clone_from(value);
                }
            }
            if let Some((path, _)) = &cover {
                entry.cover = Some(path.clone());
            }
            updated = true;
        }
        if let Some((path, bytes)) = cover {
            self.load_image(path, bytes);
        }
        if updated {
            self.save_history();
        }
        self.job_validated(job_id, Ok(vec![track.file]));
    }

    pub fn download_placed(&mut self, job_id: u64, result: Result<Vec<String>, String>) {
        let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id && j.is_running()) else { return };
        let cancelled = job.cancelling;
        job.cancelling = false;
        job.stage = None;
        if cancelled {
            job.status = Status::Cancelled;
            if let Ok(files) = result && !files.is_empty() {
                self.background_opt(move || {
                    for path in files {
                        let _ = std::fs::remove_file(path);
                    }
                    None
                });
            }
            self.refresh_tools_if_missing();
            self.pump();
            self.pump_status_checks();
            return;
        }
        match result {
            Ok(files) => {
                job.saved = files;
                job.status = Status::Done;
                self.record(job_id);
            }
            Err(error) => job.status = Status::Failed(error),
        }
        self.refresh_tools_if_missing();
        self.pump();
        self.pump_status_checks();
    }

    /// Offers downloadable results, applying a service already chosen for the batch if available.
    fn offer_choices(&mut self, job_id: u64) {
        let Some(index) = self.jobs.iter().position(|j| j.id == job_id) else { return };
        let mut choices: Vec<(usize, usize, Choice)> = Vec::new();
        for (_, url) in &self.jobs[index].links {
            let Some(host) = extensions::url_host(url) else { continue };
            let (Some(downloader), Some(service)) = (self.selected_for(&host), services::detect(&host)) else { continue };
            if choices.iter().any(|(_, _, c)| c.service.key == service.key) {
                continue;
            }
            let service_rank = services::SERVICES.iter().position(|s| s.key == service.key).unwrap_or(usize::MAX);
            let choice = Choice {
                service,
                url: url.clone(),
                downloader: downloader.manifest.id.clone(),
                downloader_name: downloader.manifest.name.clone(),
            };
            choices.push((extensions::rank(&downloader.manifest.id), service_rank, choice));
        }
        choices.sort_by_key(|(rank, service, _)| (*rank, *service));
        let resolver = self.jobs[index].extension.manifest.name.clone();
        let job = &mut self.jobs[index];
        if choices.is_empty() {
            let found: Vec<&str> = job.links.iter().map(|(service, _)| service.as_str()).collect();
            job.status = Status::Failed(if found.is_empty() {
                format!("{resolver} found this track on no other service")
            } else {
                format!("Found on {}, but no installed extension downloads from there", found.join(", "))
            });
            return;
        }
        job.choices = choices.into_iter().map(|(_, _, c)| c).collect();
        job.status = Status::Choosing;
        if self.importing == 0 {
            self.apply_batch_service(index);
        }
    }

    pub(crate) fn apply_waiting_batch_services(&mut self) {
        let waiting: Vec<_> = self.jobs.iter()
            .filter(|job| job.status == Status::Choosing && job.batch_service.is_some())
            .map(|job| job.id).collect();
        if waiting.is_empty() {
            return;
        }
        for id in waiting {
            self.offer_choices(id);
        }
        self.pump();
    }

    fn apply_batch_service(&mut self, index: usize) {
        let job = &self.jobs[index];
        let Some(service) = job.batch_service else { return };
        let Some(choice) = job.choices.iter().find(|choice| choice.service == service) else { return };
        let Some(downloader) = extensions::url_host(&choice.url)
            .and_then(|host| self.selected_for(&host)).cloned() else { return };
        self.queue_resolved_job(index, service, choice.url.clone(), downloader);
    }

    /// Hands a job waiting for a choice to the downloader of `choice` and queues it.
    pub fn choose(&mut self, job_id: u64, choice: usize) {
        if self.wait_for_import() {
            return;
        }
        let Some(index) = self.jobs.iter().position(|j| j.id == job_id && j.status == Status::Choosing) else { return };
        let Some(picked) = self.jobs[index].choices.get(choice).cloned() else { return };
        let Some(downloader) = self.installed.iter().find(|i| i.manifest.id == picked.downloader
            && !self.removing.contains(&i.manifest.id)).cloned() else {
            self.jobs[index].status = Status::Failed(format!("{} is no longer installed", picked.downloader_name));
            return;
        };
        self.queue_resolved_job(index, picked.service, picked.url, downloader);
        self.pump();
    }

    fn queue_resolved_job(&mut self, index: usize, service: &'static Service, url: String, downloader: Installed) {
        let picks = downloader.manifest.options.iter().map(|o| self.pick_index(&downloader.manifest.id, o)).collect();
        let job = &mut self.jobs[index];
        let resolver = job.extension.manifest.name.clone();
        job.log.push(format!("{resolver}: chose {}, downloading with {}", service.name, downloader.manifest.name));
        job.resolved = Some((resolver, job.service));
        job.service = Some(service);
        job.url = url;
        job.picks = picks;
        job.extension = downloader;
        job.choices.clear();
        job.status = Status::Queued;
    }

    /// Adds a finished job to the history and finds its cover when it has none yet.
    fn record(&mut self, job_id: u64) {
        let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) else { return };
        let date = now();
        job.finished = Some(date);
        let fallback_title = job.main_file().and_then(|f| PathBuf::from(f).file_stem().map(|s| s.to_string_lossy().into_owned()));
        let entry = Entry {
            title: job.title.clone().or(fallback_title).unwrap_or_else(|| job.url.clone()),
            artist: job.artist.clone().unwrap_or_default(),
            album: job.album.clone().unwrap_or_default(),
            service: job.service.map(|s| s.name.to_owned()).unwrap_or_default(),
            extension: job.extension.manifest.name.clone(),
            quality: job.quality.clone().unwrap_or_default(),
            url: job.url.clone(),
            files: job.saved.clone(),
            cover: job.cover.clone(),
            date,
        };
        let flac = job.saved.iter().find(|f| f.to_lowercase().ends_with(".flac")).cloned();
        let needs_cover = job.cover.is_none();
        let auto_tag = job.auto_tag;
        if auto_tag
            && let Some(tagger) = self.tag_writer_for(&entry.url).cloned()
            && let Some(tag_job) = self.lookup_job(&entry, tagger)
        {
            let next = self.jobs.iter().position(|job| job.status == Status::Queued).unwrap_or(self.jobs.len());
            self.jobs.insert(next, tag_job);
        }
        self.history.insert(0, entry);
        self.save_history();
        self.refresh_library_files();
        // Extensions whose server tags the file may not report artwork; read it from the file.
        if let (true, Some(flac)) = (needs_cover, flac) {
            let paths = self.paths.clone();
            self.background_opt(move || {
                library::cover_from_flac(&paths, &PathBuf::from(flac)).map(|(path, bytes)| Msg::JobCover(job_id, path, bytes))
            });
        }
    }

    pub fn download_tag_writers(&self) -> impl Iterator<Item = &Installed> {
        self.installed.iter().filter(|extension| {
            let manifest = &extension.manifest;
            manifest.can_write_tags() && !self.removing.contains(&manifest.id)
                && !manifest.hosts.is_empty()
                && extension.executable().is_some()
        })
    }

    fn tag_writer_for(&self, url: &str) -> Option<&Installed> {
        let host = extensions::url_host(url)?;
        let mut matches = self.download_tag_writers()
            .filter(|extension| extensions::host_matches(&extension.manifest.hosts, &host));
        let writer = matches.next()?;
        matches.next().is_none().then_some(writer)
    }

    pub fn job_cover(&mut self, job_id: u64, path: PathBuf, bytes: Vec<u8>) {
        let path = path.to_string_lossy().into_owned();
        self.load_image(path.clone(), bytes);
        let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) else { return };
        job.cover = Some(path.clone());
        if let Some(date) = job.finished
            && let Some(entry) = self.history.iter_mut().find(|e| e.date == date && e.url == job.url)
            && entry.cover.is_none()
        {
            entry.cover = Some(path);
            self.save_history();
        }
    }

    /// Fails jobs that have been running longer than `limit` without an exit
    /// event. A browser wedged on a locked profile used to leave the queue stuck
    /// in `Running` until the container was recreated.
    pub fn timeout_stalled_jobs(&mut self, limit: Duration) -> bool {
        let now = Instant::now();
        let stalled: Vec<u64> = self.jobs.iter()
            .filter(|job| job.is_running() && job.started.is_some_and(|s| now.duration_since(s) > limit))
            .map(|job| job.id)
            .collect();
        if stalled.is_empty() {
            return false;
        }
        let elapsed = if limit.as_secs() < 60 {
            format!("{} seconds", limit.as_secs())
        } else {
            format!("{} minutes", limit.as_secs() / 60)
        };
        let reason = format!("Timed out after {elapsed} (the browser may be locked).");
        for id in stalled {
            let Some(job) = self.jobs.iter_mut().find(|j| j.id == id && j.is_running()) else { continue };
            if let Some(handle) = job.handle.take() {
                handle.cancel();
            }
            job.cancelling = false;
            job.stage = None;
            job.progress = None;
            job.log.push(reason.clone());
            job.status = Status::Failed(reason.clone());
        }
        self.pump();
        true
    }

    pub fn cancel_job(&mut self, job_id: u64) {
        let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) else { return };
        match job.status {
            Status::Running => {
                job.cancelling = true;
                if let Some(handle) = job.handle.take() {
                    handle.cancel();
                }
            }
            Status::Queued | Status::Choosing => {
                job.choices.clear();
                job.status = Status::Cancelled;
            }
            _ => {}
        }
    }

    pub fn retry_job(&mut self, job_id: u64) {
        if self.wait_for_import() {
            return;
        }
        let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id && j.is_finished()) else { return };
        job.status = Status::Queued;
        job.cancelling = false;
        job.log.clear();
        job.saved.clear();
        job.progress = None;
        job.stage = None;
        job.finished = None;
        job.started = None;
        self.pump();
    }

    pub fn remove_job(&mut self, job_id: u64) {
        self.jobs.retain(|j| j.id != job_id || j.is_running());
    }

    pub fn clear_finished(&mut self) {
        let mut kept = Vec::with_capacity(self.jobs.len());
        let mut removed = Vec::new();
        for job in self.jobs.drain(..) {
            if job.is_finished() {
                removed.push(job);
            } else {
                kept.push(job);
            }
        }
        self.jobs = kept;
        if !removed.is_empty() {
            let count = removed.len();
            self.success_undo(
                format!("Cleared {count} finished job{}", if count == 1 { "" } else { "s" }),
                crate::app::UndoAction::FinishedJobs(removed),
            );
        }
    }

    pub fn cancel_all_jobs(&mut self) {
        let ids: Vec<u64> = self
            .jobs
            .iter()
            .filter(|j| matches!(j.status, Status::Queued | Status::Running | Status::Choosing))
            .map(|j| j.id)
            .collect();
        for id in ids {
            self.cancel_job(id);
        }
    }

    pub fn retry_failed_jobs(&mut self) {
        if self.wait_for_import() {
            return;
        }
        let mut retried = false;
        for job in &mut self.jobs {
            if matches!(job.status, Status::Failed(_) | Status::Cancelled) {
                job.status = Status::Queued;
                job.cancelling = false;
                job.log.clear();
                job.saved.clear();
                job.progress = None;
                job.stage = None;
                job.finished = None;
        job.started = None;
                retried = true;
            }
        }
        if retried {
            self.pump();
        }
    }

    pub fn toggle_log(&mut self, job_id: u64) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) {
            job.show_log = !job.show_log;
        }
    }

    pub fn toggle_queue_pause(&mut self) {
        self.queue_paused = !self.queue_paused;
        if !self.queue_paused {
            self.pump();
        }
    }

    pub fn failed_urls(&self) -> Vec<String> {
        self.jobs
            .iter()
            .filter(|j| matches!(j.status, Status::Failed(_)))
            .map(|j| j.url.clone())
            .collect()
    }

    pub fn active_jobs(&self) -> usize {
        self.jobs.iter().filter(|j| matches!(j.status, Status::Queued | Status::Running | Status::Choosing)).count()
    }
}
