<script>
  import { onMount } from "svelte";
  import { Activity, ArrowDown, ArrowUp, Check, X } from "@lucide/svelte";
  import Gauge from "./Gauge.svelte";
  import LiveChart from "./LiveChart.svelte";
  import { api, de, errText } from "./api.js";

  let { config, context, reduce = false, ondone, onerror, oncancel } = $props();

  const PHASES = [
    ["ping", "Ping", Activity],
    ["download", "Download", ArrowDown],
    ["upload", "Upload", ArrowUp],
  ];
  let phase = $state("ping");
  let done = $state(new Set());
  let down = $state([]);
  let up = $state([]);
  let pings = $state({ ping: [], download: [], upload: [] });
  let current = $state(0);
  let cancelling = $state(false);

  const med = (a) => {
    if (!a.length) return null;
    const s = [...a].sort((x, y) => x - y);
    const m = s.length >> 1;
    return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
  };
  const steady = (pts) => med(pts.filter((p) => p[0] >= 2).map((p) => p[1]));
  const jitter = (a) => (a.length < 2 ? null : a.slice(1).reduce((s, v, i) => s + Math.abs(v - a[i]), 0) / (a.length - 1));

  let idle = $derived(pings.ping.map((p) => p[1]));
  let pingMed = $derived(med(idle));

  function onEvent(e) {
    if (e.type === "phase") {
      if (phase !== e.phase) done = new Set([...done, phase]);
      phase = e.phase;
      current = 0;
    } else if (e.type === "throughput") {
      const p = [Math.round(e.t * 100) / 100, e.mbps];
      if (e.phase === "download") down = [...down, p];
      else up = [...up, p];
      current = e.mbps;
    } else if (e.type === "ping") {
      pings[e.phase] = [...pings[e.phase], [e.t, e.ms]];
      if (e.phase === "ping") current = med(pings.ping.map((p) => p[1]));
    }
  }

  async function cancel() {
    cancelling = true;
    await api.cancelTest();
  }

  onMount(() => {
    api
      .startTest(config, context, onEvent)
      .then((m) => ondone(m))
      .catch((e) => (cancelling ? oncancel() : onerror(errText(e))));
  });

  let big = $derived(phase === "ping" ? (pingMed == null ? "…" : de(pingMed)) : de(current));
  let unit = $derived(phase === "ping" ? "ms Ping" : "Mbit/s");
  let caption = $derived(
    config.scope === "external" ? "Internet · Cloudflare" : `Intern · ${config.server.split(":")[0]}`,
  );
</script>

<div class="live">
  <ol class="steps">
    {#each PHASES as [key, label, Icon], i}
      <li class:active={phase === key && !done.has(key)} class:done={done.has(key)}>
        <span class="ic">{#if done.has(key)}<Check size={15} />{:else}<Icon size={15} />{/if}</span>
        {label}
      </li>
      {#if i < PHASES.length - 1}<li class="bar" class:done={done.has(key)}></li>{/if}
    {/each}
  </ol>

  <div class="main">
    <div class="gauge-wrap">
      <Gauge value={phase === "ping" ? 0 : current} {phase} {big} {unit} {caption} {reduce} measuring={!cancelling} />
    </div>
    <div class="stats">
      <div class="stat"><span>Ping</span><b class="num" style:color="var(--ping)">{pingMed == null ? "–" : de(pingMed)}</b><small>ms</small></div>
      <div class="stat"><span>Jitter</span><b class="num">{jitter(idle) == null ? "–" : de(jitter(idle))}</b><small>ms</small></div>
      <div class="stat"><span>Download</span><b class="num" style:color="var(--down)">{steady(down) == null ? "–" : de(steady(down))}</b><small>Mbit/s</small></div>
      <div class="stat"><span>Upload</span><b class="num" style:color="var(--up)">{steady(up) == null ? "–" : de(steady(up))}</b><small>Mbit/s</small></div>
    </div>
  </div>

  <div class="charts card">
    <LiveChart
      height={150}
      xmax={config.secs}
      lines={[
        { label: "Download", color: "#22d3ee", points: down },
        { label: "Upload", color: "#a78bfa", points: up, dash: [6, 4] },
      ]}
    />
  </div>

  <div class="foot">
    <button class="danger" onclick={cancel} disabled={cancelling}><X size={16} /> {cancelling ? "Wird abgebrochen …" : "Abbrechen"}</button>
  </div>
</div>

<style>
  .live { display: flex; flex-direction: column; gap: 14px; height: 100%; }
  .steps { display: flex; align-items: center; justify-content: center; list-style: none; padding: 0; margin: 0; gap: 10px; color: var(--muted); }
  .steps li { display: flex; align-items: center; gap: 8px; font-weight: 500; }
  .steps .ic { width: 28px; height: 28px; border-radius: 50%; display: grid; place-items: center; border: 1px solid var(--border); background: var(--surface); }
  .steps li.active { color: var(--text); }
  .steps li.active .ic { border-color: var(--down); color: var(--down); box-shadow: var(--glow); }
  .steps li.done { color: var(--good); }
  .steps li.done .ic { border-color: var(--good); }
  .steps .bar { width: 60px; height: 2px; background: var(--border); border-radius: 2px; }
  .steps .bar.done { background: var(--good); }
  .main { display: grid; grid-template-columns: 1fr 220px; gap: 16px; flex: 1; min-height: 320px; }
  .gauge-wrap { min-height: 320px; }
  .stats { display: flex; flex-direction: column; gap: 10px; justify-content: center; }
  .stat { background: rgba(255, 255, 255, 0.03); border: 1px solid var(--border); border-radius: 12px; padding: 10px 14px; display: grid; grid-template-columns: 1fr auto; align-items: baseline; }
  .stat span { grid-column: 1 / -1; font-size: 11px; color: var(--muted); text-transform: uppercase; letter-spacing: 0.08em; }
  .stat b { font-size: 26px; font-weight: 650; }
  .stat small { color: var(--muted); margin-left: 6px; }
  .charts { padding: 10px 12px 8px; }
  .foot { display: flex; justify-content: center; }
</style>
