//! Interner Test gegen die Gegenstelle: Laufzeit/Jitter/Verlust per UDP, dann Download und Upload
//! mit mehreren parallelen TCP-Strömen, währenddessen weiter UDP-Pings (Laufzeit unter Last).

use crate::meter;
use crate::model::{Cancel, Event, Latency, Phase, Results, Sink, TestConfig};
use crate::net::{self, Bind, Hello};
use crate::stats;
use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const WARMUP: f64 = 2.0;

/// Prüft Erreichbarkeit und Code; liefert den Namen der Gegenstelle.
pub async fn hello(addr: SocketAddr, code: &str, bind: &Bind) -> io::Result<String> {
    let s = net::tcp_connect(addr, bind).await?;
    let (_, r) = net::handshake(s, &Hello { v: net::VERSION, code: code.into(), kind: "hello".into(), secs: 0.0 }).await?;
    Ok(r.name)
}

pub async fn run(cfg: &TestConfig, bind: Bind, sink: Sink, cancel: Cancel, cpu: impl FnMut() + Clone) -> io::Result<Results> {
    let addr = net::parse_server(&cfg.server)?;
    let code = cfg.code.trim().to_string();
    let mut res = Results::default();

    sink(Event::Phase { phase: Phase::Ping });
    let udp = net::udp_bound(&bind).await?;
    let idle = ping_udp(&udp, addr, &code, 20.0, 5.0, Phase::Ping, &sink, &cancel).await;
    res.latency = latency_from(&idle);
    if res.latency.loss_pct >= 100.0 {
        return Err(io::Error::new(io::ErrorKind::TimedOut,
            "Keine UDP-Antwort der Gegenstelle – Firewall auf der Gegenstelle prüfen (UDP 5299)."));
    }

    for phase in [Phase::Download, Phase::Upload] {
        if cancel.is_cancelled() {
            break;
        }
        sink(Event::Phase { phase });
        let counter = Arc::new(AtomicU64::new(0));
        let mut streams = Vec::new();
        for _ in 0..cfg.streams.max(1) {
            let s = net::tcp_connect(addr, &bind).await?;
            let kind = if phase == Phase::Download { "down" } else { "up" };
            let hello = Hello { v: net::VERSION, code: code.clone(), kind: kind.into(), secs: cfg.secs + 1.0 };
            let (s, _) = net::handshake(s, &hello).await?;
            let (c, cancel) = (counter.clone(), cancel.clone());
            streams.push(if phase == Phase::Download {
                tokio::spawn(download_stream(s, c, cancel))
            } else {
                tokio::spawn(upload_stream(s, c, cancel))
            });
        }
        let pinger = {
            let (udp_sink, cancel, code) = (sink.clone(), cancel.clone(), code.clone());
            let udp = net::udp_bound(&bind).await?;
            let secs = cfg.secs;
            tokio::spawn(async move { ping_udp(&udp, addr, &code, 4.0, secs, phase, &udp_sink, &cancel).await })
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
    Ok(res)
}

async fn download_stream(mut s: tokio::net::TcpStream, counter: Arc<AtomicU64>, cancel: Cancel) {
    let mut buf = vec![0u8; net::BUF];
    while !cancel.is_cancelled() {
        match s.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                counter.fetch_add(n as u64, Ordering::Relaxed);
            }
        }
    }
}

async fn upload_stream(s: tokio::net::TcpStream, counter: Arc<AtomicU64>, cancel: Cancel) {
    let (mut rd, mut wr) = s.into_split();
    let block = net::random_block();
    let c2 = cancel.clone();
    let writer = tokio::spawn(async move {
        while !c2.is_cancelled() {
            if wr.write_all(&block).await.is_err() {
                break;
            }
        }
    });
    // Die Gegenstelle meldet die empfangenen Bytes kumulativ zurück.
    let mut last = 0u64;
    let mut b = [0u8; 8];
    while rd.read_exact(&mut b).await.is_ok() {
        let total = u64::from_be_bytes(b);
        counter.fetch_add(total.saturating_sub(last), Ordering::Relaxed);
        last = last.max(total);
        if cancel.is_cancelled() {
            break;
        }
    }
    writer.abort();
}

#[derive(Debug, Default)]
pub struct PingRun {
    pub sent: u32,
    /// (Sekunde seit Beginn, Laufzeit ms)
    pub rtts: Vec<(f64, f64)>,
}

/// Sendet `rate` UDP-Pings pro Sekunde über `secs` Sekunden und wartet danach 1 s auf Nachzügler.
#[allow(clippy::too_many_arguments)]
pub async fn ping_udp(
    udp: &tokio::net::UdpSocket,
    addr: SocketAddr,
    code: &str,
    rate: f64,
    secs: f64,
    phase: Phase,
    sink: &Sink,
    cancel: &Cancel,
) -> PingRun {
    let start = Instant::now();
    let mut sent_at: HashMap<u32, Instant> = HashMap::new();
    let mut run = PingRun::default();
    let mut tick = tokio::time::interval(Duration::from_secs_f64(1.0 / rate));
    let send_end = start + Duration::from_secs_f64(secs);
    let end = tokio::time::Instant::from_std(send_end + Duration::from_secs(1));
    let mut pkt = [0u8; 64];
    pkt[..4].copy_from_slice(net::UDP_MAGIC);
    let code4: Vec<u8> = code.bytes().chain(std::iter::repeat(b'0')).take(4).collect();
    pkt[4..8].copy_from_slice(&code4);
    let mut buf = [0u8; 1500];
    loop {
        tokio::select! {
            _ = tick.tick(), if Instant::now() < send_end && !cancel.is_cancelled() => {
                pkt[8..12].copy_from_slice(&run.sent.to_be_bytes());
                sent_at.insert(run.sent, Instant::now());
                run.sent += 1;
                let _ = udp.send_to(&pkt, addr).await;
            }
            r = udp.recv_from(&mut buf) => {
                if let Ok((n, _)) = r {
                    if n >= 12 && &buf[..4] == net::UDP_MAGIC {
                        let seq = u32::from_be_bytes(buf[8..12].try_into().unwrap());
                        if let Some(t0) = sent_at.remove(&seq) {
                            let ms = t0.elapsed().as_secs_f64() * 1000.0;
                            let t = start.elapsed().as_secs_f64();
                            run.rtts.push((meter::round2(t), (ms * 1000.0).round() / 1000.0));
                            sink(Event::Ping { phase, t, ms });
                        }
                    }
                }
            }
            _ = tokio::time::sleep_until(end) => break,
        }
        if cancel.is_cancelled() {
            break;
        }
    }
    run
}

pub fn latency_from(run: &PingRun) -> Latency {
    let v: Vec<f64> = run.rtts.iter().map(|x| x.1).collect();
    let r = meter::round2;
    Latency {
        min_ms: r(stats::min(&v)),
        median_ms: r(stats::median(&v)),
        p95_ms: r(stats::percentile(&v, 95.0)),
        max_ms: r(stats::max(&v)),
        jitter_ms: r(stats::jitter(&v)),
        loss_pct: if run.sent == 0 { 0.0 } else { r(100.0 * (run.sent as f64 - v.len() as f64) / run.sent as f64) },
        samples: run.rtts.clone(),
    }
}
