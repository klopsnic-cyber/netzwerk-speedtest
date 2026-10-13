# Netzwerk-Speedtest – Hinweise für Claude

- Plan und Messmethodik: `PLAN.md`. Tauri 2 + SvelteKit (Svelte 5), Rust-Kern in `src-tauri/src/`.
- Tests: `cd src-tauri && cargo test` (Loopback-Test startet echte Gegenstelle auf 127.0.0.1:52999). `npx svelte-check` fürs Frontend.
- Windows-Code (`iface.rs`, `net.rs` cfg(windows)) lässt sich auf dem Mac nicht prüfen (ring braucht Windows-C-Header) – das macht der Windows-Job in Actions.
- Release: Version an 3 Stellen (package.json, tauri.conf.json, Cargo.toml) + CHANGELOG, dann Tag `vX.Y.Z` pushen. Updater-Schlüssel liegt in `~/.tauri/netzwerk-speedtest.key` (+ `.passwort`), in den Repo-Secrets `TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)`. Schlüssel NIE ausgeben oder committen. Ohne Schlüssel keine Auto-Updates mehr.
- Dev-Test: `SPEEDTEST_AUTOSTART=external` oder `internal:IP:CODE npm run tauri dev` (nur Debug). Headless-Gegenstelle: `cargo run --example gegenstelle 4711`.
- Cloudflare drosselt mit HTTP 429 bei vielen Tests hintereinander – dann ein paar Minuten warten.
