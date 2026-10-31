//! One generic JSON interface behind [`TimePlatform`]; endpoints and payloads in `doc/architecture_agent_wiki.md`.
//! Every call carries `Authorization: Bearer <key>`, and the key never appears in an error: everything that comes back is redacted first.

use crate::error::{IterError, Result};
use crate::reporting::{Worklog, WorklogEntry, fmt_date};
use crate::secret::Secret;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const BODY_LIMIT: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EntryResult {
    pub day: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub message: String,
}

impl EntryResult {
    pub fn accepted(&self) -> bool {
        self.status.is_empty() || self.status == "ok"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PlatformItem {
    pub id: String,
    #[serde(default)]
    pub name: String,
}

impl PlatformItem {
    #[cfg(test)]
    pub fn new(id: &str, name: &str) -> Self {
        PlatformItem {
            id: id.into(),
            name: name.into(),
        }
    }

    pub fn is(&self, text: &str) -> bool {
        let text = text.trim();
        self.id == text || self.name.eq_ignore_ascii_case(text)
    }
}

#[derive(Deserialize)]
struct Entries {
    #[serde(default)]
    entries: Vec<WireEntry>,
    #[serde(default)]
    has_more: bool,
}

#[derive(Deserialize)]
struct WireEntry {
    date: String,
    #[serde(default)]
    time: f64,
    #[serde(default)]
    note: String,
    #[serde(default)]
    project: String,
    #[serde(default)]
    activity: String,
}

impl From<WireEntry> for WorklogEntry {
    fn from(e: WireEntry) -> Self {
        WorklogEntry {
            project: e.project,
            activity: e.activity,
            day: e.date,
            time: e.time / 60.0,
            note: e.note,
        }
    }
}

#[derive(Serialize)]
struct Outgoing<'a> {
    entries: Vec<OutgoingEntry<'a>>,
}

#[derive(Serialize)]
struct OutgoingEntry<'a> {
    project: &'a str,
    activity: &'a str,
    date: &'a str,
    time: i64,
    note: &'a str,
}

const MAX_PAGES: u32 = 50;

#[derive(Deserialize)]
struct Results {
    #[serde(default)]
    results: Vec<EntryResult>,
}

#[derive(Deserialize)]
struct Projects {
    #[serde(default)]
    projects: Vec<PlatformItem>,
}

#[derive(Deserialize)]
struct Activities {
    #[serde(default)]
    activities: Vec<PlatformItem>,
}

pub trait TimePlatform {
    /// Sends `worklog`: the platform's verdict on each entry. An `Err` is the request as a whole failing; a refused entry is a result.
    fn push(&self, worklog: &Worklog) -> Result<Vec<EntryResult>>;
    fn fetch(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<WorklogEntry>>;
    fn projects(&self) -> Result<Vec<PlatformItem>>;
    fn activities(&self, project_id: &str) -> Result<Vec<PlatformItem>>;
}

fn path_segment(text: &str) -> String {
    use std::fmt::Write;
    text.bytes().fold(String::new(), |mut out, b| {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
        out
    })
}

pub struct JsonWebhook {
    url: String,
    key: Secret,
    agent: ureq::Agent,
}

impl JsonWebhook {
    pub fn new(url: &str, key: Secret) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .into();
        JsonWebhook {
            url: url.trim_end_matches('/').to_string(),
            key,
            agent,
        }
    }

    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(seconds)))
            .build()
            .into();
        self
    }

    fn bearer(&self) -> String {
        format!("Bearer {}", self.key.expose())
    }

    fn fail(&self, what: &str) -> IterError {
        IterError::Platform(self.key.redact(what))
    }

    fn finish(
        &self,
        response: std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    ) -> Result<String> {
        Ok(self.read(response, false)?.unwrap_or_default())
    }

    /// The body of a 2xx response, `None` when the endpoint is absent (404, 405, 501), or the error a refusal makes.
    fn read(
        &self,
        response: std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>,
        missing_ok: bool,
    ) -> Result<Option<String>> {
        let mut response = response.map_err(|e| self.fail(&e.to_string()))?;
        let status = response.status();
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| self.fail(&e.to_string()))?;
        if status.is_success() {
            return Ok(Some(body));
        }
        if missing_ok && matches!(status.as_u16(), 404 | 405 | 501) {
            return Ok(None);
        }
        let shown: String = body.trim().chars().take(BODY_LIMIT).collect();
        Err(self.fail(&format!("HTTP {}: {shown}", status.as_u16())))
    }

    fn get(&self, url: &str, query: &[(&str, String)]) -> Result<String> {
        Ok(self.get_or_missing(url, query, false)?.unwrap_or_default())
    }

    fn get_or_missing(
        &self,
        url: &str,
        query: &[(&str, String)],
        missing_ok: bool,
    ) -> Result<Option<String>> {
        let mut request = self.agent.get(url).header("Authorization", self.bearer());
        for (k, v) in query {
            request = request.query(k, v);
        }
        self.read(request.call(), missing_ok)
    }

    fn parse<T: serde::de::DeserializeOwned>(&self, body: &str) -> Result<T> {
        serde_json::from_str(body).map_err(|e| self.fail(&format!("unexpected response: {e}")))
    }
}

impl TimePlatform for JsonWebhook {
    fn push(&self, worklog: &Worklog) -> Result<Vec<EntryResult>> {
        let body = serde_json::to_string(&Outgoing {
            entries: worklog
                .entries
                .iter()
                .map(|e| OutgoingEntry {
                    project: &e.project,
                    activity: &e.activity,
                    date: &e.day,
                    time: (e.time * 60.0).round() as i64,
                    note: &e.note,
                })
                .collect(),
        })
        .map_err(|e| self.fail(&format!("could not encode the worklog: {e}")))?;
        let response = self
            .agent
            .post(format!("{}/time-entries", self.url))
            .header("Authorization", self.bearer())
            .content_type("application/json")
            .send(body);
        let reply = self.finish(response)?;
        // an empty reply is "all accepted"; any other unrecognised reply (a proxy's or captive portal's HTML) is not, so nothing is recorded as pushed on its strength
        let mut results = match reply.trim() {
            "" => Vec::new(),
            text => self.parse::<Results>(text)?.results,
        };
        for entry in &worklog.entries {
            if !results.iter().any(|r| r.day == entry.day) {
                results.push(EntryResult {
                    day: entry.day.clone(),
                    status: "ok".into(),
                    message: String::new(),
                });
            }
        }
        Ok(results)
    }

    fn fetch(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<WorklogEntry>> {
        let mut out = Vec::new();
        for page in 1..=MAX_PAGES {
            let mut query = vec![("from", fmt_date(from)), ("to", fmt_date(to))];
            if page > 1 {
                query.push(("page", page.to_string()));
            }
            let body = self.get(&format!("{}/time-entries", self.url), &query)?;
            let got = self.parse::<Entries>(&body)?;
            out.extend(got.entries.into_iter().map(WorklogEntry::from));
            if !got.has_more {
                return Ok(out);
            }
        }
        // Stopping here quietly would hide the days beyond it from the
        // conflict check, and they would be pushed twice.
        Err(self.fail(&format!(
            "more than {MAX_PAGES} pages of entries between {from} and {to}: narrow the period"
        )))
    }

    fn projects(&self) -> Result<Vec<PlatformItem>> {
        // No such endpoint: the platform does not list its projects.
        match self.get_or_missing(&format!("{}/projects", self.url), &[], true)? {
            Some(body) => Ok(self.parse::<Projects>(&body)?.projects),
            None => Ok(Vec::new()),
        }
    }

    fn activities(&self, project_id: &str) -> Result<Vec<PlatformItem>> {
        let url = format!(
            "{}/projects/{}/activities",
            self.url,
            path_segment(project_id)
        );
        match self.get_or_missing(&url, &[], true)? {
            Some(body) => Ok(self.parse::<Activities>(&body)?.activities),
            None => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
pub mod fake {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct FakePlatform {
        pub pushed: RefCell<Vec<Worklog>>,
        pub existing: Vec<WorklogEntry>,
        pub refuse: Option<String>,
        pub conflicts: Vec<String>,
        pub projects: Vec<PlatformItem>,
        pub activities: Vec<PlatformItem>,
    }

    impl TimePlatform for FakePlatform {
        fn push(&self, worklog: &Worklog) -> Result<Vec<EntryResult>> {
            if let Some(why) = &self.refuse {
                return Err(IterError::Platform(why.clone()));
            }
            self.pushed.borrow_mut().push(worklog.clone());
            Ok(worklog
                .entries
                .iter()
                .map(|e| EntryResult {
                    day: e.day.clone(),
                    status: if self.conflicts.contains(&e.day) {
                        "conflict"
                    } else {
                        "ok"
                    }
                    .into(),
                    message: String::new(),
                })
                .collect())
        }
        fn fetch(&self, _: NaiveDate, _: NaiveDate) -> Result<Vec<WorklogEntry>> {
            Ok(self.existing.clone())
        }
        fn projects(&self) -> Result<Vec<PlatformItem>> {
            Ok(self.projects.clone())
        }
        fn activities(&self, _: &str) -> Result<Vec<PlatformItem>> {
            Ok(self.activities.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    const KEY: &str = "s3cr3t-key-123";

    fn stub_all(bodies: Vec<String>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        serve("200 OK", bodies)
    }

    fn stub(status: &str, body: &str) -> (String, std::thread::JoinHandle<String>) {
        let (url, handle) = serve(status, vec![body.to_string()]);
        let one = std::thread::spawn(move || handle.join().unwrap().remove(0));
        (url, one)
    }

    fn serve(status: &str, bodies: Vec<String>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/log", listener.local_addr().unwrap());
        let status = status.to_string();
        let handle = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for body in bodies {
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let (mut stream, _) = listener.accept().unwrap();
                let mut raw = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    let n = stream.read(&mut buf).unwrap();
                    raw.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&raw).to_string();
                    if let Some(split) = text.find("\r\n\r\n") {
                        let length = text[..split]
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if raw.len() >= split + 4 + length {
                            break;
                        }
                    }
                    if n == 0 {
                        break;
                    }
                }
                stream.write_all(reply.as_bytes()).unwrap();
                seen.push(String::from_utf8_lossy(&raw).to_string());
            }
            seen
        });
        (url, handle)
    }

    fn log() -> Worklog {
        Worklog {
            entries: vec![WorklogEntry {
                project: "acme".into(),
                activity: "dev".into(),
                day: "2026-09-04".into(),
                time: 1.5,
                note: "- Worked on A".into(),
            }],
            open: false,
        }
    }

    fn webhook(url: &str) -> JsonWebhook {
        JsonWebhook::new(url, Secret::new(KEY).unwrap())
    }

    #[test]
    fn a_push_answered_with_a_page_is_an_error_not_an_acceptance() {
        let (url, request) = stub("200 OK", "<html>please log in</html>");
        let err = webhook(&url).push(&log()).unwrap_err();
        request.join().unwrap();
        assert!(err.to_string().contains("unexpected response"), "{err}");
    }

    #[test]
    fn a_push_answered_with_nothing_is_an_acceptance() {
        let (url, request) = stub("200 OK", "");
        let results = webhook(&url).push(&log()).unwrap();
        request.join().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "ok");
    }

    /// More pages than the cap must not read as "no more entries".
    #[test]
    fn a_fetch_that_hits_the_page_cap_is_an_error() {
        let page = r#"{"page":1,"total":9,"entries":[],"has_more":true,"page_size":1}"#;
        let (url, handle) = stub_all(vec![page.to_string(); MAX_PAGES as usize]);
        let day = NaiveDate::from_ymd_opt(2026, 9, 4).unwrap();
        let err = webhook(&url).fetch(day, day).unwrap_err();
        handle.join().unwrap();
        assert!(err.to_string().contains("pages"), "{err}");
    }

    #[test]
    fn a_push_posts_json_with_a_bearer_key() {
        let (url, request) = stub("200 OK", "{}");
        webhook(&url).push(&log()).unwrap();
        let raw = request.join().unwrap();
        assert!(raw.starts_with("POST /log/time-entries "), "{raw}");
        assert!(
            raw.contains(&format!("authorization: Bearer {KEY}"))
                || raw.contains(&format!("Authorization: Bearer {KEY}"))
        );
        assert!(
            raw.to_ascii_lowercase()
                .contains("content-type: application/json")
        );
        assert!(raw.contains(r#""date":"2026-09-04""#) && raw.contains(r#""time":90"#));
    }

    #[test]
    fn a_refusal_shows_its_body_without_the_key() {
        let (url, _) = stub("401 Unauthorized", &format!("bad token {KEY}"));
        let err = webhook(&url).push(&log()).unwrap_err().to_string();
        assert!(err.contains("HTTP 401"), "{err}");
        assert!(err.contains("bad token ***"), "{err}");
        assert!(!err.contains(KEY));
    }

    #[test]
    fn a_server_error_is_an_error_too() {
        let (url, _) = stub("500 Internal Server Error", "boom");
        let err = webhook(&url).push(&log()).unwrap_err().to_string();
        assert!(err.contains("HTTP 500") && err.contains("boom"), "{err}");
    }

    #[test]
    fn an_unreachable_host_is_an_error_without_the_key() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let err = webhook(&format!("http://127.0.0.1:{port}/log"))
            .push(&log())
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("time platform:"), "{err}");
        assert!(!err.contains(KEY));
    }

    #[test]
    fn fetch_sends_the_period_and_reads_minutes_and_ids() {
        let body = r#"{"page":1,"total":1,"entries":[{"id":"e1","date":"2026-09-04","note":" x","time":90,"project":"p1","activity":"a1","status":"historical"}],"has_more":false,"page_size":100}"#;
        let (url, request) = stub("200 OK", body);
        let day = NaiveDate::from_ymd_opt(2026, 9, 4).unwrap();
        let got = webhook(&url).fetch(day, day).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(
            (got[0].time, got[0].day.as_str(), got[0].project.as_str()),
            (1.5, "2026-09-04", "p1")
        );
        let raw = request.join().unwrap();
        assert!(
            raw.starts_with("GET /log/time-entries?from=2026-09-04&to=2026-09-04 "),
            "{raw}"
        );
    }

    #[test]
    fn fetch_follows_the_pages_while_there_are_more() {
        let page = |more: bool, date: &str| {
            format!(
                r#"{{"entries":[{{"date":"{date}","time":60,"note":"","project":"p","activity":"a"}}],"has_more":{more}}}"#
            )
        };
        let (url, requests) = stub_all(vec![page(true, "2026-09-03"), page(false, "2026-09-04")]);
        let got = webhook(&url)
            .fetch(
                NaiveDate::from_ymd_opt(2026, 9, 3).unwrap(),
                NaiveDate::from_ymd_opt(2026, 9, 4).unwrap(),
            )
            .unwrap();
        assert_eq!(
            got.iter().map(|e| e.day.as_str()).collect::<Vec<_>>(),
            ["2026-09-03", "2026-09-04"]
        );
        let raw = requests.join().unwrap();
        assert!(raw[0].starts_with("GET /log/time-entries?from=2026-09-03&to=2026-09-04 "));
        assert!(raw[1].contains("&page=2 "), "{}", raw[1]);
    }

    #[test]
    fn projects_and_a_projects_activities_are_listed_with_their_ids() {
        let (url, request) = stub("200 OK", r#"{"projects":[{"id":"p1","name":"Acme"}]}"#);
        assert_eq!(
            webhook(&url).projects().unwrap(),
            vec![PlatformItem::new("p1", "Acme")]
        );
        assert!(request.join().unwrap().starts_with("GET /log/projects "));
        let (url, request) = stub("200 OK", r#"{"activities":[{"id":"a1","name":"Dev"}]}"#);
        assert_eq!(
            webhook(&url).activities("p 1/x").unwrap(),
            vec![PlatformItem::new("a1", "Dev")]
        );
        assert!(
            request
                .join()
                .unwrap()
                .starts_with("GET /log/projects/p%201%2Fx/activities ")
        );
    }

    #[test]
    fn a_name_matches_without_regard_to_case_and_an_id_exactly() {
        let n = PlatformItem::new("p1", "Next-gen Energy");
        assert!(n.is("p1") && n.is(" next-GEN energy ") && !n.is("P1x"));
    }

    #[test]
    fn a_conflicting_entry_is_a_result_not_a_failure() {
        let body = r#"{"results":[{"day":"2026-09-04","status":"conflict","message":"already has an entry"}]}"#;
        let (url, _) = stub("200 OK", body);
        let results = webhook(&url).push(&log()).unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].accepted());
        assert_eq!(results[0].message, "already has an entry");
    }

    #[test]
    fn entries_a_response_does_not_mention_were_accepted() {
        let (url, _) = stub("200 OK", "{}");
        let results = webhook(&url).push(&log()).unwrap();
        assert!(results.len() == 1 && results[0].accepted());
    }

    #[test]
    fn the_key_is_not_in_debug_output() {
        let hook = webhook("http://localhost/log");
        assert!(!format!("{:?}", hook.key).contains(KEY));
    }
}
