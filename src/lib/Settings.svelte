<script>
  import { Save, Check, RefreshCw } from "@lucide/svelte";
  import { api, errText } from "./api.js";

  let { settings, version, onsaved, oncheckupdate } = $props();

  // svelte-ignore state_referenced_locally (Formular übernimmt bewusst nur den Startwert)
  let s = $state(structuredClone($state.snapshot(settings)));
  let ok = $state(false);
  let error = $state("");

  async function store() {
    error = "";
    const num = (v) => (v === "" || v == null ? null : Number(String(v).replace(",", ".")) || null);
    const next = { ...s, plan_down: num(s.plan_down), plan_up: num(s.plan_up), streams: Math.min(16, Math.max(1, Number(s.streams) || 6)), secs: Math.min(30, Math.max(5, Number(s.secs) || 10)) };
    try {
      await api.saveSettings(next);
      onsaved(next);
      ok = true;
      setTimeout(() => (ok = false), 2000);
    } catch (e) {
      error = errText(e);
    }
  }
</script>

<div class="set">
  <div class="card">
    <h3>Bericht</h3>
    <div class="grid">
      <div><label for="co">Firma (Kopf des PDF)</label><input id="co" bind:value={s.company} /></div>
      <div><label for="te">Techniker</label><input id="te" bind:value={s.technician} placeholder="Name im Bericht" /></div>
      <div><label for="cu">Praxis / Kunde (Vorgabe)</label><input id="cu" bind:value={s.customer} /></div>
    </div>
    <label class="check"><input type="checkbox" bind:checked={s.show_public_ip} /> Öffentliche IP-Adresse im PDF zeigen</label>
    <label class="check"><input type="checkbox" bind:checked={s.show_ssid} /> WLAN-Namen (SSID) im PDF zeigen</label>
  </div>

  <div class="card">
    <h3>Internet-Tarif (Vorgabe)</h3>
    <div class="grid">
      <div><label for="pd">Download (Mbit/s)</label><input id="pd" inputmode="decimal" bind:value={s.plan_down} /></div>
      <div><label for="pu">Upload (Mbit/s)</label><input id="pu" inputmode="decimal" bind:value={s.plan_up} /></div>
    </div>
  </div>

  <div class="card">
    <h3>Messung</h3>
    <div class="grid">
      <div><label for="st">Parallele Verbindungen (1–16)</label><input id="st" type="number" min="1" max="16" bind:value={s.streams} /></div>
      <div><label for="se">Dauer je Richtung in Sekunden (5–30)</label><input id="se" type="number" min="5" max="30" bind:value={s.secs} /></div>
    </div>
    <label class="check"><input type="checkbox" bind:checked={s.reduce_effects} /> Effekte reduzieren (für langsame Geräte, ruhigere Anzeige)</label>
  </div>

  <div class="row">
    <button class="primary" onclick={store}><Save size={16} /> Speichern</button>
    {#if ok}<span class="ok"><Check size={15} /> Gespeichert</span>{/if}
    {#if error}<span class="warn">{error}</span>{/if}
    <span class="grow"></span>
    <span class="muted">Version {version}</span>
    <button class="ghost" onclick={oncheckupdate}><RefreshCw size={15} /> Nach Updates suchen</button>
  </div>
</div>

<style>
  .set { max-width: 760px; margin: 0 auto; display: flex; flex-direction: column; gap: 14px; }
  .card h3 { margin-bottom: 12px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 12px; margin-bottom: 8px; }
  .check { display: flex; align-items: center; gap: 8px; color: var(--text); font-size: 13.5px; margin-top: 8px; cursor: pointer; }
  .check input { width: auto; accent-color: var(--down); }
  .ok { color: var(--good); display: flex; gap: 6px; align-items: center; }
  .warn { color: var(--warn); }
</style>
