//! Gemeinsamer Durchsatz-Messer: Die Datenströme zählen Bytes in einen Zähler, dieser Messer
//! liest ihn alle 250 ms und meldet die Rate über das jeweils letzte Sekundenfenster (Datenströme
//! liefern stoßweise, besonders beim Upload - 250-ms-Werte allein springen zwischen 0 und Spitze).
//! Median/Max/Mittel zählen nur die Zeit NACH der Anlaufphase (TCP-Slow-Start verfälscht sonst).

use crate::model::{Cancel, Event, Phase, Sink, Throughput};
use crate::stats;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const TICK: Duration = Duration::from_millis(250);
const WINDOW: f64 = 1.0;

pub async fn sample(
    phase: Phase,
    secs: f64,
    warmup: f64,
    counter: Arc<AtomicU64>,
    sink: &Sink,
    cancel: &Cancel,
    mut on_tick: impl FnMut(),
) -> Throughput {
    let start = Instant::now();
    let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + TICK, TICK);
    let mut last_bytes = 0u64;
    let mut last_t = 0.0f64;
    let mut hist: VecDeque<(f64, u64)> = VecDeque::from([(0.0, 0)]);
    let mut warm_bytes = None::<(u64, f64)>;
    let mut samples = Vec::new();
    let mut steady = Vec::new();

    loop {
        tick.tick().await;
        let t = start.elapsed().as_secs_f64();
        let bytes = counter.load(Ordering::Relaxed);
        hist.push_back((t, bytes));
        while hist.len() > 2 && t - hist[1].0 >= WINDOW {
            hist.pop_front();
        }
        let (t0, b0) = hist[0];
        let rate = stats::mbps(bytes - b0, t - t0);
        samples.push((round2(t), round2(rate)));
        sink(Event::Throughput { phase, t, mbps: rate });
        on_tick();
        if t >= warmup {
            warm_bytes.get_or_insert((last_bytes, last_t));
            steady.push(rate);
        }
        last_bytes = bytes;
        last_t = t;
        if t >= secs || cancel.is_cancelled() {
            break;
        }
    }

    let avg = match warm_bytes {
        Some((b0, t0)) => stats::mbps(last_bytes - b0, last_t - t0),
        None => stats::mbps(last_bytes, last_t),
    };
    Throughput {
        median_mbps: round2(stats::median(&steady)),
        max_mbps: round2(stats::max(&steady)),
        avg_mbps: round2(avg),
        samples,
    }
}

pub fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}
