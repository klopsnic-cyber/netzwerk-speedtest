//! Lokale Ablage: eine JSON-Datei je Messung, dazu die Einstellungen.

use crate::model::Measurement;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub technician: String,
    pub company: String,
    pub customer: String,
    pub plan_down: Option<f64>,
    pub plan_up: Option<f64>,
    pub reduce_effects: bool,
    pub show_public_ip: bool,
    pub show_ssid: bool,
    pub streams: usize,
    pub secs: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            technician: String::new(),
            company: "infralatis".into(),
            customer: String::new(),
            plan_down: None,
            plan_up: None,
            reduce_effects: false,
            show_public_ip: false,
            show_ssid: false,
            streams: 6,
            secs: 10.0,
        }
    }
}

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Store { dir }
    }

    fn results_dir(&self) -> PathBuf {
        self.dir.join("messungen")
    }

    fn path_for(&self, id: &str) -> io::Result<PathBuf> {
        // IDs sind UUIDs; alles andere ablehnen, damit kein Pfad außerhalb entsteht
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "ungültige Messungs-ID"));
        }
        Ok(self.results_dir().join(format!("{id}.json")))
    }

    pub fn save(&self, m: &Measurement) -> io::Result<()> {
        fs::create_dir_all(self.results_dir())?;
        write_atomic(&self.path_for(&m.id)?, &serde_json::to_vec_pretty(m)?)
    }

    pub fn list(&self) -> Vec<Measurement> {
        let mut v: Vec<Measurement> = fs::read_dir(self.results_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| serde_json::from_slice(&fs::read(e.path()).ok()?).ok())
            .collect();
        v.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        v
    }

    pub fn get(&self, id: &str) -> io::Result<Measurement> {
        serde_json::from_slice(&fs::read(self.path_for(id)?)?).map_err(io::Error::other)
    }

    pub fn delete(&self, id: &str) -> io::Result<()> {
        fs::remove_file(self.path_for(id)?)
    }

    /// Übernimmt Raum/Notiz nachträglich (z. B. nach dem Test im Ergebnisbildschirm eingetragen).
    pub fn update_context(&self, id: &str, room: &str, notes: &str, customer: &str) -> io::Result<Measurement> {
        let mut m = self.get(id)?;
        m.context.room = room.into();
        m.context.notes = notes.into();
        m.context.customer = customer.into();
        self.save(&m)?;
        Ok(m)
    }

    pub fn settings(&self) -> Settings {
        fs::read(self.dir.join("einstellungen.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, s: &Settings) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        write_atomic(&self.dir.join("einstellungen.json"), &serde_json::to_vec_pretty(s)?)
    }
}

fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path)
}
