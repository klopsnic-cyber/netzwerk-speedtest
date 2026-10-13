<script>
  import { CircleCheck, CircleAlert, CircleX, FileDown, RotateCcw, TriangleAlert, Check, FolderOpen } from "@lucide/svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import LiveChart from "./LiveChart.svelte";
  import { api, de, medium, dateTime, ratingWord, errText } from "./api.js";

  let { measurement, settings, onnew, onchange } = $props();

  let m = $derived(measurement);
  let r = $derived(m.results);
  // svelte-ignore state_referenced_locally (Formular übernimmt bewusst nur den Startwert)
  let room = $state(measurement.context.room);
  // svelte-ignore state_referenced_locally (Formular übernimmt bewusst nur den Startwert)
  let notes = $state(measurement.context.notes);
  let busy = $state(false);
  let saved = $state("");
  let error = $state("");

  const ICON = { good: CircleCheck, warn: CircleAlert, bad: CircleX };
  const rating = (key) => m.verdict.metrics?.[key] || "good";

  let cards = $derived.by(() => {
    const c = [
      ["Download", r.download.median_mbps, "Mbit/s", "throughput", "var(--down)"],
      ["Upload", r.upload.median_mbps, "Mbit/s", "throughput", "var(--up)"],
      ["Ping", r.latency.median_ms, "ms", "ping", "var(--ping)"],
      ["Jitter", r.latency.jitter_ms, "ms", "jitter", null],
      ["Verlust", r.latency.loss_pct, "%", "loss", null],
    ];
    const loaded = Math.max(r.loaded_down_ms ?? 0, r.loaded_up_ms ?? 0);
    if (m.config.scope === "external" && loaded > 0) c.push(["Ping unter Last", loaded, "ms", "bufferbloat", null]);
    return c;
  });

  let plan = $derived(
    m.config.scope === "external"
      ? [["Download", r.download.median_mbps, m.context.plan_down], ["Upload", r.upload.median_mbps, m.context.plan_up]].filter((x) => x[2] > 0)
      : [],
  );

  async function keep() {
    if (room === m.context.room && notes === m.context.notes) return;
    try {
      onchange(await api.updateMeasurement(m.id, room, notes, m.context.customer));
    } catch (e) {
      error = errText(e);
    }
  }

  async function pdf() {
    error = "";
    await keep();
    const path = await save({
      defaultPath: await api.reportFileName(m.context.customer),
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (!path) return;
    busy = true;
    try {
      saved = await api.exportPdf([m.id], path, {
        company: settings.company,
        customer: m.context.customer,
        technician: m.context.technician || settings.technician,
        show_public_ip: settings.show_public_ip,
        show_ssid: settings.show_ssid,
      });
    } catch (e) {
      error = errText(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="result">
  <div class="banner card r-{m.verdict.rating}">
    {#if m.verdict.rating === "good"}<CircleCheck size={40} />{:else if m.verdict.rating === "warn"}<CircleAlert size={40} />{:else}<CircleX size={40} />{/if}
    <div class="grow">
      <h2>{m.verdict.summary}</h2>
      <div class="muted">
        {m.config.scope === "external" ? "Internet" : "Intern (LAN)"} · {medium(m.environment.iface)} · {m.environment.server}{m.environment.server_location ? ` · ${m.environment.server_location}` : ""} · {dateTime(m.started_at)}
      </div>
    </div>
  </div>

  <div class="cards" style:grid-template-columns="repeat({cards.length}, 1fr)">
    {#each cards as [label, v, unit, key, color]}
      {@const rt = rating(key)}
      {@const Icon = ICON[rt]}
      <div class="kpi r-{rt}">
        <span>{label}</span>
        <b class="num" style:color={color}>{de(v)}<small>{unit}</small></b>
        <em><Icon size={13} /> {ratingWord[rt]}</em>
      </div>
    {/each}
  </div>

  <div class="two">
    <div class="card">
      <h3>Durchsatz im Verlauf</h3>
      <LiveChart height={170} xmax={m.config.secs} lines={[
        { label: "Download", color: "#22d3ee", points: r.download.samples },
        { label: "Upload", color: "#a78bfa", points: r.upload.samples, dash: [6, 4] },
      ]} />
    </div>
    <div class="card">
      <h3>Ping in Ruhe</h3>
      <LiveChart height={170} unit="ms" lines={[{ label: "Ping", color: "#fbbf24", points: r.latency.samples }]} />
    </div>
  </div>

  {#if plan.length}
    <div class="card plan">
      <h3>Im Vergleich zum Tarif</h3>
      {#each plan as [label, got, p]}
        {@const pct = Math.round((got / p) * 100)}
        <div class="pbar">
          <span>{label}</span>
          <div class="track"><div class="fill r-{pct >= 90 ? 'good' : pct >= 60 ? 'warn' : 'bad'}" style:width="{Math.min(100, pct)}%"></div></div>
          <b class="num">{pct} %</b><small class="muted">von {de(p)} Mbit/s</small>
        </div>
      {/each}
    </div>
  {/if}

  <div class="two">
    <div class="card">
      <h3>Bewertung</h3>
      <ul class="findings">
        {#each m.verdict.findings as f}
          {@const Icon = ICON[f.rating]}
          <li class="r-{f.rating}"><Icon size={17} /><div><b>{f.title}</b><span>{f.advice}</span></div></li>
        {/each}
      </ul>
      {#each m.warnings as w}<p class="warn"><TriangleAlert size={15} /> {w}</p>{/each}
    </div>
    <div class="card meta">
      <h3>Zum Bericht</h3>
      <label for="room">Raum / Messpunkt</label>
      <input id="room" bind:value={room} onblur={keep} placeholder="z. B. Anmeldung" />
      <label for="notes">Notiz</label>
      <textarea id="notes" rows="3" bind:value={notes} onblur={keep} placeholder="z. B. Access Point im Flur, Tür geschlossen"></textarea>
      <div class="actions">
        <button class="primary" onclick={pdf} disabled={busy}><FileDown size={17} /> {busy ? "PDF wird erstellt …" : "PDF-Bericht"}</button>
        <button onclick={onnew}><RotateCcw size={16} /> Neue Messung</button>
      </div>
      {#if saved}
        <p class="ok"><Check size={15} /> Gespeichert. <button class="ghost link" onclick={() => revealItemInDir(saved)}><FolderOpen size={14} /> Im Ordner zeigen</button></p>
      {/if}
      {#if error}<p class="warn"><TriangleAlert size={15} /> {error}</p>{/if}
      <p class="muted tiny">Für mehrere Räume in einem Bericht: im Verlauf die Messungen auswählen.</p>
    </div>
  </div>
</div>

<style>
  .result { display: flex; flex-direction: column; gap: 14px; padding-bottom: 24px; }
  .banner { display: flex; gap: 16px; align-items: center; }
  .banner h2 { font-size: 22px; margin-bottom: 4px; color: var(--text); }
  .r-good { color: var(--good); } .r-warn { color: var(--warn); } .r-bad { color: var(--bad); }
  .banner.r-good { border-color: rgba(52, 211, 153, 0.5); background: rgba(52, 211, 153, 0.06); }
  .banner.r-warn { border-color: rgba(251, 191, 36, 0.5); background: rgba(251, 191, 36, 0.06); }
  .banner.r-bad { border-color: rgba(248, 113, 113, 0.5); background: rgba(248, 113, 113, 0.07); }
  .cards { display: grid; gap: 10px; }
  .kpi { background: rgba(255, 255, 255, 0.03); border: 1px solid var(--border); border-radius: 12px; padding: 12px 14px; border-left: 3px solid currentColor; display: flex; flex-direction: column; gap: 2px; }
  .kpi span { font-size: 11px; color: var(--muted); text-transform: uppercase; letter-spacing: 0.08em; }
  .kpi b { font-size: 26px; font-weight: 650; color: var(--text); }
  .kpi b small { font-size: 12px; color: var(--muted); margin-left: 5px; font-weight: 500; }
  .kpi em { font-style: normal; font-size: 12px; display: flex; align-items: center; gap: 5px; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
  .card h3 { margin-bottom: 10px; }
  .plan .pbar { display: grid; grid-template-columns: 90px 1fr 60px auto; gap: 12px; align-items: center; margin: 6px 0; }
  .track { height: 10px; background: var(--surface-2); border-radius: 6px; overflow: hidden; }
  .fill { height: 100%; border-radius: 6px; background: currentColor; box-shadow: 0 0 12px currentColor; transition: width 0.8s ease; }
  .findings { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 10px; }
  .findings li { display: flex; gap: 10px; align-items: flex-start; }
  .findings li :global(svg) { flex-shrink: 0; margin-top: 2px; }
  .findings b { display: block; color: var(--text); font-weight: 600; }
  .findings span { color: var(--muted); font-size: 13px; }
  .warn { color: var(--warn); display: flex; gap: 8px; align-items: flex-start; font-size: 13px; margin: 10px 0 0; }
  .ok { color: var(--good); display: flex; gap: 6px; align-items: center; font-size: 13px; margin: 10px 0 0; }
  .meta label { margin-top: 8px; }
  textarea { resize: vertical; }
  .actions { display: flex; gap: 10px; margin-top: 14px; flex-wrap: wrap; }
  .link { padding: 2px 6px; border: none; color: var(--down); }
  .tiny { font-size: 12px; margin: 12px 0 0; }
</style>
