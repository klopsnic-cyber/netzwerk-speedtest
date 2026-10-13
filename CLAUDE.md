# Netzwerk-Speedtest – Hinweise für Claude

- Plan und Messmethodik: `PLAN.md`. Tauri 2 + SvelteKit (Svelte 5), Rust-Kern in `src-tauri/src/`.
- Tests: `cd src-tauri && cargo test` (Loopback-Test startet echte Gegenstelle auf 127.0.0.1:52999). `npx svelte-check` fürs Frontend.
- Windows-Code (`iface.rs`, `net.rs` cfg(windows)) lässt sich auf dem Mac nicht prüfen (ring braucht Windows-C-Header) – das macht der Windows-Job in Actions.
- Release: `./release.sh X.Y.Z "Was ist neu"` (Version an 3 Stellen, CHANGELOG, Tag, Push). Actions baut, Apps prüfen beim Start + alle 4 h und zeigen ein Update-Banner.
- Dev-Test: `SPEEDTEST_AUTOSTART=external` oder `internal:IP:CODE npm run tauri dev` (nur Debug). Headless-Gegenstelle: `cargo run --example gegenstelle 4711`.
- Cloudflare drosselt mit HTTP 429 bei vielen Tests hintereinander – dann ein paar Minuten warten.
