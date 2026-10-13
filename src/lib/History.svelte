<script>
  import { onMount } from "svelte";
  import { FileDown, Trash2, Search, Check, FolderOpen, TriangleAlert, Cable, Wifi, Globe, Network } from "@lucide/svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api, de, dateTime, ratingWord, errText } from "./api.js";

  let { settings, onopen } = $props();

  let items = $state([]);
  let q = $state("");
  let picked = $state(new Set());
  let busy = $state(false);
  let saved = $state("");
  let error = $state("");

  async function load() {
    items = await api.history();
  }
  onMount(load);

  let shown = $derived(
    items.filter((m) => !q.trim() || `${m.context.customer} ${m.context.room}`.toLowerCase().includes(q.trim().toLowerCase())),
  );

  function toggle(id) {
    const s = new Set(picked);
    s.has(id) ? s.delete(id) : s.add(id);
    picked = s;
  }

  async function report() {
    error = "";
    const sel = items.filter((m) => picked.has(m.id));
    const customer = sel[0]?.context.customer || "";
    const path = await save({ defaultPath: await api.reportFileName(customer), filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (!path) return;
    busy = true;
    try {
      saved = await api.exportPdf([...picked], path, {
        company: settings.company,
        customer,
        technician: settings.technician,
        show_public_ip: settings.show_public_ip,
        show_ssid: settings.show_ssid,
      });
    } catch (e) {
      error = errText(e);
    } finally {
      busy = false;
    }
  }

  async function remove() {
    for (const id of picked) await api.deleteMeasurement(id);
    picked = new Set();
    await load();
  }
</script>

<div class="hist">
  <div class="bar">
    <div class="search"><Search size={15} /><input placeholder="Praxis oder Raum suchen" bind:value={q} /></div>
    <span class="muted">{picked.size} ausgewählt</span>
    <button class="primary" disabled={!picked.size || busy} onclick={report}><FileDown size={16} /> {busy ? "Erstelle …" : "Bericht aus Auswahl"}</button>
    <button class="danger" disabled={!picked.size} onclick={remove}><Trash2 size={15} /> Löschen</button>
  </div>
  {#if saved}<p class="ok"><Check size={15} /> PDF gespeichert. <button class="ghost link" onclick={() => revealItemInDir(saved)}><FolderOpen size={14} /> Im Ordner zeigen</button></p>{/if}
  {#if error}<p class="warn"><TriangleAlert size={15} /> {error}</p>{/if}

  {#if !items.length}
    <div class="empty card">Noch keine Messungen. Starte unter „Messen“ den ersten Test.</div>
  {:else}
    <table>
      <thead>
        <tr><th></th><th>Datum</th><th>Praxis</th><th>Raum</th><th>Art</th><th>Verbindung</th><th class="n">Download</th><th class="n">Upload</th><th class="n">Ping</th><th>Bewertung</th></tr>
      </thead>
      <tbody>
        {#each shown as m (m.id)}
          {@const wifi = m.environment.iface.kind === "wifi"}
          <tr class:sel={picked.has(m.id)} onclick={() => onopen(m)}>
            <td onclick={(e) => { e.stopPropagation(); toggle(m.id); }}><span class="box" class:on={picked.has(m.id)}>{#if picked.has(m.id)}<Check size={12} />{/if}</span></td>
            <td class="muted">{dateTime(m.started_at)}</td>
            <td>{m.context.customer || "–"}</td>
            <td>{m.context.room || "–"}</td>
            <td>{#if m.config.scope === "external"}<Globe size={14} /> Internet{:else}<Network size={14} /> LAN{/if}</td>
            <td>{#if wifi}<Wifi size={14} /> WLAN{:else}<Cable size={14} /> Kabel{/if}</td>
            <td class="n num" style:color="var(--down)">{de(m.results.download.median_mbps)}</td>
            <td class="n num" style:color="var(--up)">{de(m.results.upload.median_mbps)}</td>
            <td class="n num">{de(m.results.latency.median_ms)}</td>
            <td><span class="pill r-{m.verdict.rating}">{ratingWord[m.verdict.rating]}</span></td>
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="muted tiny">Zeile anklicken = Ergebnis öffnen · Kästchen = für den Bericht auswählen (mehrere Räume ergeben einen Gesamtbericht).</p>
  {/if}
</div>

<style>
  .hist { display: flex; flex-direction: column; gap: 12px; }
  .bar { display: flex; gap: 10px; align-items: center; }
  .search { flex: 1; display: flex; align-items: center; gap: 8px; background: var(--surface); border: 1px solid var(--border); border-radius: 9px; padding: 0 10px; color: var(--muted); }
  .search input { border: none; background: transparent; box-shadow: none; padding: 8px 0; }
  table { width: 100%; border-collapse: collapse; font-size: 13px; }
  th { text-align: left; font-size: 11px; color: var(--muted); text-transform: uppercase; letter-spacing: 0.06em; font-weight: 600; padding: 8px; border-bottom: 1px solid var(--border); }
  td { padding: 9px 8px; border-bottom: 1px solid #1c2436; white-space: nowrap; }
  td :global(svg) { vertical-align: -2px; margin-right: 3px; }
  tr { cursor: pointer; transition: background 0.15s; }
  tbody tr:hover { background: rgba(255, 255, 255, 0.03); }
  tr.sel { background: rgba(34, 211, 238, 0.06); }
  .n { text-align: right; }
  .box { width: 16px; height: 16px; border: 1px solid var(--border); border-radius: 4px; display: grid; place-items: center; }
  .box.on { background: var(--down); border-color: var(--down); color: #04131a; }
  .r-good { color: var(--good); } .r-warn { color: var(--warn); } .r-bad { color: var(--bad); }
  .empty { text-align: center; color: var(--muted); padding: 40px; }
  .ok { color: var(--good); display: flex; gap: 6px; align-items: center; margin: 0; font-size: 13px; }
  .warn { color: var(--warn); display: flex; gap: 6px; align-items: center; margin: 0; font-size: 13px; }
  .link { padding: 2px 6px; border: none; color: var(--down); }
  .tiny { font-size: 12px; margin: 0; }
</style>
