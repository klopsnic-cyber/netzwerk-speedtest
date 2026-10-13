// Brücke zum Rust-Kern (Tauri-Befehle) plus kleine Formatierhilfen.
import { invoke, Channel } from "@tauri-apps/api/core";

export const api = {
  interfaces: () => invoke("list_interfaces"),
  startTest: (config, context, onEvent) => {
    const ch = new Channel();
    ch.onmessage = onEvent;
    return invoke("start_test", { config, context, onEvent: ch });
  },
  cancelTest: () => invoke("cancel_test"),
  startServer: (onEvent) => {
    const ch = new Channel();
    ch.onmessage = onEvent;
    return invoke("start_server", { onEvent: ch });
  },
  stopServer: () => invoke("stop_server"),
  discover: () => invoke("discover_servers"),
  history: () => invoke("history"),
  deleteMeasurement: (id) => invoke("delete_measurement", { id }),
  updateMeasurement: (id, room, notes, customer) => invoke("update_measurement", { id, room, notes, customer }),
  settings: () => invoke("get_settings"),
  saveSettings: (settings) => invoke("save_settings", { settings }),
  reportFileName: (customer) => invoke("report_file_name", { customer }),
  exportPdf: (ids, path, options) => invoke("export_pdf", { ids, path, options }),
  version: () => invoke("app_version"),
};

/** Zahl mit deutschem Komma, Stellen je nach Größe (wie im PDF). */
export function de(x) {
  if (x == null || Number.isNaN(x)) return "–";
  const a = Math.abs(x);
  const d = a >= 100 ? 0 : a >= 10 ? 1 : 2;
  return x.toLocaleString("de-DE", { maximumFractionDigits: d, minimumFractionDigits: 0 });
}

export const ratingWord = { good: "Gut", warn: "Eingeschränkt", bad: "Kritisch" };

export function medium(iface) {
  if (!iface) return "";
  if (iface.kind === "wifi") {
    const w = iface.wifi || {};
    return ["WLAN", w.band, w.rssi_dbm != null ? `(${w.rssi_dbm} dBm)` : null].filter(Boolean).join(" ");
  }
  if (iface.kind === "ethernet") {
    const l = iface.link_mbps;
    return l ? (l >= 1000 ? `Kabel ${de(l / 1000)} Gbit/s` : `Kabel ${l} Mbit/s`) : "Kabel";
  }
  return "Netzwerk";
}

export function dateTime(iso) {
  return new Date(iso).toLocaleString("de-DE", { dateStyle: "medium", timeStyle: "short" });
}

export function errText(e) {
  return typeof e === "string" ? e : e?.message || String(e);
}
