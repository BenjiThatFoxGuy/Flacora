//! Download history (`history.json`) and the cover art cache (`covers/`).
//! Blocking; background threads only.

use crate::store::{self, Paths};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SidecarInfo {
    pub exists: bool,
    /// Detected sidecar files alongside the audio file: (short badge label, full path).
    pub sidecars: Vec<(String, String)>,
}

/// Checks whether each audio file exists on disk and discovers sibling sidecar files
/// (`.lrc`, `.ttml`, `.srt`, `.json`) in one directory scan per unique parent folder.
pub fn inspect_files(audio_paths: &[String]) -> BTreeMap<String, SidecarInfo> {
    let mut dir_cache: BTreeMap<PathBuf, Vec<(String, PathBuf)>> = BTreeMap::new();
    let mut results = BTreeMap::new();
    for raw in audio_paths {
        if results.contains_key(raw) {
            continue;
        }
        let path = Path::new(raw);
        let exists = path.is_file();
        let mut sidecars = Vec::new();
        if let (Some(parent), Some(stem)) = (path.parent(), path.file_stem().and_then(|s| s.to_str())) {
            let entries = dir_cache.entry(parent.to_path_buf()).or_insert_with(|| {
                fs::read_dir(parent)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|entry| {
                        let p = entry.path();
                        if !p.is_file() {
                            return None;
                        }
                        let name = p.file_name()?.to_str()?.to_owned();
                        Some((name, p))
                    })
                    .collect()
            });
            let prefix = format!("{}.", stem.to_lowercase());
            let mut seen_labels = BTreeSet::new();
            for (name, candidate) in entries {
                let lower = name.to_lowercase();
                if !lower.starts_with(&prefix) {
                    continue;
                }
                let ext = candidate.extension().and_then(|e| e.to_str()).unwrap_or("");
                let label = if ext.eq_ignore_ascii_case("lrc") {
                    "LRC"
                } else if ext.eq_ignore_ascii_case("ttml") {
                    "TTML"
                } else if ext.eq_ignore_ascii_case("srt") {
                    "SRT"
                } else if ext.eq_ignore_ascii_case("json") {
                    "Meta"
                } else {
                    continue;
                };
                if seen_labels.insert(label) {
                    sidecars.push((label.to_owned(), candidate.to_string_lossy().into_owned()));
                }
            }
            sidecars.sort_by(|a, b| a.0.cmp(&b.0));
        }
        results.insert(raw.clone(), SidecarInfo { exists, sidecars });
    }
    results
}

/// History entries kept on disk.
const LIMIT: usize = 500;
const MAX_COVER: u64 = 10 << 20;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Entry {
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub album: String,
    /// Service display name, e.g. "Tidal".
    #[serde(default)]
    pub service: String,
    /// Extension display name.
    #[serde(default)]
    pub extension: String,
    #[serde(default)]
    pub quality: String,
    pub url: String,
    /// Saved files; the first audio file is the main one.
    pub files: Vec<String>,
    /// Cached cover art file.
    #[serde(default)]
    pub cover: Option<String>,
    /// Unix seconds.
    pub date: u64,
}

impl Entry {
    pub fn flac_file(&self) -> Option<&str> {
        self.files.iter().find(|file| {
            Path::new(file).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("flac"))
        }).map(String::as_str)
    }

    /// The audio file to play or reveal.
    pub fn main_file(&self) -> Option<&str> {
        self.files.iter().find(|f| is_audio_file(f)).or(self.files.first()).map(String::as_str)
    }
}

pub fn is_audio_file(path: &str) -> bool {
    Path::new(path).extension().is_some_and(|ext| {
        ["flac", "m4a", "mp3", "ogg", "wav", "opus"]
            .iter()
            .any(|candidate| ext.eq_ignore_ascii_case(candidate))
    })
}

pub fn load(paths: &Paths) -> Vec<Entry> {
    let history_path = paths.history();
    let Ok(text) = fs::read_to_string(&history_path) else {
        return Vec::new();
    };
    match serde_json::from_str(&text) {
        Ok(entries) => entries,
        Err(_) => {
            store::backup_corrupt(&history_path);
            Vec::new()
        }
    }
}

/// Newest first, at most `LIMIT` entries.
pub fn save(paths: &Paths, entries: &[Entry]) -> Result<(), String> {
    fs::create_dir_all(&paths.root).map_err(|e| e.to_string())?;
    let kept = &entries[..entries.len().min(LIMIT)];
    let json = serde_json::to_string_pretty(kept).map_err(|e| e.to_string())?;
    store::write_atomic(&paths.history(), json.as_bytes())
}

/// Downloads cover art into the cache: HTTPS, JPEG or PNG, at most 10 MB.
pub fn cover_from_url(paths: &Paths, url: &str) -> Option<(PathBuf, Vec<u8>)> {
    cache(paths, fetch_image(&store::client().ok()?, url, MAX_COVER)?)
}

pub fn fetch_image(client: &reqwest::blocking::Client, url: &str, limit: u64) -> Option<Vec<u8>> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.scheme() != "https" || !parsed.username().is_empty() || parsed.password().is_some() {
        return None;
    }
    let response = client.get(parsed).send().ok()?.error_for_status().ok()?;
    let mut bytes = Vec::new();
    response.take(limit + 1).read_to_end(&mut bytes).ok()?;
    let webp = bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP");
    (bytes.len() as u64 <= limit && (is_image(&bytes) || webp)).then_some(bytes)
}

/// The front cover embedded in a FLAC file (any picture when there is no front cover).
pub fn cover_from_flac(paths: &Paths, file: &Path) -> Option<(PathBuf, Vec<u8>)> {
    let tag = metaflac::Tag::read_from_path(file).ok()?;
    let pictures: Vec<_> = tag.pictures().collect();
    let picture = pictures
        .iter()
        .find(|p| p.picture_type == metaflac::block::PictureType::CoverFront)
        .or(pictures.first())?;
    cache(paths, picture.data.clone())
}

/// Reads a cached cover, ensuring it stays within `paths.covers()` and the size limit.
pub fn read_cover(paths: &Paths, path: &str) -> Option<Vec<u8>> {
    let covers_dir = paths.covers().canonicalize().ok()?;
    let candidate = Path::new(path).canonicalize().ok()?;
    if !candidate.starts_with(&covers_dir) {
        return None;
    }
    let metadata = fs::metadata(&candidate).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_COVER {
        return None;
    }
    let bytes = fs::read(&candidate).ok()?;
    is_image(&bytes).then_some(bytes)
}

/// Removes cached cover files in `paths.covers()` that are no longer referenced.
pub fn prune_unused_covers(paths: &Paths, keep: &[String]) {
    let Ok(entries) = fs::read_dir(paths.covers()) else { return };
    let kept: std::collections::BTreeSet<PathBuf> = keep
        .iter()
        .filter_map(|p| Path::new(p).canonicalize().ok())
        .collect();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.canonicalize().is_ok_and(|c| !kept.contains(&c)) {
            let _ = fs::remove_file(path);
        }
    }
}

fn cache(paths: &Paths, bytes: Vec<u8>) -> Option<(PathBuf, Vec<u8>)> {
    if !is_image(&bytes) || bytes.len() as u64 > MAX_COVER {
        return None;
    }
    let extension = if bytes.starts_with(b"\x89PNG") { "png" } else { "jpg" };
    let name = format!("{:x}", Sha256::digest(&bytes));
    let path = paths.covers().join(format!("{}.{extension}", &name[..32]));
    if !path.exists() {
        fs::create_dir_all(paths.covers()).ok()?;
        store::write_atomic(&path, &bytes).ok()?;
    }
    Some((path, bytes))
}

fn is_image(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8]) || bytes.starts_with(b"\x89PNG")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_cover_rejects_paths_outside_covers_dir_and_corrupt_history_is_backed_up() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths { root: dir.path().to_path_buf() };
        fs::create_dir_all(paths.covers()).unwrap();
        let inside = paths.covers().join("valid.png");
        fs::write(&inside, b"\x89PNG\r\n\x1a\nrest").unwrap();
        let outside = dir.path().join("outside.png");
        fs::write(&outside, b"\x89PNG\r\n\x1a\nrest").unwrap();

        assert!(read_cover(&paths, &inside.to_string_lossy()).is_some());
        assert!(read_cover(&paths, &outside.to_string_lossy()).is_none());

        fs::write(paths.history(), "{not json").unwrap();
        assert!(load(&paths).is_empty());
        assert!(paths.root.join("history.json.corrupt").is_file());
    }

    #[test]
    fn inspect_files_discovers_sibling_sidecars_and_missing_audio() {
        let dir = tempfile::tempdir().unwrap();
        let track = dir.path().join("Artist - Song.flac");
        let lrc = dir.path().join("Artist - Song.lrc");
        let meta = dir.path().join("Artist - Song.musicbrainz.json");
        let missing = dir.path().join("Missing.flac");
        fs::write(&track, b"fLaC").unwrap();
        fs::write(&lrc, b"[00:01.00] hi").unwrap();
        fs::write(&meta, b"{}").unwrap();

        let track_s = track.to_string_lossy().into_owned();
        let missing_s = missing.to_string_lossy().into_owned();
        let info = inspect_files(&[track_s.clone(), missing_s.clone()]);
        assert!(info[&track_s].exists);
        let labels: Vec<&str> = info[&track_s].sidecars.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["LRC", "Meta"]);
        assert!(!info[&missing_s].exists);
        assert!(info[&missing_s].sidecars.is_empty());
    }
}
