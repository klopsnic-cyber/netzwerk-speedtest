//! Beispielbericht mit erfundenen Werten (zum Prüfen des Layouts): cargo run --example pdf_demo -- out.pdf
use speedtest_lib::model::*;
use speedtest_lib::{report, verdict};

fn series(base: f64, wobble: f64, n: usize) -> Vec<(f64, f64)> {
    (1..=n).map(|i| { let t = i as f64 * 0.25; (t, (base * (1.0 - (-t).exp()) + wobble * (t * 3.0).sin()).max(0.0)) }).collect()
}

fn m(scope: Scope, kind: &str, room: &str, down: f64, up: f64, ping: f64, link: Option<u64>, wifi: Option<WifiInfo>, plan: Option<(f64, f64)>) -> Measurement {
    let mut m = Measurement {
        schema: SCHEMA, id: uuid::Uuid::new_v4().to_string(), started_at: chrono::Local::now(),
        config: TestConfig { scope, iface: "en0".into(), server: "192.168.1.20".into(), code: "4711".into(), streams: 6, secs: 10.0 },
        context: Context { customer: "Praxis Dr. Müller".into(), room: room.into(), technician: "N. John".into(), notes: String::new(), plan_down: plan.map(|p| p.0), plan_up: plan.map(|p| p.1) },
        environment: Environment {
            os: "macOS 26.5".into(), hostname: "MacBook-Techniker".into(), app_version: "0.1.0".into(),
            iface: IfaceInfo { name: "en0".into(), label: if kind == "wifi" { "WLAN (Wi-Fi)".into() } else { "Kabel (AX88179A)".into() }, index: 1, kind: kind.into(), ipv4: Some("192.168.1.34".into()), gateway: Some("192.168.1.1".into()), link_mbps: link, is_default: true, wifi },
            server: if scope == Scope::Internal { "Praxis-Server (192.168.1.20)".into() } else { "speed.cloudflare.com".into() },
            server_location: (scope == Scope::External).then(|| "Cloudflare Frankfurt (FRA)".into()),
            public_ip: Some("203.0.113.7".into()), ..Default::default()
        },
        results: Results {
            latency: Latency { min_ms: ping * 0.8, median_ms: ping, p95_ms: ping * 1.4, max_ms: ping * 2.0, jitter_ms: ping * 0.15, loss_pct: 0.0, samples: series(ping, ping * 0.2, 80) },
            download: Throughput { median_mbps: down, max_mbps: down * 1.05, avg_mbps: down, samples: series(down, down * 0.04, 40) },
            upload: Throughput { median_mbps: up, max_mbps: up * 1.05, avg_mbps: up, samples: series(up, up * 0.05, 40) },
            loaded_down_ms: (scope == Scope::External).then_some(ping + 85.0), loaded_up_ms: (scope == Scope::External).then_some(ping + 22.0),
        },
        verdict: Verdict { rating: Rating::Good, summary: String::new(), findings: vec![], metrics: Default::default() },
        warnings: vec![],
    };
    m.verdict = verdict::evaluate(&m);
    m
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or("bericht-demo.pdf".into());
    let ms = vec![
        m(Scope::Internal, "ethernet", "Anmeldung", 941.0, 938.0, 0.4, Some(1000), None, None),
        m(Scope::Internal, "wifi", "Behandlungsraum 2", 182.0, 164.0, 6.5, None, Some(WifiInfo { ssid: Some("Praxis-Intern".into()), bssid: Some("a4:2b:b0:11:22:33".into()), rssi_dbm: Some(-71), noise_dbm: Some(-95), channel: Some(6), band: Some("2,4 GHz".into()), width_mhz: Some(20), tx_rate_mbps: Some(144.0), quality_pct: None }), None),
        m(Scope::External, "ethernet", "Serverraum (Router)", 212.0, 38.5, 24.0, Some(1000), None, Some((250.0, 40.0))),
    ];
    let pdf = report::pdf(&ms, &report::Options { company: "infralatis".into(), customer: String::new(), technician: String::new(), show_public_ip: false, show_ssid: false }).unwrap();
    std::fs::write(&out, pdf).unwrap();
    println!("{out}");
}
