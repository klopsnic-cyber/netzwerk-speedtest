use speedtest_lib::model::{Cancel, Scope, Sink, TestConfig};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let name = std::env::args().nth(1).unwrap_or_default();
    let i = speedtest_lib::iface::find(&name).expect("Schnittstelle nicht gefunden");
    let bind = speedtest_lib::iface::bind_for(&i);
    let cfg = TestConfig { scope: Scope::External, iface: i.name.clone(), server: String::new(), code: String::new(), streams: 6, secs: 10.0 };
    let sink: Sink = Arc::new(|_| {});
    let (r, m) = speedtest_lib::cloud::run(&cfg, bind, sink, Cancel::default(), || {}).await.unwrap();
    println!("UP {:?}\nAVG {}", r.upload.samples.iter().map(|s| s.1 as i64).collect::<Vec<_>>(), r.upload.avg_mbps);
    println!("{} {:?}: down {} / max {} Mbit/s, up {} / max {} Mbit/s, ping {} ms, jitter {} ms, unter Last {:?}/{:?} ms, {:?}",
        i.name, i.ipv4, r.download.median_mbps, r.download.max_mbps, r.upload.median_mbps, r.upload.max_mbps,
        r.latency.median_ms, r.latency.jitter_ms, r.loaded_down_ms, r.loaded_up_ms, m.location);
}
