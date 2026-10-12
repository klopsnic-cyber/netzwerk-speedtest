# Netzwerk-Speedtest – Projektplan

Stand 12.10.2026. Zusammengeführt aus sechs Planungen: Architektur, Messmethodik, Design, Auswertung/PDF,
Build/Verteilung, Risiken/Tests. Noch nichts gebaut.

## 1. Worum es geht (in einfachen Worten)

Ein Programm für Mac und Windows, mit dem ein Techniker in einer Praxis misst:

- **Intern:** Wie schnell ist das Netz *zwischen zwei Geräten* in der Praxis?
  Dafür läuft dasselbe Programm auf einem zweiten Gerät als **Gegenstelle** („Server-Modus“).
  Ohne Gegenstelle kann man LAN-Geschwindigkeit nicht ehrlich messen. Ein Router als Gegenstelle bremst selbst.
- **Extern:** Wie schnell ist das **Internet**? Gemessen wird gegen Cloudflare (Rechenzentrum meist Frankfurt).
- **Kabel oder WLAN:** Der Techniker wählt die Schnittstelle. Das Programm misst *nachweislich* über genau diese
  Schnittstelle, auch wenn Kabel und WLAN gleichzeitig verbunden sind. Bei WLAN zeigt es Signal, Kanal und Band.
- **Während des Tests:** großer animierter Tacho, Live-Kurve Download/Upload, Latenz-Kurve, dezente Partikel und
  Glow-Effekte.
- **Danach:** Auswertung mit Ampel und Klartext-Empfehlungen im Programm, als PDF-Bericht für den Praxisinhaber.
  Mehrere Räume lassen sich in einem Bericht zusammenfassen.

```
 Techniker-Laptop                                  zweites Gerät (Praxis-PC/Laptop)
┌────────────────────────┐   intern: TCP 4–8 Streams  ┌──────────────────────┐
│ Speedtest  (Client)    │ ◄────────────────────────► │ Speedtest (Gegenstelle)│
│  Kabel ⌁  /  WLAN ≋    │   + UDP-Ping/Jitter        │  wartet auf Port 5201 │
└──────────┬─────────────┘                            └──────────────────────┘
           │ extern: HTTPS Down/Up + Ping unter Last
           ▼
   speed.cloudflare.com (Frankfurt)
```

## 2. Technik (Empfehlung aller Planer)

| Teil | Wahl | Warum |
|---|---|---|
| App-Gerüst | **Tauri 2** | Ein Code für Mac und Windows, ca. 10 MB statt 100 MB (Electron), Web-Optik |
| Messkern | **Rust** (tokio, eigene Threads) | Schafft 1–10 Gbit/s; Animationen können die Messung nicht ausbremsen |
| Oberfläche | **Svelte 5 + Vite** | Klein, schnell, wenig Code |
| Graphen | **uPlot** (Canvas) + eigener Canvas-Tacho | Echtzeitfähig, winzig; ECharts/Chart.js zu schwer |
| Effekte | Canvas 2D (≤60 Partikel) + CSS | Kein WebGL nötig; während Messung auf 30 fps gedrosselt |
| PDF | **Typst** als Rust-Bibliothek | Auf Mac/Windows identische Ausgabe, Schriften eingebettet, Graphen als Vektor |
| Speicherung | JSON-Dateien je Messung (später ggf. SQLite) | Einfach, exportierbar |
| Auto-Update | Tauri-Updater + GitHub Releases | Eigener Signaturschlüssel, keine Apple-ID nötig |
| Build | GitHub Actions (Mac universal .dmg, Windows .exe per NSIS) | Wie Netzwerk-Scanner, aber beides in der Cloud |

Python/Tkinter scheidet aus: zu langsam für Gbit-Messung und optisch nicht modern genug.

## 3. Messmethodik

**Intern (LAN)**
- Gegenstelle per Bonjour/mDNS automatisch finden, manuelle IP als Ersatz (VLANs, Gäste-WLAN).
- Pairing-Code beim Verbinden, damit niemand Fremdes die Gegenstelle nutzt.
- Download, dann Upload mit je 4 parallelen TCP-Streams (8 bei >1 Gbit), je 10 s.
  Die ersten 2 s (Anlaufphase) werden verworfen. Gezählt wird in 250-ms-Intervallen, der Bericht nennt den Median.
- Latenz, Jitter und Paketverlust per UDP-Echo (20/s über 10 s).
- Zusätzlich Ping zum Router (nur Latenz).

**Extern (Internet)**
- Cloudflare-Endpunkte `__down`/`__up`: Ruhe-Ping, Download, Upload, dazu **Ping unter Last** (zeigt
  „Bufferbloat“, also ob der Router bei Last hakt). Der Serverstandort wird angezeigt.
- Ookla (speedtest.net) ist ausgeschlossen, weil die Lizenz keine Firmennutzung erlaubt.
- M-Lab ist optional, weil es Ergebnisse samt IP öffentlich macht (DSGVO).

**Kabel/WLAN**
- Schnittstellen auflisten, Typ erkennen.
- Sockets fest an die gewählte Schnittstelle binden (macOS `IP_BOUND_IF`, Windows `IP_UNICAST_IF`).
- Im Bericht stehen Name, Link-Geschwindigkeit und IP der Schnittstelle.
- Ist die gewählte Schnittstelle nicht verbunden, startet kein Test. Es gibt keinen stillen Wechsel auf die andere.
- WLAN-Infos: Signal (dBm), Kanal, Band, Rate.
  - Den **WLAN-Namen (SSID)** liefert macOS nur mit Ortungsdienst-Freigabe.
  - Ohne Freigabe steht dort „SSID unbekannt“.

**Warnungen im Programm:** VPN aktiv (z. B. Praxis-/TI-VPN), Proxy, Akkubetrieb, hohe CPU-Last, Gegenstelle zu
schwach, fehlende macOS-Berechtigung „Lokales Netzwerk“.

**Ampel (Startwerte, später anpassbar)**

| | Grün | Gelb | Rot |
|---|---|---|---|
| LAN-Durchsatz Kabel | ≥ 90 % der Link-Rate | 70–90 % | < 70 % |
| Internet ggü. Tarif | ≥ 90 % | 60–90 % | < 60 % |
| Latenz LAN / WLAN / Internet | < 2 / 10 / 25 ms | bis 10 / 30 / 60 ms | darüber |
| Jitter | < 5 ms | 5–20 ms | > 20 ms |
| Paketverlust | 0 % | < 0,5 % | ≥ 0,5 % |
| WLAN-Signal | > −60 dBm | −60 bis −70 | < −70 |
| Ping unter Last (Zuwachs) | < 30 ms | 30–100 ms | > 100 ms |

Der schlechteste Wert bestimmt die Gesamtfarbe, mit je einer Klartextzeile, z. B.:
- „WLAN-Signal schwach (−78 dBm): Access Point näher platzieren.“
- „Kabel nur 100 Mbit/s: Kabel/Switch-Port prüfen.“

## 4. Oberfläche

Dunkles „Cockpit“-Design, seriös statt Gaming-Neon:
- Grundton Anthrazit-Blau `#0B0F17`, Download Cyan `#22D3EE`, Upload Violett `#A78BFA`, Ping Gelb `#FBBF24`.
- Ampelfarben nur für Bewertungen, immer zusätzlich mit Symbol und Text.
- Schrift Inter, Zahlen in fester Breite, damit sie nicht springen. Symbole von Lucide.
- Alles ist lokal eingebettet, nichts wird aus dem Internet nachgeladen.

```
Start                                    Live-Test
┌─────────────────────────────────┐      ┌─────────────────────────────────┐
│ ◉ Speedtest        Verlauf  ⚙   │      │ ① Ping ── ② Download ── ③ Upload │
│ [⇄ Intern LAN] [☁ Extern Inet]  │      │            ╭──────╮              │
│ ┌ ⌁ Kabel ● 1 Gbit ┐┌ ≋ WLAN ▂▄▆ ┐│      │          ╱  847   ╲  Tacho+Glow  │
│ └ en0 aktiv        ┘└ -52 dBm Ch36┘│      │           ╲Mbit/s╱  + Partikel   │
│ Gegenstelle: Praxis-PC ● gefunden│      │ Down ▁▂▄▆▇█▇▇▆  Up ·····         │
│      [ ▶ Test starten ]          │      │ Latenz 4 ms ╱╲_╱  Jitter  Verlust │
└─────────────────────────────────┘      └─────────────────────────────────┘
Ergebnis                                 Gegenstelle (Server-Modus)
┌─────────────────────────────────┐      ┌─────────────────────────────────┐
│ ● Gut – für Praxisbetrieb geeignet│     │       ((  ◎  ))  Radar-Puls      │
│ [Down 847✓][Up 412✓][Ping 4✓]... │      │   Bereit – 192.168.1.20 : 5201   │
│ Kurve · Tarif ████████▎ 85 %      │      │   Code: 4711   [ Beenden ]       │
│ Raum: [Anmeldung] [PDF] [↻ neu]  │      └─────────────────────────────────┘
└─────────────────────────────────┘
```

**Damit die Effekte die Messung nicht verfälschen:**
- Die Messung läuft im Rust-Kern, die Oberfläche bekommt höchstens 10 Werte pro Sekunde.
- Während Download und Upload ist die Darstellung auf 30 fps gedrosselt.
- In der Ping-Phase pausieren die Effekte fast ganz.
- Es gibt einen Schalter „Effekte reduzieren“, außerdem wird die Systemeinstellung „Bewegung reduzieren“ beachtet.

## 5. Auswertung und PDF

- **Kopfdaten:**
  - Praxis, Raum/Standort, Techniker, Datum, Gerät, App-Version.
  - Schnittstelle mit WLAN-Details, lokale IP, Gateway und Gegenstelle bzw. Server.
- **Kennzahlen:** Download/Upload (Median und Maximum), Ping (Median, P95), Jitter, Verlust, Ping unter Last.
- **Datenschutz:**
  - Öffentliche IP und Provider sind standardmäßig **aus**.
  - BSSID wird gekürzt, keine fremden WLAN-Namen.
  - Patientendaten gibt es nie.
- **PDF (A4, hell, Inter eingebettet):**
  - S. 1: Gesamtampel mit 3–5 Kernaussagen und den 5 wichtigsten Maßnahmen.
  - S. 2: Übersicht aller Räume mit Balken „Ist gegen Tarif“.
  - Je Raum eine Seite mit Kacheln, Kurven und Empfehlung.
  - Anhang: Messmethode und Begriffe für Laien.
- **Dateiname:** `Netzwerkbericht_Praxis-Mueller_2026-10-12_01.pdf`. Dazu gibt es dieselben Daten als `.json`
  zum Weitergeben oder Wiedereinlesen.

## 6. Verteilung

- Repo `klopsnic-cyber/netzwerk-speedtest`. Builds und Updates laufen über GitHub Releases.
- Bei jedem `v*`-Tag baut GitHub Actions die Mac-.dmg (Intel und Apple Silicon) und die Windows-.exe.
- Auto-Update mit eigenem Signaturschlüssel.
  - Der Schlüssel kommt in den Passwort-Tresor.
  - Geht er verloren, lassen sich die installierten Versionen nie mehr automatisch aktualisieren.
- Hürden ohne kostenpflichtige Zertifikate:
  - **macOS:** beim ersten Start „Dennoch öffnen“ unter Datenschutz & Sicherheit.
  - **Windows:** SmartScreen-Warnung „Weitere Informationen → Trotzdem ausführen“.
  - **Firewall:** Beim ersten Start der Gegenstelle fragt die Firewall nach. Diese Freigabe ist einmalig nötig.
  - **Ortungsdienst (für die SSID):** Die Freigabe könnte bei jedem Update erneut abgefragt werden, weil die
    Signatur nur ad hoc ist. Abhilfe ist das Apple Developer Program für 99 $/Jahr.

## 7. Ablauf und Meilensteine

| # | Inhalt | Ergebnis |
|---|---|---|
| M1 | Gerüst, Rust-Messkern extern (Cloudflare), Live-Tacho + Kurve | Erster echter Internettest mit Optik |
| M2 | Gegenstelle/Server-Modus, interner Test, Bonjour + manuelle IP, Pairing-Code | LAN-Messung zwischen zwei Geräten |
| M3 | Kabel/WLAN-Auswahl mit Bindung, WLAN-Infos, Latenz/Jitter/Verlust, Ampel | Vollständige Messung + Bewertung |
| M4 | Ergebnisansicht, Verlauf, mehrere Räume, PDF-Bericht | Bericht für den Kunden |
| M5 | GitHub-Actions-Build, Auto-Update, Effekte verfeinern, Abnahmetest | Version 1.0 zum Verteilen |

**Genauigkeit belegen:**
- Vergleich mit `iperf3` im selben Aufbau, erlaubt sind ±5 % im LAN.
- Vergleich mit speed.cloudflare.com, erlaubt sind ±15 % beim Internet.
- Test mit künstlicher Drosselung (Network Link Conditioner), erlaubt sind ±3 %.
- Gleiches Ergebnis mit und ohne Effekte, erlaubt sind < 2 % Abweichung.

**Bewusst nicht in Version 1:** Konten/Cloud, Zeitpläne, WLAN-Kanalanalyse, Traceroute, Grundriss mit Markern,
Logo-Editor, iperf3-Kompatibilität, eigener Messserver.
