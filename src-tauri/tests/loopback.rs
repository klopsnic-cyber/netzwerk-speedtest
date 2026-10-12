//! Gegenstelle und Client auf demselben Rechner: Protokoll, Code-Prüfung, Messablauf.
use speedtest_lib::model::{Cancel, Scope, Sink, TestConfig};
use speedtest_lib::{lan, net, server};
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn intern_ueber_loopback() {
    let port = 52999;
    let _srv = server::start(port, "4711".into(), Arc::new(|_| {})).await.unwrap();
    let addr = format!("127.0.0.1:{port}").parse().unwrap();
    let bind = net::Bind::default();

    assert!(lan::hello(addr, "0000", &bind).await.is_err(), "falscher Code muss abgewiesen werden");
    assert!(!lan::hello(addr, "4711", &bind).await.unwrap().is_empty());

    let cfg = TestConfig { scope: Scope::Internal, iface: String::new(), server: addr.to_string(), code: "4711".into(), streams: 4, secs: 4.0 };
    let sink: Sink = Arc::new(|_| {});
    let r = lan::run(&cfg, bind, sink, Cancel::default(), || {}).await.unwrap();
    println!("down {:?} Mbit/s, up {:?} Mbit/s, ping {:?} ms, loss {}%", r.download.median_mbps, r.upload.median_mbps, r.latency.median_ms, r.latency.loss_pct);
    assert!(r.download.median_mbps > 500.0, "Loopback-Download zu langsam: {}", r.download.median_mbps);
    assert!(r.upload.median_mbps > 500.0, "Loopback-Upload zu langsam: {}", r.upload.median_mbps);
    assert_eq!(r.latency.loss_pct, 0.0);
    assert!(r.latency.median_ms < 5.0);
    assert!(r.download.samples.len() >= 14);
}
