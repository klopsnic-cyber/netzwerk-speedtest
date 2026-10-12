//! Ablauf einer Messung: Schnittstelle prüfen, Umgebung erfassen, intern/extern messen,
//! Warnhinweise sammeln, bewerten.

use crate::model::*;
use crate::{cloud, iface, lan, verdict};
use std::io;
use std::sync::{Arc, Mutex};

pub async fn run(cfg: TestConfig, ctx: Context, sink: Sink, cancel: Cancel) -> io::Result<Measurement> {
    let iface = iface::find(&cfg.iface).ok_or_else(|| io::Error::other("Die gewählte Schnittstelle gibt es nicht mehr."))?;
    if iface.ipv4.is_none() {
        let what = if iface.kind == "wifi" { "WLAN ist nicht verbunden" } else { "Netzwerkkabel ist nicht verbunden" };
        return Err(io::Error::other(format!("{what} – bitte verbinden oder andere Verbindung wählen.")));
    }
    let bind = iface::bind_for(&iface);
    let mut env = Environment {
        os: sysinfo::System::long_os_version().unwrap_or_else(|| std::env::consts::OS.into()),
        hostname: crate::server::host_name(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        on_battery: iface::on_battery(),
        vpn: iface::active_vpns(),
        iface: iface.clone(),
        ..Default::default()
    };
    let started_at = chrono::Local::now();
    let _awake = iface::KeepAwake::new();

    // CPU-Last während der Messung mitschreiben (hohe Last verfälscht Gbit-Messungen)
    let cpu = Arc::new(Mutex::new((sysinfo::System::new(), 0.0f64)));
    let cpu_tick = {
        let cpu = cpu.clone();
        move || {
            if let Ok(mut g) = cpu.lock() {
                g.0.refresh_cpu_usage();
                let u = g.0.global_cpu_usage() as f64;
                g.1 = g.1.max(u);
            }
        }
    };

    let results = match cfg.scope {
        Scope::Internal => {
            let addr = crate::net::parse_server(&cfg.server)?;
            let name = lan::hello(addr, &cfg.code, &bind).await?;
            env.server = format!("{name} ({})", addr.ip());
            lan::run(&cfg, bind, sink.clone(), cancel.clone(), cpu_tick).await?
        }
        Scope::External => {
            let (r, meta) = cloud::run(&cfg, bind, sink.clone(), cancel.clone(), cpu_tick).await?;
            env.server = "speed.cloudflare.com".into();
            env.server_location = meta.location;
            env.public_ip = meta.ip;
            env.provider = meta.provider;
            r
        }
    };
    if cancel.is_cancelled() {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "Messung abgebrochen."));
    }
    env.max_cpu_pct = cpu.lock().map(|g| (g.1 * 10.0).round() / 10.0).unwrap_or(0.0);
    // WLAN-Werte nach der Messung frisch lesen (Signal/Rate während der Last aussagekräftiger)
    if env.iface.kind == "wifi" {
        env.iface.wifi = iface::wifi_info(&env.iface.name).or(env.iface.wifi.take());
    }

    let mut warnings = Vec::new();
    if cfg.scope == Scope::External && !env.vpn.is_empty() {
        warnings.push(format!(
            "VPN aktiv ({}). Läuft der Internetverkehr durch den Tunnel, misst der Test den VPN-Weg statt des Anschlusses.",
            env.vpn.join(", ")
        ));
    }
    if env.on_battery {
        warnings.push("Gerät lief im Akkubetrieb – Energiesparfunktionen können WLAN und Netzwerk drosseln. Netzteil anschließen.".into());
    }
    if env.max_cpu_pct > 85.0 {
        warnings.push(format!("Hohe CPU-Last ({:.0} %) während der Messung – Werte können zu niedrig sein.", env.max_cpu_pct));
    }
    let mut m = Measurement {
        schema: SCHEMA,
        id: uuid::Uuid::new_v4().to_string(),
        started_at,
        config: cfg,
        context: ctx,
        environment: env,
        results,
        verdict: Verdict { rating: Rating::Good, summary: String::new(), findings: vec![], metrics: Default::default() },
        warnings,
    };
    m.verdict = verdict::evaluate(&m);
    Ok(m)
}
