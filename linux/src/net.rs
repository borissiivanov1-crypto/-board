//! HTTP plumbing: the clients, Glances requests, the up/down checks for service buttons,
//! and the LIVE / OFFLINE check (is anything on the server answering?).

use crate::glances::ApiResult;
use crate::layout::Service;
use crate::settings::{host_of, Settings};
use base64::Engine;
use regex::Regex;
use reqwest::blocking::{Client, RequestBuilder, Response};
use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};

/// Requests to the server go straight to it (a proxy would answer for a server that is down, and home
/// servers are on the local network or a VPN). Only logos from the internet use the system proxy.
pub struct Http {
    /// the internet (logos): checks certificates, uses the system proxy
    pub web: Client,
    /// checks certificates (like a browser)
    pub strict: Client,
    /// accepts self-signed certificates (most NAS systems use one out of the box)
    pub insecure: Client,
    /// insecure and doesn't follow redirects (Unraid's login answer is a redirect that sets the cookie)
    pub no_redirect: Client,
}

const UA: &str = concat!("-board/", env!("CARGO_PKG_VERSION"), " (GNOME)");

pub fn http() -> &'static Http {
    static H: OnceLock<Http> = OnceLock::new();
    H.get_or_init(|| Http {
        web: Client::builder().user_agent(UA).build().expect("http client"),
        strict: Client::builder().user_agent(UA).no_proxy().build().expect("http client"),
        insecure: Client::builder().user_agent(UA).no_proxy().danger_accept_invalid_certs(true).build().expect("http client"),
        no_redirect: Client::builder()
            .user_agent(UA)
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("http client"),
    })
}

pub fn basic_auth(user: &str, pass: &str) -> String {
    format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}")))
}

/// Send and read the whole body. Err = no answer (refused, timed out, certificate ...).
pub fn send(req: RequestBuilder) -> Result<(u16, reqwest::header::HeaderMap, String), reqwest::Error> {
    let r: Response = req.send()?;
    let code = r.status().as_u16();
    let headers = r.headers().clone();
    let body = r.text().unwrap_or_default();
    Ok((code, headers, body))
}

/// "name=value; ..." from the cookies a response set.
pub fn cookies_of(headers: &reqwest::header::HeaderMap) -> String {
    headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|v| v.split(';').next().unwrap_or("").trim().to_string())
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn form_body(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v))).collect::<Vec<_>>().join("&")
}

/// GET {glances}/api/{version}/{path}
pub fn glances_request(set: &Settings, path: &str, pass: &str) -> ApiResult {
    let url = format!("{}/api/{}/{}", set.glances_url(), set.api_version, path);
    let mut req = http().strict.get(&url).timeout(Duration::from_secs(5));
    if !set.username.is_empty() {
        req = req.header("Authorization", basic_auth(&set.username, pass));
    }
    let r = req.send().map_err(|e| e.to_string())?;
    let code = r.status().as_u16();
    if code >= 400 {
        return Err(format!("Glances answered HTTP {code}"));
    }
    r.json::<serde_json::Value>().map_err(|e| e.to_string())
}

/// An error that still means something is listening (a self-signed https, or a port that isn't a web
/// server, like SSH).
pub fn error_means_up(msg: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)certificate|secure connection|ssl|tls|bad response|bad server|malformed|could not be parsed|invalid http|invalid status|parse|corrupt message|unexpected message|handshake|alert").unwrap()
    })
    .is_match(msg)
}

fn err_text(e: &reqwest::Error) -> String {
    let mut s = format!("{e}");
    let mut src = std::error::Error::source(e);
    while let Some(x) = src {
        s.push_str(" | ");
        s.push_str(&x.to_string());
        src = x.source();
    }
    s
}

/// Any HTTP answer (even an error page) counts as up. Refused or timed-out connections count as down.
pub fn ping_url(url: &str, timeout: f64) -> bool {
    match http().strict.head(url).timeout(Duration::from_secs_f64(timeout)).send() {
        Ok(_) => true,
        Err(e) => !e.is_timeout() && error_means_up(&err_text(&e)),
    }
}

/// One place on the server: a quick HEAD request; some servers turn HEAD away, so a refusal that isn't a
/// timeout gets one ordinary GET before the place counts as not answering.
pub fn reach_probe(url: &str, timeout: f64) -> bool {
    let t0 = Instant::now();
    if ping_url(url, timeout) {
        return true;
    }
    if t0.elapsed().as_secs_f64() >= timeout - 0.2 {
        return false; // timed out: nothing there
    }
    match http().insecure.get(url).timeout(Duration::from_secs_f64(timeout)).send() {
        Ok(_) => true,
        Err(e) => !e.is_timeout() && error_means_up(&err_text(&e)),
    }
}

/// The places asked for LIVE / OFFLINE: the server address (with its port), the usual web ports, the server
/// system's own address, Glances when Monitoring is on, and service buttons on that host.
pub fn reach_urls(s: &Settings, services: &[Service]) -> Vec<String> {
    let host = s.server_host();
    if host.is_empty() {
        return vec![];
    }
    let mut seen = HashSet::new();
    let mut urls = vec![];
    let mut add = |u: String, urls: &mut Vec<String>| {
        if seen.insert(u.clone()) {
            urls.push(u);
        }
    };
    add(format!("{}/", s.server_url()), &mut urls);
    add(format!("http://{host}/"), &mut urls);
    add(format!("https://{host}/"), &mut urls);
    if crate::config::platform(&s.platform).is_some() {
        add(format!("{}/", s.platform_base(&s.platform)), &mut urls);
    }
    if s.monitoring {
        add(format!("{}/", s.glances_url()), &mut urls);
    }
    for sv in services {
        if host_of(&sv.url) == host && urls.len() < 12 {
            add(sv.url.clone(), &mut urls);
        }
    }
    urls
}

/// Asks the usual places on the server at once. Returns what answered first ("host:port"), or "" for none.
pub fn server_reachable(s: &Settings, services: &[Service], timeout: f64) -> String {
    let urls = reach_urls(s, services);
    if urls.is_empty() {
        return String::new();
    }
    let (tx, rx) = mpsc::channel();
    let n = urls.len();
    for u in urls {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let ok = reach_probe(&u, timeout);
            let _ = tx.send((ok, u));
        });
    }
    drop(tx);
    for _ in 0..n {
        match rx.recv() {
            Ok((true, u)) => {
                let lab = u.split('/').nth(2).unwrap_or(&u).to_string();
                return lab;
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    String::new()
}

/// Up / down for every button (buttons with the same address are only checked once).
pub fn check_services(list: &[Service], timeout: f64) -> HashMap<String, bool> {
    let urls: Vec<String> = list.iter().map(|s| s.url.clone()).collect::<HashSet<_>>().into_iter().collect();
    let results: HashMap<String, bool> = std::thread::scope(|sc| {
        let hs: Vec<_> = urls.iter().map(|u| (u.clone(), sc.spawn(move || ping_url(u, timeout)))).collect();
        hs.into_iter().map(|(u, h)| (u, h.join().unwrap_or(false))).collect()
    });
    list.iter().map(|s| (s.id.clone(), *results.get(&s.url).unwrap_or(&false))).collect()
}

/// GET bytes (logos), checked certificates.
pub fn get_bytes(url: &str, timeout: f64) -> Result<Vec<u8>, String> {
    let r = http().web.get(url).timeout(Duration::from_secs_f64(timeout)).send().map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("HTTP {}", r.status().as_u16()));
    }
    r.bytes().map(|b| b.to_vec()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::clean_settings;
    use serde_json::json;
    use std::net::TcpListener;

    #[test]
    fn error_classification() {
        assert!(error_means_up("invalid peer certificate: UnknownIssuer"));
        assert!(error_means_up("received corrupt message of type InvalidContentType"));
        assert!(error_means_up("invalid HTTP version parsed"));
        assert!(!error_means_up("Connection refused (os error 111)"));
        assert!(!error_means_up("dns error: failed to lookup address"));
    }

    #[test]
    fn reach_urls_cover_the_usual_places() {
        let s = clean_settings(&json!({ "serverHost": "10.0.0.5", "platform": "proxmox" }));
        let u = reach_urls(&s, &[]);
        assert_eq!(u, vec!["https://10.0.0.5:8006/", "http://10.0.0.5/", "https://10.0.0.5/", "http://10.0.0.5:61208/"]);
        let s = clean_settings(&json!({ "serverHost": "10.0.0.5", "monitoring": false }));
        assert_eq!(reach_urls(&s, &[]).len(), 2);
        assert!(reach_urls(&clean_settings(&json!({})), &[]).is_empty());
    }

    fn serve_once(body: &'static str) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            for c in l.incoming().take(4) {
                use std::io::{Read, Write};
                let mut c = c.unwrap();
                let mut buf = [0u8; 2048];
                let _ = c.read(&mut buf);
                let _ = c.write_all(body.as_bytes());
            }
        });
        format!("http://{addr}/")
    }

    #[test]
    fn up_down_checks_against_real_sockets() {
        // an HTTP error page counts as up
        let up = serve_once("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        assert!(ping_url(&up, 3.0));
        // something that isn't a web server (an SSH banner) still counts as up
        let ssh = serve_once("SSH-2.0-OpenSSH_9.6\r\n");
        assert!(ping_url(&ssh, 3.0));
        // nothing listening: down
        let free = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
        let down = format!("http://{free}/");
        assert!(!ping_url(&down, 2.0));
        let svc = |id: &str, url: &str| Service {
            id: id.into(), name: id.into(), port: String::new(), icon: String::new(), logo: String::new(), url: url.into(),
            scheme: "http".into(), addr: String::new(), label: String::new(), on_widget: true, group: None, zima: String::new(),
        };
        let m = check_services(&[svc("a", &up), svc("b", &down), svc("c", &up)], 2.0);
        assert!(m["a"]);
        assert!(!m["b"]);
        assert!(m["c"]);
    }

    #[test]
    fn live_check_finds_the_answering_place() {
        let up = serve_once("HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        let port = up.trim_end_matches('/').rsplit(':').next().unwrap().to_string();
        let s = clean_settings(&json!({ "serverHost": "127.0.0.1", "serverPort": port, "monitoring": false }));
        assert_eq!(server_reachable(&s, &[], 2.0), format!("127.0.0.1:{port}"));
        let free = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let s = clean_settings(&json!({ "serverHost": "127.0.0.1", "serverPort": free.to_string(), "monitoring": false }));
        // port 80 / 443 on this machine might answer; only check that nothing on the chosen port does
        let r = server_reachable(&s, &[], 1.0);
        assert_ne!(r, format!("127.0.0.1:{free}"));
    }

    #[test]
    fn cookies_and_forms() {
        let mut h = reqwest::header::HeaderMap::new();
        h.append(reqwest::header::SET_COOKIE, "a=1; Path=/; HttpOnly".parse().unwrap());
        h.append(reqwest::header::SET_COOKIE, "b=2".parse().unwrap());
        assert_eq!(cookies_of(&h), "a=1; b=2");
        assert_eq!(form_body(&[("username", "root@pam"), ("password", "p&w d")]), "username=root%40pam&password=p%26w%20d");
        assert_eq!(basic_auth("u", "p"), "Basic dTpw");
    }
}
