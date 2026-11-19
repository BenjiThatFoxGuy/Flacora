//! Structured lines extensions print when `FLACORA_EVENTS=1` is set:
//! `@flacora {"event": "track" | "quality" | "stage" | "progress" | "saved", ...}`;
//! resolvers print `links` and status checks print `services`.
//! Everything else an extension prints is plain log text.

use crate::{extensions, services};
use serde_json::Value;

pub const PREFIX: &str = "@flacora ";

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Track { title: String, artist: String, album: String, cover: Option<String> },
    Quality(String),
    Stage(String),
    /// 0.0 ..= 1.0
    Progress(f32),
    Saved(String),
    /// A resolver's result: service name and HTTPS link.
    Links(Vec<(String, String)>),
    /// A status check's result: services that work right now.
    Services(Vec<String>),
}

fn https_url(value: &str) -> Option<String> {
    let url = reqwest::Url::parse(value.trim()).ok()?;
    (url.scheme() == "https" && url.has_host() && url.username().is_empty() && url.password().is_none())
        .then(|| value.trim().to_owned())
}

/// `None` for ordinary log lines and for events Flacora doesn't know.
pub fn parse(line: &str) -> Option<Event> {
    let json: Value = serde_json::from_str(line.strip_prefix(PREFIX)?).ok()?;
    let text = |key: &str| json[key].as_str().unwrap_or("").trim().to_owned();
    let non_empty = |key: &str| {
        let value = text(key);
        (!value.is_empty()).then_some(value)
    };
    Some(match json["event"].as_str()? {
        "track" => Event::Track {
            title: text("title"),
            artist: text("artist"),
            album: text("album"),
            cover: json["cover"].as_str().and_then(https_url),
        },
        "quality" => Event::Quality(non_empty("text")?),
        "stage" => Event::Stage(non_empty("text")?),
        "progress" => Event::Progress(json["value"].as_f64()?.clamp(0.0, 1.0) as f32),
        "saved" => Event::Saved(non_empty("path")?),
        "links" => Event::Links(
            json["links"]
                .as_object()?
                .iter()
                .filter_map(|(service, url)| {
                    let url = https_url(url.as_str()?)?;
                    let excluded = services::is_excluded_service(service)
                        || extensions::url_host(&url).is_some_and(|host| services::is_excluded_host(&host));
                    (!excluded).then(|| (service.clone(), url))
                })
                .collect(),
        ),
        "services" => Event::Services(
            json["services"]
                .as_array()?
                .iter()
                .filter_map(|s| s.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty() && !services::is_excluded_service(s))
                .map(str::to_owned)
                .collect(),
        ),
        _ => return None,
    })
}

/// Fallback for extensions without events: "Saved <path> (...)" lines.
pub fn saved_line(line: &str) -> Option<String> {
    let rest = line.strip_prefix("Saved ")?.trim();
    let path = if rest.ends_with(')') {
        rest.rsplit_once(" (").map_or(rest, |(path, _)| path).trim()
    } else {
        rest
    };
    (!path.is_empty()).then(|| path.to_owned())
}

/// The extension's own "Error: ..." line, if it printed one.
pub fn error_line(line: &str) -> Option<String> {
    line.strip_prefix("Error: ").map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_parse_and_plain_lines_do_not() {
        let track = r#"@flacora {"event":"track","title":"Golden","artist":"HUNTR/X","album":"KPop","cover":"https://x/1.jpg"}"#;
        assert_eq!(
            parse(track),
            Some(Event::Track { title: "Golden".into(), artist: "HUNTR/X".into(), album: "KPop".into(), cover: Some("https://x/1.jpg".into()) })
        );
        assert_eq!(parse(r#"@flacora {"event":"progress","value":1.7}"#), Some(Event::Progress(1.0)));
        assert_eq!(parse(r#"@flacora {"event":"saved","path":"C:\\a.flac"}"#), Some(Event::Saved("C:\\a.flac".into())));
        assert_eq!(parse(r#"@flacora {"event":"cover","cover":"http://insecure"}"#), None);
        assert_eq!(parse("Track: HUNTR/X - Golden"), None);
        let links = parse(r#"@flacora {"event":"links","links":{"Tidal":"https://tidal.com/track/1","Bad":"http://x","Yandex Music":"https://example.com/track/1","Unknown":"https://music.yandex.ru/track/1","Pandora":"https://example.org/track/1","Other":"https://www.pandora.com./TR:1"}}"#);
        assert_eq!(links, Some(Event::Links(vec![("Tidal".into(), "https://tidal.com/track/1".into())])));
        let services = parse(r#"@flacora {"event":"services","services":["Qobuz"," Yandex Music ","Pandora","Amazon Music"]}"#);
        assert_eq!(services, Some(Event::Services(vec!["Qobuz".into(), "Amazon Music".into()])));
        let insecure = parse(r#"@flacora {"event":"track","title":"a","cover":"http://x"}"#);
        assert!(matches!(insecure, Some(Event::Track { cover: None, .. })));
    }

    #[test]
    fn saved_and_error_lines_are_understood() {
        assert_eq!(saved_line("Saved out\\A - B.flac (12 bytes)").as_deref(), Some("out\\A - B.flac"));
        assert_eq!(saved_line("Saved out\\B.lrc (1 bytes, SHA-256 ab)").as_deref(), Some("out\\B.lrc"));
        assert_eq!(saved_line("Saved out\\Song (Remix).flac").as_deref(), Some("out\\Song (Remix).flac"));
        assert_eq!(saved_line("Saved   "), None);
        assert_eq!(parse(r#"@flacora {"event":"saved","path":"   "}"#), None);
        assert_eq!(parse(r#"@flacora {"event":"quality","text":""}"#), None);
        assert_eq!(parse(r#"@flacora {"event":"stage","text":""}"#), None);
        assert_eq!(error_line("Error: FLAC CD is not available").as_deref(), Some("FLAC CD is not available"));
    }
}
