# Netzwerk-Speedtest

Messwerkzeug für Praxis-Netzwerke (macOS/Windows): **intern** (LAN zwischen zwei Geräten) und **extern** (Internet über Cloudflare), wahlweise über **Kabel oder WLAN**. Live-Tacho und Kurven während der Messung, verständliche Bewertung, PDF-Bericht für den Kunden.

## Download
Neueste Version: [Releases](https://github.com/klopsnic-cyber/netzwerk-speedtest/releases/latest) – `.dmg` (macOS, Intel + Apple Silicon) bzw. `-setup.exe` (Windows, ohne Admin-Rechte). Updates kommen danach automatisch über die App.

**macOS, erster Start:** Die App ist nicht notarisiert. Einmal öffnen, dann *Systemeinstellungen → Datenschutz & Sicherheit → Dennoch öffnen*. Die Abfrage für „Lokales Netzwerk“ zulassen (Gegenstelle/Suche) und für „Ortungsdienste“ (nur nötig, damit macOS den WLAN-Namen herausgibt).

**Windows, erster Start:** SmartScreen → *Weitere Informationen → Trotzdem ausführen*. Beim Start der Gegenstelle die Firewall-Abfrage für **private Netzwerke** zulassen.

## Bedienung
- **Extern:** Anschluss (Kabel/WLAN) wählen, optional Tarif eintragen, *Messung starten*.
- **Intern:** Auf einem zweiten Gerät *Gegenstelle → starten*. Auf dem Messgerät „Intern (LAN)“ wählen, die Gegenstelle wird automatisch gefunden (sonst IP eintragen), den 4-stelligen Code eingeben.
- **Bericht:** Im Ergebnis *PDF-Bericht*, oder im *Verlauf* mehrere Messpunkte (Räume) auswählen und zu einem Bericht zusammenfassen.

Gegenstelle: Port 5299 TCP/UDP, mDNS `_netzspeed._tcp`.

## Entwicklung
```sh
npm install
npm run tauri dev
cd src-tauri && cargo test
```
Release: Version in `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` erhöhen, CHANGELOG ergänzen, Tag `vX.Y.Z` pushen – GitHub Actions baut dmg + exe und `latest.json` für den Updater.
