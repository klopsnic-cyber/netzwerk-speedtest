//! Externer Test gegen speed.cloudflare.com: Laufzeit in Ruhe, Download und Upload mit mehreren
//! parallelen HTTP/1.1-Verbindungen, dabei weiter Laufzeit messen (Bufferbloat).

use crate::meter;
use crate::model::{Cancel, Event, Phase, Results, Sink, TestConfig};
use crate::net::{self, Bind};
use crate::lan::{PingRun, latency_from};
use crate::stats;
use futures_util::StreamExt;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use std::time::{Duration, Instant};

const BASE: &str = "https://speed.cloudflare.com";
pub const WARMUP: f64 = 2.0;

#[derive(Debug, Default, Clone)]
pub struct Meta {
    pub ip: Option<String>,
    pub provider: Option<String>,
    pub location: Option<String>,
}

pub fn client(bind: &Bind, iface: &str) -> io::Result<reqwest::Client> {
    let mut b = reqwest::Client::builder()
        .user_agent(concat!("Netzwerk-Speedtest/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .pool_max_idle_per_host(16);
    if let Some(ip) = bind.ip {
        b = b.local_address(std::net::IpAddr::V4(ip));
    }
    #[cfg(target_os = "macos")]
    if !iface.is_empty() {
        b = b.interface(iface);
    }
    let _ = iface;
    b.build().map_err(io::Error::other)
}

fn err(e: reqwest::Error) -> io::Error {
    let msg = if e.is_connect() || e.is_timeout() {
        "Cloudflare nicht erreichbar – besteht eine Internetverbindung über die gewählte Schnittstelle?".to_string()
    } else {
        format!("Fehler bei der Internetmessung: {e}")
    };
    io::Error::other(msg)
}

/// Öffentliche IP (cf-meta-ip) und Rechenzentrum (Kürzel am Ende von cf-ray, z. B. "…-FRA").
pub async fn meta(c: &reqwest::Client) -> Meta {
    let Ok(r) = c.get(format!("{BASE}/__down?bytes=0")).timeout(Duration::from_secs(5)).send().await else {
        return Meta::default();
    };
    let h = |k: &str| r.headers().get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
    let colo = h("cf-ray").and_then(|ray| ray.rsplit('-').next().map(str::to_uppercase));
    let location = colo.map(|c| match colo_city(&c) {
        Some(city) => format!("Cloudflare {city} ({c})"),
        None => format!("Cloudflare {c}"),
    });
    Meta { ip: h("cf-meta-ip"), provider: None, location }
}

fn colo_city(c: &str) -> Option<&'static str> {
    Some(match c {
        "FRA" => "Frankfurt",
        "DUS" => "Düsseldorf",
        "HAM" => "Hamburg",
        "MUC" => "München",
        "BER" | "TXL" => "Berlin",
        "STR" => "Stuttgart",
        "AMS" => "Amsterdam",
        "VIE" => "Wien",
        "ZRH" => "Zürich",
        "BRU" => "Brüssel",
        "CPH" => "Kopenhagen",
        "PRG" => "Prag",
        "WAW" => "Warschau",
        "LUX" => "Luxemburg",
        "CDG" => "Paris",
        "LHR" => "London",
        _ => return None,
    })
}

/// Laufzeit = Dauer des TCP-Verbindungsaufbaus zum Messserver (reine Netzlaufzeit, ohne Bearbeitungszeit
/// des Servers; funktioniert auch unter Last, weil das SYN durch dieselben Warteschlangen muss).
async fn rtt(addr: SocketAddr, bind: &Bind) -> Option<f64> {
    let t0 = Instant::now();
    let s = tokio::time::timeout(Duration::from_secs(3), net::tcp_connect(addr, bind)).await.ok()?.ok()?;
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    drop(s);
    Some(ms)
}

async fn ping_tcp(addr: SocketAddr, bind: Bind, rate: f64, secs: f64, phase: Phase, sink: &Sink, cancel: &Cancel) -> PingRun {
    let start = Instant::now();
    let mut run = PingRun::default();
    let mut tick = tokio::time::interval(Duration::from_secs_f64(1.0 / rate));
    while start.elapsed().as_secs_f64() < secs && !cancel.is_cancelled() {
        tick.tick().await;
        run.sent += 1;
        if let Some(ms) = rtt(addr, &bind).await {
            let t = start.elapsed().as_secs_f64();
            run.rtts.push((meter::round2(t), (ms * 1000.0).round() / 1000.0));
            sink(Event::Ping { phase, t, ms });
        }
    }
    run
}

pub async fn run(cfg: &TestConfig, bind: Bind, sink: Sink, cancel: Cancel, cpu: impl FnMut() + Clone) -> io::Result<(Results, Meta)> {
    let c = client(&bind, &cfg.iface)?;
    // Erste Anfrage baut die Verbindung (TLS) auf und zählt nicht.
    c.get(format!("{BASE}/__down?bytes=0")).send().await.map_err(err)?.bytes().await.map_err(err)?;
    let meta = meta(&c).await;
    let addr = tokio::net::lookup_host("speed.cloudflare.com:443")
        .await?
        .find(|a| a.is_ipv4())
        .ok_or_else(|| io::Error::other("speed.cloudflare.com nicht auflösbar (DNS)"))?;
    let mut res = Results::default();
    let status = Arc::new(AtomicU16::new(0)); // letzter HTTP-Fehlercode der Datenströme

    sink(Event::Phase { phase: Phase::Ping });
    let idle = ping_tcp(addr, bind, 5.0, 4.0, Phase::Ping, &sink, &cancel).await;
    res.latency = latency_from(&idle);

    for phase in [Phase::Download, Phase::Upload] {
        if cancel.is_cancelled() {
            break;
        }
        sink(Event::Phase { phase });
        let counter = Arc::new(AtomicU64::new(0));
        let deadline = Instant::now() + Duration::from_secs_f64(cfg.secs + 0.5);
        let streams: Vec<_> = (0..cfg.streams.max(1))
            .map(|_| {
                let (c, n, cancel, st) = (c.clone(), counter.clone(), cancel.clone(), status.clone());
                if phase == Phase::Download {
                    tokio::spawn(download(c, n, cancel, deadline, st))
                } else {
                    tokio::spawn(upload(c, n, cancel, deadline, st))
                }
            })
            .collect();
        // Laufzeit unter Last über eine eigene Verbindung
        let pinger = {
            let (s, cancel, secs) = (sink.clone(), cancel.clone(), cfg.secs);
            tokio::spawn(async move { ping_tcp(addr, bind, 4.0, secs, phase, &s, &cancel).await })
        };
        let tp = meter::sample(phase, cfg.secs, WARMUP, counter, &sink, &cancel, cpu.clone()).await;
        for s in &streams {
            s.abort();
        }
        let loaded = pinger.await.unwrap_or_default();
        let rtts: Vec<f64> = loaded.rtts.iter().map(|x| x.1).collect();
        let loaded_ms = (!rtts.is_empty()).then(|| meter::round2(stats::median(&rtts)));
        if phase == Phase::Download {
            res.download = tp;
            res.loaded_down_ms = loaded_ms;
        } else {
            res.upload = tp;
            res.loaded_up_ms = loaded_ms;
        }
    }
    if (res.download.avg_mbps == 0.0 || res.upload.avg_mbps == 0.0) && !cancel.is_cancelled() {
        return Err(io::Error::other(match status.load(Ordering::Relaxed) {
            429 => "Cloudflare begrenzt gerade die Anzahl der Tests von diesem Anschluss. Bitte in einigen Minuten erneut messen.".to_string(),
            0 => "Keine Daten von Cloudflare empfangen – Firewall/Proxy prüfen.".to_string(),
            code => format!("Cloudflare lehnt die Messung ab (HTTP {code}) – Firewall/Proxy prüfen oder später erneut versuchen."),
        }));
    }
    Ok((res, meta))
}

async fn download(c: reqwest::Client, n: Arc<AtomicU64>, cancel: Cancel, deadline: Instant, status: Arc<AtomicU16>) {
    while Instant::now() < deadline && !cancel.is_cancelled() {
        // Cloudflare liefert höchstens ca. 25-99 MB je Anfrage (100 MB -> 403)
        let mut r = match c.get(format!("{BASE}/__down?bytes=25000000")).send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                status.store(r.status().as_u16(), Ordering::Relaxed);
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            }
        };
        while let Ok(Some(chunk)) = r.chunk().await {
            n.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            if Instant::now() >= deadline || cancel.is_cancelled() {
                return;
            }
        }
    }
}

async fn upload(c: reqwest::Client, n: Arc<AtomicU64>, cancel: Cancel, deadline: Instant, status: Arc<AtomicU16>) {
    let block = net::random_block().slice(..64 * 1024);
    while Instant::now() < deadline && !cancel.is_cancelled() {
        // 25 MB je Anfrage; gezählt wird, was der Sender abholt (abzüglich Puffer: kleiner Fehler bei
        // hohen Raten, durch die verworfene Anlaufphase und den Median weitgehend ausgeglichen)
        let (n2, cancel2, block2) = (n.clone(), cancel.clone(), block.clone());
        let body = futures_util::stream::iter(0..400).map(move |_| {
            if Instant::now() >= deadline || cancel2.is_cancelled() {
                return Err(io::Error::other("Ende"));
            }
            n2.fetch_add(block2.len() as u64, Ordering::Relaxed);
            Ok::<_, io::Error>(block2.clone())
        });
        let r = c
            .post(format!("{BASE}/__up"))
            .header("content-type", "application/octet-stream")
            .header("content-length", (400 * block.len()).to_string())
            .body(reqwest::Body::wrap_stream(body))
            .send()
            .await;
        match r {
            Ok(r) if !r.status().is_success() => {
                status.store(r.status().as_u16(), Ordering::Relaxed);
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            _ => {}
        }
    }
}
