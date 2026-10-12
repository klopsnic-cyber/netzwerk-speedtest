//! Datenmodell einer Messung (so wird sie gespeichert, angezeigt und ins PDF gegeben).

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Internal,
    External,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Ping,
    Download,
    Upload,
}

/// Was der Techniker vor dem Start auswählt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub scope: Scope,
    /// Name der Schnittstelle (z. B. "en0", "Ethernet"), leer = Systemstandard
    pub iface: String,
    /// Gegenstelle für intern, z. B. "192.168.1.20" oder "192.168.1.20:5299"
    #[serde(default)]
    pub server: String,
    #[serde(default)]
    pub code: String,
    #[serde(default = "default_streams")]
    pub streams: usize,
    #[serde(default = "default_secs")]
    pub secs: f64,
}

fn default_streams() -> usize { 4 }
fn default_secs() -> f64 { 10.0 }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Context {
    #[serde(default)]
    pub customer: String,
    #[serde(default)]
    pub room: String,
    #[serde(default)]
    pub technician: String,
    #[serde(default)]
    pub notes: String,
    /// gebuchter Tarif in Mbit/s (nur extern sinnvoll)
    #[serde(default)]
    pub plan_down: Option<f64>,
    #[serde(default)]
    pub plan_up: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WifiInfo {
    pub ssid: Option<String>,
    pub bssid: Option<String>,
    pub rssi_dbm: Option<i32>,
    pub noise_dbm: Option<i32>,
    pub channel: Option<u32>,
    pub band: Option<String>,
    pub width_mhz: Option<u32>,
    pub tx_rate_mbps: Option<f64>,
    /// Signalqualität in % (Windows liefert das statt dBm teils genauer)
    pub quality_pct: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IfaceInfo {
    pub name: String,
    pub label: String,
    pub index: u32,
    /// "ethernet" | "wifi" | "other"
    pub kind: String,
    pub ipv4: Option<String>,
    pub gateway: Option<String>,
    pub link_mbps: Option<u64>,
    pub is_default: bool,
    pub wifi: Option<WifiInfo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Environment {
    pub os: String,
    pub hostname: String,
    pub app_version: String,
    pub iface: IfaceInfo,
    /// Gegenstelle (intern) bzw. Messserver (extern), lesbar
    pub server: String,
    pub server_location: Option<String>,
    pub public_ip: Option<String>,
    pub provider: Option<String>,
    pub on_battery: bool,
    pub vpn: Vec<String>,
    pub max_cpu_pct: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Throughput {
    pub median_mbps: f64,
    pub max_mbps: f64,
    pub avg_mbps: f64,
    /// (Sekunde seit Phasenbeginn, Mbit/s) je 250-ms-Intervall
    pub samples: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Latency {
    pub min_ms: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub jitter_ms: f64,
    pub loss_pct: f64,
    pub samples: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Results {
    pub latency: Latency,
    pub download: Throughput,
    pub upload: Throughput,
    /// Median der Laufzeit während Download bzw. Upload (Bufferbloat = Differenz zur Ruhe)
    pub loaded_down_ms: Option<f64>,
    pub loaded_up_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Rating {
    Good,
    Warn,
    Bad,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub rating: Rating,
    /// kurze Überschrift, z. B. "WLAN-Signal schwach"
    pub title: String,
    /// Empfehlung in Klartext
    pub advice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    pub rating: Rating,
    pub summary: String,
    pub findings: Vec<Finding>,
    /// Ampel je Kennzahl: "throughput", "ping", "jitter", "loss", "bufferbloat", "signal"
    #[serde(default)]
    pub metrics: std::collections::BTreeMap<String, Rating>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    pub schema: u32,
    pub id: String,
    pub started_at: chrono::DateTime<chrono::Local>,
    pub config: TestConfig,
    pub context: Context,
    pub environment: Environment,
    pub results: Results,
    pub verdict: Verdict,
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// Live-Ereignisse an die Oberfläche (höchstens ca. 10 pro Sekunde).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Event {
    Phase { phase: Phase },
    Throughput { phase: Phase, t: f64, mbps: f64 },
    Ping { phase: Phase, t: f64, ms: f64 },
    Info { message: String },
}

pub type Sink = Arc<dyn Fn(Event) + Send + Sync>;

/// Abbruch-Schalter, von allen Messschleifen geprüft.
#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn cancel(&self) { self.0.store(true, Ordering::Relaxed) }
    pub fn is_cancelled(&self) -> bool { self.0.load(Ordering::Relaxed) }
}
