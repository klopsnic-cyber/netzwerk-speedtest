pub mod cloud;
pub mod engine;
pub mod iface;
pub mod lan;
pub mod meter;
pub mod model;
pub mod net;
pub mod report;
pub mod server;
pub mod stats;
pub mod store;
pub mod verdict;

use model::{Cancel, Context, Event, IfaceInfo, Measurement, TestConfig};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tauri::ipc::Channel;

#[derive(Default)]
struct AppState {
    cancel: Mutex<Option<Cancel>>,
    server: Mutex<Option<server::ServerHandle>>,
}

fn store(app: &tauri::AppHandle) -> Result<store::Store, String> {
    Ok(store::Store::new(app.path().app_data_dir().map_err(|e| e.to_string())?))
}

#[tauri::command]
fn list_interfaces() -> Vec<IfaceInfo> {
    iface::list()
}

#[tauri::command]
async fn start_test(app: tauri::AppHandle, config: TestConfig, context: Context, on_event: Channel<Event>) -> Result<Measurement, String> {
    let cancel = Cancel::default();
    {
        let st = app.state::<AppState>();
        let mut c = st.cancel.lock().unwrap();
        if c.as_ref().is_some_and(|c| !c.is_cancelled()) {
            return Err("Es läuft bereits eine Messung.".into());
        }
        *c = Some(cancel.clone());
    }
    let sink: model::Sink = Arc::new(move |e| {
        let _ = on_event.send(e);
    });
    let res = engine::run(config, context, sink, cancel.clone()).await;
    cancel.cancel(); // markiert "fertig", damit die nächste Messung starten darf
    let m = res.map_err(|e| e.to_string())?;
    store(&app)?.save(&m).map_err(|e| format!("Messung konnte nicht gespeichert werden: {e}"))?;
    Ok(m)
}

#[tauri::command]
fn cancel_test(state: tauri::State<AppState>) {
    if let Some(c) = state.cancel.lock().unwrap().as_ref() {
        c.cancel();
    }
}

#[derive(serde::Serialize)]
struct ServerInfo {
    code: String,
    port: u16,
    name: String,
    addresses: Vec<String>,
}

#[tauri::command]
async fn start_server(state: tauri::State<'_, AppState>, on_event: Channel<server::ServerEvent>) -> Result<ServerInfo, String> {
    drop(state.server.lock().unwrap().take());
    let code = server::new_code();
    let h = server::start(net::PORT, code.clone(), Arc::new(move |e| {
        let _ = on_event.send(e);
    }))
    .await
    .map_err(|e| e.to_string())?;
    let info = ServerInfo {
        code,
        port: h.port,
        name: server::host_name(),
        addresses: iface::list().into_iter().filter_map(|i| i.ipv4.map(|ip| format!("{ip} ({})", i.label))).collect(),
    };
    *state.server.lock().unwrap() = Some(h);
    Ok(info)
}

#[tauri::command]
fn stop_server(state: tauri::State<AppState>) {
    drop(state.server.lock().unwrap().take());
}

#[tauri::command]
async fn discover_servers() -> Vec<server::Found> {
    server::discover(std::time::Duration::from_millis(2500)).await
}

#[tauri::command]
fn history(app: tauri::AppHandle) -> Result<Vec<Measurement>, String> {
    Ok(store(&app)?.list())
}

#[tauri::command]
fn delete_measurement(app: tauri::AppHandle, id: String) -> Result<(), String> {
    store(&app)?.delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_measurement(app: tauri::AppHandle, id: String, room: String, notes: String, customer: String) -> Result<Measurement, String> {
    store(&app)?.update_context(&id, &room, &notes, &customer).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> Result<store::Settings, String> {
    Ok(store(&app)?.settings())
}

#[tauri::command]
fn save_settings(app: tauri::AppHandle, settings: store::Settings) -> Result<(), String> {
    store(&app)?.save_settings(&settings).map_err(|e| e.to_string())
}

#[tauri::command]
fn report_file_name(customer: String) -> String {
    report::file_name(&customer)
}

/// Erzeugt das PDF aus den gewählten Messungen und schreibt es nach `path` (vom Speichern-Dialog).
#[tauri::command]
async fn export_pdf(app: tauri::AppHandle, ids: Vec<String>, path: String, options: report::Options) -> Result<String, String> {
    let st = store(&app)?;
    let mut ms = ids.iter().map(|id| st.get(id)).collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    ms.sort_by_key(|m| m.started_at);
    let bytes = tauri::async_runtime::spawn_blocking(move || report::pdf(&ms, &options))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| format!("PDF konnte nicht gespeichert werden: {e}"))?;
    Ok(path)
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .setup(|_| {
            // macOS zeigt die WLAN-SSID nur mit Ortungsdienste-Freigabe
            iface::request_location_permission();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_interfaces,
            start_test,
            cancel_test,
            start_server,
            stop_server,
            discover_servers,
            history,
            delete_measurement,
            update_measurement,
            get_settings,
            save_settings,
            report_file_name,
            export_pdf,
            app_version,
        ])
        .run(tauri::generate_context!())
        .expect("Netzwerk-Speedtest konnte nicht starten");
}
