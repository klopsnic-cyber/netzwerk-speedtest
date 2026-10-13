<script>
  import "$lib/theme.css";
  import { onMount } from "svelte";
  import { Gauge as GaugeIcon, History as HistoryIcon, Radio, Settings as SettingsIcon, Download, TriangleAlert, X } from "@lucide/svelte";
  import { check } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import Start from "$lib/Start.svelte";
  import Live from "$lib/Live.svelte";
  import Result from "$lib/Result.svelte";
  import History from "$lib/History.svelte";
  import Server from "$lib/Server.svelte";
  import Settings from "$lib/Settings.svelte";
  import { api, errText } from "$lib/api.js";
  import { invoke } from "@tauri-apps/api/core";

  let view = $state("start"); // start | live | result | history | server | settings
  let settings = $state(null);
  let version = $state("");
  let run = $state(null); // { config, context } während des Tests
  let measurement = $state(null);
  let error = $state("");
  let update = $state(null);
  let updating = $state(false);
  let updateMsg = $state("");
  let srv = $state({ info: null, log: [], error: "" });
  let reduce = $derived(settings?.reduce_effects || matchMedia("(prefers-reduced-motion: reduce)").matches);

  const NAV = [
    ["start", "Messen", GaugeIcon],
    ["history", "Verlauf", HistoryIcon],
    ["server", "Gegenstelle", Radio],
    ["settings", "Einstellungen", SettingsIcon],
  ];

  onMount(async () => {
    const log = (m) => invoke("frontend_log", { msg: String(m) }).catch(() => {});
    window.addEventListener("error", (e) => log(`${e.message} @ ${e.filename}:${e.lineno}`));
    window.addEventListener("unhandledrejection", (e) => log(`Promise: ${e.reason?.stack || e.reason}`));
    settings = await api.settings();
    version = await api.version();
    checkUpdate(true);
    devAutostart();
  });

  async function devAutostart() {
    const a = await invoke("dev_autostart");
    if (!a) return;
    const [scope, ip, code] = a.split(":");
    const ifaces = await api.interfaces();
    const i = ifaces.find((x) => x.is_default && x.ipv4) || ifaces.find((x) => x.ipv4);
    startTest(
      { scope, iface: i.name, server: ip || "", code: code || "", streams: settings.streams, secs: settings.secs },
      { customer: "Praxis Test", room: "Anmeldung", technician: settings.technician, notes: "", plan_down: scope === "external" ? 250 : null, plan_up: scope === "external" ? 40 : null },
    );
  }

  async function checkUpdate(silent) {
    updateMsg = "";
    try {
      update = await check();
      if (!update && !silent) updateMsg = `Version ${version} ist aktuell.`;
    } catch (e) {
      if (!silent) updateMsg = `Update-Prüfung fehlgeschlagen: ${errText(e)}`;
    }
  }

  async function installUpdate() {
    updating = true;
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (e) {
      updateMsg = `Update fehlgeschlagen: ${errText(e)}`;
      updating = false;
    }
  }

  function startTest(config, context) {
    error = "";
    run = { config, context };
    view = "live";
  }

  async function startServer() {
    srv.error = "";
    try {
      srv.info = await api.startServer((e) => {
        const what = { download: "misst Download", upload: "misst Upload", verbunden: "verbunden", abgewiesen: "falscher Code – abgewiesen" }[e.kind] || e.kind;
        srv.log = [...srv.log, `${new Date().toLocaleTimeString("de-DE")}  ${e.peer}  ${what}`];
      });
    } catch (e) {
      srv.error = errText(e);
    }
  }

  async function stopServer() {
    await api.stopServer();
    srv = { info: null, log: [], error: "" };
  }

  function go(v) {
    if (view === "live") return; // während der Messung nicht wegnavigieren
    view = v;
  }
</script>

<div class="app">
  <header>
    <div class="brand">
      <span class="logo"><GaugeIcon size={18} /></span>
      <b>Netzwerk-Speedtest</b>
    </div>
    <nav>
      {#each NAV as [key, label, Icon]}
        <button class="ghost" class:on={view === key || (key === "start" && (view === "live" || view === "result"))} disabled={view === "live" && key !== "start"} onclick={() => go(key)}>
          <Icon size={16} /> {label}
          {#if key === "server" && srv.info}<span class="live-dot" title="Gegenstelle aktiv"></span>{/if}
        </button>
      {/each}
    </nav>
  </header>

  {#if update}
    <div class="update">
      <Download size={16} /> Version {update.version} ist verfügbar.
      <button class="primary small" disabled={updating} onclick={installUpdate}>{updating ? "Wird installiert …" : "Jetzt aktualisieren"}</button>
      <button class="ghost small" onclick={() => (update = null)}><X size={14} /></button>
    </div>
  {/if}
  {#if updateMsg}<div class="update muted">{updateMsg} <button class="ghost small" onclick={() => (updateMsg = "")}><X size={14} /></button></div>{/if}

  <main>
    {#if !settings}
      <p class="muted">Lade …</p>
    {:else if view === "start"}
      {#if error}<div class="err card"><TriangleAlert size={18} /> {error}</div>{/if}
      <Start {settings} onstart={startTest} />
    {:else if view === "live"}
      <Live
        config={run.config}
        context={run.context}
        {reduce}
        ondone={(m) => { measurement = m; view = "result"; }}
        onerror={(msg) => { error = msg; view = "start"; }}
        oncancel={() => (view = "start")}
      />
    {:else if view === "result" && measurement}
      <Result {measurement} {settings} onnew={() => (view = "start")} onchange={(m) => (measurement = m)} />
    {:else if view === "history"}
      <History {settings} onopen={(m) => { measurement = m; view = "result"; }} />
    {:else if view === "server"}
      <Server info={srv.info} log={srv.log} error={srv.error} onstart={startServer} onstop={stopServer} />
    {:else if view === "settings"}
      <Settings {settings} {version} onsaved={(s) => (settings = s)} oncheckupdate={() => checkUpdate(false)} />
    {/if}
  </main>
</div>

<style>
  .app { height: 100vh; display: flex; flex-direction: column; }
  header {
    display: flex; align-items: center; justify-content: space-between; padding: 12px 22px;
    border-bottom: 1px solid #1a2233; background: rgba(11, 15, 23, 0.7); backdrop-filter: blur(10px);
  }
  .brand { display: flex; align-items: center; gap: 10px; font-size: 15px; }
  .logo { width: 30px; height: 30px; border-radius: 9px; display: grid; place-items: center; color: #04131a; background: linear-gradient(135deg, #22d3ee, #a78bfa); box-shadow: var(--glow); }
  nav { display: flex; gap: 4px; }
  nav button { border: 1px solid transparent; color: var(--muted); position: relative; }
  nav button.on { color: var(--text); background: var(--surface-2); border-color: var(--border); }
  .live-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--good); box-shadow: 0 0 8px var(--good); animation: blink 1.6s ease-in-out infinite; }
  @keyframes blink { 50% { opacity: 0.35; } }
  main { flex: 1; overflow: auto; padding: 22px 26px; }
  .update { display: flex; align-items: center; gap: 10px; justify-content: center; padding: 8px; background: rgba(34, 211, 238, 0.08); border-bottom: 1px solid rgba(34, 211, 238, 0.3); font-size: 13px; }
  .small { padding: 4px 10px; font-size: 12.5px; }
  .err { max-width: 860px; margin: 0 auto 18px; display: flex; gap: 10px; align-items: center; color: var(--bad); border-color: rgba(248, 113, 113, 0.5); background: rgba(248, 113, 113, 0.07); }
</style>
