// SPDX-License-Identifier: MIT
//! Minimal blocking client for the keyless TVMaze API, used by `titles`.

use serde::Deserialize;
use std::thread;
use std::time::{Duration, Instant};

const API: &str = "https://api.tvmaze.com";
const USER_AGENT: &str = "acervo/0.1 (personal Jellyfin library)";
const REQUEST_GAP: Duration = Duration::from_millis(350);
const MAX_RETRIES: u32 = 3;

#[derive(Deserialize, Clone, Debug)]
pub struct Show {
    pub id: u64,
    pub name: Option<String>,
    pub premiered: Option<String>,
}

#[derive(Deserialize)]
struct SearchResult {
    show: Show,
}

#[derive(Deserialize)]
struct Episode {
    season: Option<u32>,
    number: Option<u32>,
    name: Option<String>,
}

pub trait Client {
    fn search_shows(&mut self, title: &str) -> Vec<Show>;
    fn episodes(&mut self, show_id: u64) -> Vec<(u32, u32, String)>;
}

pub struct HttpClient {
    last_request: Option<Instant>,
    timeout: Duration,
}

impl HttpClient {
    pub fn new(timeout_secs: u64) -> Self {
        Self { last_request: None, timeout: Duration::from_secs(timeout_secs) }
    }

    fn throttle(&mut self) {
        if let Some(last) = self.last_request {
            let elapsed = last.elapsed();
            if elapsed < REQUEST_GAP {
                thread::sleep(REQUEST_GAP - elapsed);
            }
        }
        self.last_request = Some(Instant::now());
    }

    fn get(&mut self, url: &str) -> Option<serde_json::Value> {
        for attempt in 0..MAX_RETRIES {
            self.throttle();
            let result = ureq::get(url).set("User-Agent", USER_AGENT).timeout(self.timeout).call();
            match result {
                Ok(resp) => return resp.into_json().ok(),
                Err(ureq::Error::Status(429, resp)) => {
                    if attempt + 1 < MAX_RETRIES {
                        let retry_after = resp.header("Retry-After").and_then(|v| v.parse::<u64>().ok()).unwrap_or(10).min(30);
                        thread::sleep(Duration::from_secs(retry_after));
                        continue;
                    }
                    return None;
                }
                Err(ureq::Error::Status(404, _)) => return None,
                Err(e) => {
                    if attempt + 1 < MAX_RETRIES {
                        thread::sleep(Duration::from_secs(2 * (attempt as u64 + 1)));
                        continue;
                    }
                    eprintln!("  !! network error contacting TVMaze: {e}");
                    return None;
                }
            }
        }
        None
    }
}

impl Client for HttpClient {
    fn search_shows(&mut self, title: &str) -> Vec<Show> {
        let url = format!("{API}/search/shows?q={}", urlencoding::encode(title));
        let Some(value) = self.get(&url) else { return Vec::new() };
        serde_json::from_value::<Vec<SearchResult>>(value).map(|v| v.into_iter().map(|r| r.show).collect()).unwrap_or_default()
    }

    fn episodes(&mut self, show_id: u64) -> Vec<(u32, u32, String)> {
        let url = format!("{API}/shows/{show_id}/episodes");
        let Some(value) = self.get(&url) else { return Vec::new() };
        let episodes: Vec<Episode> = serde_json::from_value(value).unwrap_or_default();
        episodes
            .into_iter()
            .filter_map(|ep| {
                let name = ep.name.unwrap_or_default().trim().to_string();
                match (ep.season, ep.number) {
                    (Some(s), Some(n)) if !name.is_empty() => Some((s, n, name)),
                    _ => None,
                }
            })
            .collect()
    }
}
