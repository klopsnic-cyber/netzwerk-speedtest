//! PDF-Bericht: Messungen aufbereiten (deutsche Zahlen, Datenschutz-Schalter) und mit Typst setzen.
//! Die Vorlage liegt in assets/bericht.typ, Schrift Inter ist eingebettet -> identisch auf Mac/Windows.

use crate::model::{Finding, Measurement, Rating, Scope};
use serde::Serialize;
use std::io;
use typst::foundations::{Dict, IntoValue};
use typst_as_lib::TypstEngine;

static TEMPLATE: &str = include_str!("../assets/bericht.typ");
static FONTS: [&[u8]; 4] = [
    include_bytes!("../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../assets/fonts/Inter-Bold.ttf"),
];

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct Options {
    pub company: String,
    pub customer: String,
    pub technician: String,
    pub show_public_ip: bool,
    pub show_ssid: bool,
}

#[derive(Serialize)]
struct Data {
    company: String,
    customer: String,
    technician: String,
    date: String,
    report_id: String,
    app_version: String,
    rating: Rating,
    summary: String,
    points: usize,
    scopes: String,
    top: Vec<Finding>,
    rows: Vec<Row>,
    plan: Option<Vec<PlanBar>>,
    pages: Vec<Page>,
}

#[derive(Serialize)]
struct Row { room: String, medium: String, down: String, up: String, ping: String, scope: String, rating: Rating }

#[derive(Serialize)]
struct PlanBar { label: String, pct: u32, plan: String, rating: Rating }

#[derive(Serialize)]
struct Metric { label: String, value: String, unit: String, rating: Rating }

#[derive(Serialize)]
struct Page {
    heading: String,
    subtitle: String,
    rating: Rating,
    metrics: Vec<Metric>,
    findings: Vec<Finding>,
    info: Vec<(String, String)>,
    warnings: Vec<String>,
    notes: String,
    down: Vec<(f64, f64)>,
    up: Vec<(f64, f64)>,
    ping: Vec<(f64, f64)>,
}

/// Zahl mit deutschem Komma; Nachkommastellen je nach Größe.
pub fn de(x: f64) -> String {
    let digits = if x.abs() >= 100.0 { 0 } else if x.abs() >= 10.0 { 1 } else { 2 };
    let s = format!("{x:.digits$}");
    let s = if digits > 0 { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    s.replace('.', ",")
}

pub fn medium(m: &Measurement) -> String {
    let i = &m.environment.iface;
    match i.kind.as_str() {
        "wifi" => {
            let w = i.wifi.clone().unwrap_or_default();
            let mut s = "WLAN".to_string();
            if let Some(b) = w.band { s += &format!(" {b}"); }
            if let Some(r) = w.rssi_dbm { s += &format!(" ({r} dBm)"); }
            s
        }
        "ethernet" => match i.link_mbps {
            Some(l) if l >= 1000 => format!("Kabel {} Gbit/s", de(l as f64 / 1000.0)),
            Some(l) => format!("Kabel {l} Mbit/s"),
            None => "Kabel".into(),
        },
        _ => "Netzwerk".into(),
    }
}

fn mask_bssid(b: &str) -> String {
    let parts: Vec<&str> = b.split(':').collect();
    if parts.len() == 6 { format!("{}:xx:xx:xx", parts[..3].join(":")) } else { "–".into() }
}

fn room(m: &Measurement, n: usize) -> String {
    if m.context.room.trim().is_empty() { format!("Messpunkt {n}") } else { m.context.room.clone() }
}

fn rating_of(m: &Measurement, key: &str) -> Rating {
    m.verdict.metrics.get(key).cloned().unwrap_or(Rating::Good)
}

fn page(m: &Measurement, n: usize, o: &Options) -> Page {
    let r = &m.results;
    let e = &m.environment;
    let scope = if m.config.scope == Scope::Internal { "Intern (LAN)" } else { "Internet" };
    let mut info: Vec<(String, String)> = vec![
        ("Zeitpunkt".into(), m.started_at.format("%d.%m.%Y, %H:%M Uhr").to_string()),
        ("Messart".into(), scope.into()),
        ("Verbindung".into(), format!("{} · {}", medium(m), e.iface.label)),
    ];
    if let Some(ip) = &e.iface.ipv4 { info.push(("IP-Adresse".into(), ip.clone())); }
    if let Some(g) = &e.iface.gateway { info.push(("Gateway".into(), g.clone())); }
    if let Some(w) = &e.iface.wifi {
        if o.show_ssid {
            if let Some(s) = &w.ssid { info.push(("WLAN-Name".into(), s.clone())); }
        }
        if let Some(b) = &w.bssid { info.push(("Access Point".into(), mask_bssid(b))); }
        if let Some(c) = w.channel {
            let width = w.width_mhz.map(|x| format!(", {x} MHz")).unwrap_or_default();
            info.push(("Kanal".into(), format!("{c}{width}")));
        }
        if let Some(rate) = w.tx_rate_mbps { info.push(("WLAN-Datenrate".into(), format!("{} Mbit/s", de(rate)))); }
        if let Some(q) = w.quality_pct { info.push(("Signalqualität".into(), format!("{q} %"))); }
    }
    let server = match &e.server_location { Some(l) => format!("{} · {l}", e.server), None => e.server.clone() };
    info.push((if m.config.scope == Scope::Internal { "Gegenstelle" } else { "Messserver" }.into(), server));
    if o.show_public_ip {
        if let Some(ip) = &e.public_ip { info.push(("Öffentliche IP".into(), ip.clone())); }
    }
    info.push(("Messgerät".into(), format!("{} ({})", e.hostname, e.os)));
    info.push(("Messung".into(), format!("{} Verbindungen, je {} s", m.config.streams, de(m.config.secs))));

    let mut metrics = vec![
        Metric { label: "Download".into(), value: de(r.download.median_mbps), unit: "Mbit/s".into(), rating: rating_of(m, "throughput") },
        Metric { label: "Upload".into(), value: de(r.upload.median_mbps), unit: "Mbit/s".into(), rating: rating_of(m, "throughput") },
        Metric { label: "Ping".into(), value: de(r.latency.median_ms), unit: "ms".into(), rating: rating_of(m, "ping") },
        Metric { label: "Jitter".into(), value: de(r.latency.jitter_ms), unit: "ms".into(), rating: rating_of(m, "jitter") },
        Metric { label: "Verlust".into(), value: de(r.latency.loss_pct), unit: "%".into(), rating: rating_of(m, "loss") },
    ];
    if m.config.scope == Scope::External {
        if let Some(l) = r.loaded_down_ms.into_iter().chain(r.loaded_up_ms).reduce(f64::max) {
            metrics.push(Metric { label: "Ping unter Last".into(), value: de(l), unit: "ms".into(), rating: rating_of(m, "bufferbloat") });
        }
    }
    Page {
        heading: room(m, n),
        subtitle: format!("{scope} · {} · {}", medium(m), m.started_at.format("%d.%m.%Y %H:%M")),
        rating: m.verdict.rating.clone(),
        metrics,
        findings: m.verdict.findings.clone(),
        info,
        warnings: m.warnings.clone(),
        notes: m.context.notes.clone(),
        down: r.download.samples.clone(),
        up: r.upload.samples.clone(),
        ping: r.latency.samples.clone(),
    }
}

fn build(ms: &[Measurement], o: &Options) -> Data {
    let rating = ms.iter().map(|m| m.verdict.rating.clone()).max().unwrap_or(Rating::Good);
    let summary = match rating {
        Rating::Good => "Das Netzwerk ist für den Praxisbetrieb geeignet.",
        Rating::Warn => "Das Netzwerk funktioniert, an einzelnen Stellen wird eine Verbesserung empfohlen.",
        Rating::Bad => "Es besteht Handlungsbedarf – Details und Maßnahmen siehe unten.",
    };
    // Maßnahmen: alle nicht-grünen Befunde, schlechteste zuerst, ohne Doppelungen, höchstens 5
    let mut top: Vec<Finding> = Vec::new();
    for (i, m) in ms.iter().enumerate() {
        for f in m.verdict.findings.iter().filter(|f| f.rating != Rating::Good) {
            let mut f = f.clone();
            if ms.len() > 1 { f.title = format!("{}: {}", room(m, i + 1), f.title); }
            if !top.iter().any(|t| t.advice == f.advice && t.title == f.title) { top.push(f); }
        }
    }
    top.sort_by(|a, b| b.rating.cmp(&a.rating));
    top.truncate(5);

    let rows = ms.iter().enumerate().map(|(i, m)| Row {
        room: room(m, i + 1),
        medium: medium(m),
        down: de(m.results.download.median_mbps),
        up: de(m.results.upload.median_mbps),
        ping: de(m.results.latency.median_ms),
        scope: if m.config.scope == Scope::Internal { "LAN".into() } else { "Internet".into() },
        rating: m.verdict.rating.clone(),
    }).collect();

    let plan = ms.iter().find(|m| m.config.scope == Scope::External && m.context.plan_down.is_some()).map(|m| {
        let mut bars = Vec::new();
        for (label, got, plan) in [("Download", m.results.download.median_mbps, m.context.plan_down), ("Upload", m.results.upload.median_mbps, m.context.plan_up)] {
            if let Some(p) = plan.filter(|p| *p > 0.0) {
                let pct = (got / p * 100.0).round().max(0.0) as u32;
                let rating = if pct >= 90 { Rating::Good } else if pct >= 60 { Rating::Warn } else { Rating::Bad };
                bars.push(PlanBar { label: label.into(), pct, plan: de(p), rating });
            }
        }
        bars
    });

    let mut scopes: Vec<&str> = ms.iter().map(|m| if m.config.scope == Scope::Internal { "LAN" } else { "Internet" }).collect();
    scopes.dedup();
    Data {
        company: o.company.clone(),
        customer: if o.customer.is_empty() { ms.first().map(|m| m.context.customer.clone()).unwrap_or_default() } else { o.customer.clone() },
        technician: if o.technician.is_empty() { ms.first().map(|m| m.context.technician.clone()).unwrap_or_default() } else { o.technician.clone() },
        date: chrono::Local::now().format("%d.%m.%Y").to_string(),
        report_id: ms.first().map(|m| m.id[..8].to_uppercase()).unwrap_or_default(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        rating,
        summary: summary.into(),
        points: ms.len(),
        scopes: scopes.join(" und "),
        top,
        rows,
        plan: plan.filter(|b| !b.is_empty()),
        pages: ms.iter().enumerate().map(|(i, m)| page(m, i + 1, o)).collect(),
    }
}

/// Erzeugt das PDF (Messungen in der übergebenen Reihenfolge).
pub fn pdf(ms: &[Measurement], o: &Options) -> io::Result<Vec<u8>> {
    if ms.is_empty() {
        return Err(io::Error::other("Keine Messung ausgewählt."));
    }
    let json = serde_json::to_string(&build(ms, o))?;
    let mut inputs = Dict::new();
    inputs.insert("daten".into(), json.into_value());
    let engine = TypstEngine::builder().main_file(TEMPLATE).fonts(FONTS).build();
    let doc = engine
        .compile_with_input(inputs)
        .output
        .map_err(|e| io::Error::other(format!("PDF-Satz fehlgeschlagen: {e:?}")))?;
    typst_pdf::pdf(&doc, &Default::default()).map_err(|e| io::Error::other(format!("PDF-Export fehlgeschlagen: {e:?}")))
}

/// Dateiname: Netzwerkbericht_Praxis-Mueller_2026-10-12.pdf (Umlaute ersetzt, nur sichere Zeichen).
pub fn file_name(customer: &str) -> String {
    let c: String = customer
        .replace('ä', "ae").replace('ö', "oe").replace('ü', "ue").replace('Ä', "Ae").replace('Ö', "Oe").replace('Ü', "Ue").replace('ß', "ss")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let c = c.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    let date = chrono::Local::now().format("%Y-%m-%d");
    if c.is_empty() { format!("Netzwerkbericht_{date}.pdf") } else { format!("Netzwerkbericht_{c}_{date}.pdf") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zahlen_und_dateiname() {
        assert_eq!(de(912.44), "912");
        assert_eq!(de(42.26), "42,3");
        assert_eq!(de(4.0), "4");
        assert_eq!(de(0.25), "0,25");
        assert_eq!(file_name("Praxis Müller & Söhne"), format!("Netzwerkbericht_Praxis-Mueller-Soehne_{}.pdf", chrono::Local::now().format("%Y-%m-%d")));
        assert_eq!(mask_bssid("aa:bb:cc:dd:ee:ff"), "aa:bb:cc:xx:xx:xx");
    }
}
