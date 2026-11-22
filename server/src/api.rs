//! HTTP API and file browsing for the headless engine.

use crate::engine::{Command, State};
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path as AxPath, Query, State as AxState};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;

pub struct Shared {
    pub cmd_tx: Sender<Command>,
    pub state: Mutex<Option<State>>,
}

type SharedState = AxState<Arc<Shared>>;

pub fn router(shared: Arc<Shared>) -> Router {
    Router::new()
        .route("/api/state", get(get_state))
        .route("/api/enqueue", post(post_enqueue))
        .route("/api/jobs/clear-finished", post(clear_finished))
        .route("/api/jobs/cancel-all", post(cancel_all))
        .route("/api/jobs/retry-failed", post(retry_failed))
        .route("/api/queue/pause", post(set_queue_pause))
        .route("/api/jobs/{id}/cancel", post(cancel_job))
        .route("/api/jobs/{id}/retry", post(retry_job))
        .route("/api/jobs/{id}/remove", post(remove_job))
        .route("/api/jobs/{id}/toggle-log", post(toggle_log))
        .route("/api/jobs/{id}/choose", post(choose))
        .route("/api/extensions/install", post(install))
        .route("/api/extensions/check-all", post(check_all))
        .route("/api/extensions/remove-all", post(remove_all_extensions))
        .route("/api/extensions/{id}/remove", post(remove_extension))
        .route("/api/extensions/{id}/check", post(check_extension))
        .route("/api/library/{index}/remove", post(remove_entry))
        .route("/api/library/clear", post(clear_history))
        .route("/api/library/{index}/lookup", post(lookup))
        .route("/api/settings/output", post(set_output))
        .route("/api/settings/layout", post(set_layout))
        .route("/api/settings/auto-tag", post(set_auto_tag))
        .route("/api/settings/preferred", post(set_preferred))
        .route("/api/settings/pick", post(set_pick))
        .route("/api/catalog/submit", post(catalog_submit))
        .route("/api/catalog/open", post(catalog_open))
        .route("/api/catalog/select", post(catalog_select))
        .route("/api/catalog/select-all", post(catalog_select_all))
        .route("/api/catalog/queue", post(catalog_queue))
        .route("/api/catalog/back", post(catalog_back))
        .route("/api/catalog/more", post(catalog_more))
        .route("/api/catalog/retry", post(catalog_retry))
        .route("/api/catalog/home", post(catalog_home))
        .route("/api/catalog/provider", post(catalog_provider))
        .route("/api/files", get(list_files))
        .route("/api/files/download", get(download_file))
        .route("/api/version", get(version))
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/style.css", get(style_css))
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .layer(CorsLayer::permissive())
        .with_state(shared)
}

pub async fn serve(host: String, port: u16, shared: Arc<Shared>) {
    let listener = match tokio::net::TcpListener::bind((host.as_str(), port)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("flacora-server: could not bind {host}:{port}: {error}");
            std::process::exit(1);
        }
    };
    let app = router(shared);
    if let Err(error) = axum::serve(listener, app).await {
        eprintln!("flacora-server: {error}");
        std::process::exit(1);
    }
}

// ------------------------------------------------------------------- helpers

fn send(cmd_tx: &Sender<Command>, command: Command) -> Response {
    match cmd_tx.send(command) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

fn latest(shared: &Shared) -> State {
    shared.state.lock().ok().and_then(|state| state.clone()).unwrap_or_default()
}

fn output_root(shared: &Arc<Shared>) -> PathBuf {
    PathBuf::from(latest(shared).settings.output)
}

/// Resolves `path` under `root`, refusing anything that escapes it.
fn contained(root: &Path, path: &str) -> Option<PathBuf> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let candidate = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        root.join(path)
    };
    let canonical = candidate.canonicalize().unwrap_or(candidate);
    canonical.starts_with(&root).then_some(canonical)
}

// -------------------------------------------------------------------- handlers

async fn get_state(state: SharedState) -> Json<State> {
    Json(latest(&state))
}

async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "flacora": flacora::updates::CURRENT_VERSION,
        "server": env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(Deserialize)]
struct EnqueueRequest {
    #[serde(default)]
    urls: Vec<String>,
    #[serde(default)]
    service: Option<String>,
    #[serde(default)]
    options: Vec<PickRequest>,
}

async fn post_enqueue(state: SharedState, Json(request): Json<EnqueueRequest>) -> Response {
    let options = request
        .options
        .into_iter()
        .map(|pick| (pick.extension, pick.option, pick.index))
        .collect();
    send(
        &state.cmd_tx,
        Command::Enqueue { urls: request.urls, service: request.service, options },
    )
}

async fn clear_finished(state: SharedState) -> Response {
    send(&state.cmd_tx, Command::ClearFinished)
}

async fn cancel_all(state: SharedState) -> Response {
    send(&state.cmd_tx, Command::CancelAll)
}

async fn retry_failed(state: SharedState) -> Response {
    send(&state.cmd_tx, Command::RetryFailed)
}

#[derive(Deserialize)]
struct PauseRequest {
    paused: bool,
}

async fn set_queue_pause(state: SharedState, Json(request): Json<PauseRequest>) -> Response {
    send(&state.cmd_tx, Command::SetQueuePaused(request.paused))
}

async fn cancel_job(state: SharedState, AxPath(id): AxPath<u64>) -> Response {
    send(&state.cmd_tx, Command::CancelJob(id))
}

async fn retry_job(state: SharedState, AxPath(id): AxPath<u64>) -> Response {
    send(&state.cmd_tx, Command::RetryJob(id))
}

async fn remove_job(state: SharedState, AxPath(id): AxPath<u64>) -> Response {
    send(&state.cmd_tx, Command::RemoveJob(id))
}

async fn toggle_log(state: SharedState, AxPath(id): AxPath<u64>) -> Response {
    send(&state.cmd_tx, Command::ToggleLog(id))
}

#[derive(Deserialize)]
struct ChooseRequest {
    choice: usize,
}

async fn choose(state: SharedState, AxPath(id): AxPath<u64>, Json(request): Json<ChooseRequest>) -> Response {
    send(&state.cmd_tx, Command::Choose { job: id, choice: request.choice })
}

async fn install(state: SharedState, mut multipart: Multipart) -> Response {
    let mut saved = Vec::new();
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
        };
        let file_name = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| format!("package-{}.fext", saved.len()));
        let bytes = match field.bytes().await {
            Ok(bytes) => bytes,
            Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
        };
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("flacora-upload-{unique}-{file_name}"));
        if let Err(error) = std::fs::write(&path, &bytes) {
            return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response();
        }
        saved.push(path);
    }
    if saved.is_empty() {
        return (StatusCode::BAD_REQUEST, "no package uploaded").into_response();
    }
    for path in saved {
        let _ = state.cmd_tx.send(Command::Install(path));
    }
    StatusCode::NO_CONTENT.into_response()
}

async fn check_all(state: SharedState) -> Response {
    send(&state.cmd_tx, Command::CheckAll)
}

async fn remove_all_extensions(state: SharedState) -> Response {
    send(&state.cmd_tx, Command::RemoveAllExtensions)
}

async fn remove_extension(state: SharedState, AxPath(id): AxPath<String>) -> Response {
    send(&state.cmd_tx, Command::RemoveExtension(id))
}

async fn check_extension(state: SharedState, AxPath(id): AxPath<String>) -> Response {
    send(&state.cmd_tx, Command::CheckExtension(id))
}

async fn remove_entry(state: SharedState, AxPath(index): AxPath<usize>) -> Response {
    send(&state.cmd_tx, Command::RemoveEntry(index))
}

async fn clear_history(state: SharedState) -> Response {
    send(&state.cmd_tx, Command::ClearHistory)
}

#[derive(Deserialize)]
struct LookupRequest {
    extension: String,
}

async fn lookup(
    state: SharedState,
    AxPath(index): AxPath<usize>,
    Json(request): Json<LookupRequest>,
) -> Response {
    send(&state.cmd_tx, Command::Lookup { index, extension: request.extension })
}

#[derive(Deserialize)]
struct OutputRequest {
    output: String,
}

async fn set_output(state: SharedState, Json(request): Json<OutputRequest>) -> Response {
    send(&state.cmd_tx, Command::SetOutput(PathBuf::from(request.output)))
}

#[derive(Deserialize)]
struct LayoutRequest {
    filename: String,
    folders: String,
}

async fn set_layout(state: SharedState, Json(request): Json<LayoutRequest>) -> Response {
    let filename = match request.filename.as_str() {
        "artist-title" => flacora::output::Filename::ArtistTitle,
        "title-artist" => flacora::output::Filename::TitleArtist,
        "title" => flacora::output::Filename::Title,
        _ => flacora::output::Filename::ExtensionDefault,
    };
    let folders = match request.folders.as_str() {
        "artist" => flacora::output::Folders::Artist,
        "artist-album" => flacora::output::Folders::ArtistAlbum,
        _ => flacora::output::Folders::Single,
    };
    send(&state.cmd_tx, Command::SetLayout(flacora::output::Layout { filename, folders }))
}

#[derive(Deserialize)]
struct AutoTagRequest {
    value: bool,
}

async fn set_auto_tag(state: SharedState, Json(request): Json<AutoTagRequest>) -> Response {
    send(&state.cmd_tx, Command::SetAutoTag(request.value))
}

#[derive(Deserialize)]
struct PreferredRequest {
    service: String,
    #[serde(default)]
    extension: Option<String>,
}

async fn set_preferred(state: SharedState, Json(request): Json<PreferredRequest>) -> Response {
    send(&state.cmd_tx, Command::SetPreferred { service: request.service, extension: request.extension })
}

#[derive(Deserialize)]
struct PickRequest {
    extension: String,
    option: String,
    index: usize,
}

async fn set_pick(state: SharedState, Json(request): Json<PickRequest>) -> Response {
    send(
        &state.cmd_tx,
        Command::SetPick { extension: request.extension, option: request.option, index: request.index },
    )
}

// -------------------------------------------------------------------- catalog

#[derive(Deserialize)]
struct CatalogInputRequest {
    #[serde(default)]
    input: String,
}

#[derive(Deserialize)]
struct CatalogSelectRequest {
    url: String,
    #[serde(default)]
    selected: bool,
}

#[derive(Deserialize)]
struct CatalogSelectAllRequest {
    #[serde(default)]
    selected: bool,
}

#[derive(Deserialize)]
struct CatalogProviderRequest {
    id: String,
}

async fn catalog_submit(state: SharedState, Json(request): Json<CatalogInputRequest>) -> Response {
    send(&state.cmd_tx, Command::CatalogSubmit { input: request.input })
}

async fn catalog_open(state: SharedState, Json(request): Json<CatalogInputRequest>) -> Response {
    send(&state.cmd_tx, Command::CatalogOpen { url: request.input })
}

async fn catalog_select(state: SharedState, Json(request): Json<CatalogSelectRequest>) -> Response {
    send(&state.cmd_tx, Command::CatalogSelect { url: request.url, selected: request.selected })
}

async fn catalog_select_all(state: SharedState, Json(request): Json<CatalogSelectAllRequest>) -> Response {
    send(&state.cmd_tx, Command::CatalogSelectAll { selected: request.selected })
}

async fn catalog_queue(state: SharedState) -> Response {
    send(&state.cmd_tx, Command::CatalogQueue)
}

async fn catalog_back(state: SharedState) -> Response {
    send(&state.cmd_tx, Command::CatalogBack)
}

async fn catalog_more(state: SharedState) -> Response {
    send(&state.cmd_tx, Command::CatalogMore)
}

async fn catalog_retry(state: SharedState) -> Response {
    send(&state.cmd_tx, Command::CatalogRetry)
}

async fn catalog_home(state: SharedState) -> Response {
    send(&state.cmd_tx, Command::CatalogHome)
}

async fn catalog_provider(state: SharedState, Json(request): Json<CatalogProviderRequest>) -> Response {
    send(&state.cmd_tx, Command::CatalogProvider { id: request.id })
}

// ---------------------------------------------------------------------- files

#[derive(Deserialize)]
struct FilesQuery {
    #[serde(default)]
    path: Option<String>,
}

#[derive(Serialize)]
struct FileDto {
    name: String,
    path: String,
    size: u64,
    is_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    modified: Option<u64>,
}

async fn list_files(state: SharedState, Query(query): Query<FilesQuery>) -> Response {
    let root = output_root(&state);
    let Some(path) = contained(&root, query.path.as_deref().unwrap_or("")) else {
        return (StatusCode::FORBIDDEN, "path is outside the output folder").into_response();
    };
    let mut entries = match tokio::fs::read_dir(&path).await {
        Ok(entries) => entries,
        Err(error) => return (StatusCode::NOT_FOUND, error.to_string()).into_response(),
    };
    let mut files = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let meta = match entry.metadata().await {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let modified = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs());
        files.push(FileDto {
            path: entry.path().to_string_lossy().into_owned(),
            name,
            size: meta.len(),
            is_dir: meta.is_dir(),
            modified,
        });
    }
    files.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Json(serde_json::json!({ "root": root.to_string_lossy(), "path": path.to_string_lossy(), "files": files }))
        .into_response()
}

#[derive(Deserialize)]
struct DownloadQuery {
    path: String,
}

async fn download_file(state: SharedState, Query(query): Query<DownloadQuery>) -> Response {
    let root = output_root(&state);
    let Some(path) = contained(&root, &query.path) else {
        return (StatusCode::FORBIDDEN, "path is outside the output folder").into_response();
    };
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) => return (StatusCode::NOT_FOUND, error.to_string()).into_response(),
    };
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    let file_name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{file_name}\""))
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

// ------------------------------------------------------------------ static UI

async fn index() -> Response {
    static_response("text/html; charset=utf-8", include_str!("../assets/index.html"))
}

async fn app_js() -> Response {
    static_response("text/javascript; charset=utf-8", include_str!("../assets/app.js"))
}

async fn style_css() -> Response {
    static_response("text/css; charset=utf-8", include_str!("../assets/style.css"))
}

fn static_response(content_type: &str, body: &'static str) -> Response {
    (
        [(
            header::CONTENT_TYPE,
            content_type.to_string(),
        )],
        body,
    )
        .into_response()
}
