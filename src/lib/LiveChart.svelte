<script>
  // Echtzeit-Liniendiagramm (uPlot, Canvas). lines = [{ label, color, points: [[t, v], …], dash }]
  import { onMount } from "svelte";
  import uPlot from "uplot";
  import "uplot/dist/uPlot.min.css";
  import { de } from "./api.js";

  let { lines = [], unit = "Mbit/s", height = 180, xmax = null } = $props();

  let el;
  let plot;

  function data() {
    // gemeinsame Zeitachse aus allen Reihen; fehlende Werte = null (Lücke)
    const xs = [...new Set(lines.flatMap((l) => l.points.map((p) => p[0])))].sort((a, b) => a - b);
    const cols = lines.map((l) => {
      const m = new Map(l.points.map((p) => [p[0], p[1]]));
      return xs.map((x) => (m.has(x) ? m.get(x) : null));
    });
    return [xs, ...cols];
  }

  function fill(color) {
    return (u) => {
      const g = u.ctx.createLinearGradient(0, u.bbox.top, 0, u.bbox.top + u.bbox.height);
      g.addColorStop(0, color + "55");
      g.addColorStop(1, color + "00");
      return g;
    };
  }

  function build() {
    plot?.destroy();
    const axis = { stroke: "#7d8aa5", grid: { stroke: "#1f2840", width: 1 }, ticks: { show: false }, font: "11px Inter" };
    plot = new uPlot(
      {
        width: el.clientWidth,
        height,
        legend: { show: false },
        cursor: { show: false },
        scales: { x: { time: false, range: (u, a, b) => [0, Math.max(xmax ?? 0, b || 1)] }, y: { range: (u, a, b) => [0, Math.max(1, (b || 1) * 1.15)] } },
        axes: [
          { ...axis, values: (u, v) => v.map((x) => `${de(x)} s`) },
          { ...axis, size: (u, vals) => Math.max(64, ...(vals || []).map((t) => t.length * 7 + 16)), values: (u, v) => v.map((x) => `${de(x)} ${unit}`) },
        ],
        series: [
          {},
          ...lines.map((l) => ({ stroke: l.color, width: 2, fill: fill(l.color), dash: l.dash, spanGaps: true, points: { show: false } })),
        ],
      },
      data(),
      el,
    );
  }

  $effect(() => {
    // Struktur geändert (andere Reihen) -> neu aufbauen, sonst nur Daten tauschen
    const key = lines.map((l) => l.label).join("|");
    void lines.map((l) => l.points.length);
    if (!el) return;
    if (!plot || plot._key !== key) {
      build();
      plot._key = key;
    } else {
      plot.setData(data());
    }
  });

  onMount(() => {
    const ro = new ResizeObserver(() => plot?.setSize({ width: el.clientWidth, height }));
    ro.observe(el);
    return () => { ro.disconnect(); plot?.destroy(); };
  });
</script>

<div class="chart" bind:this={el} style:height="{height}px"></div>
<div class="legend">
  {#each lines as l}
    <span><i style:background={l.dash ? "none" : l.color} style:border-top={l.dash ? `3px dashed ${l.color}` : null}></i>{l.label}</span>
  {/each}
</div>

<style>
  .chart { width: 100%; }
  .legend { display: flex; gap: 16px; justify-content: center; font-size: 12px; color: var(--muted); margin-top: 4px; }
  .legend i { display: inline-block; width: 14px; height: 3px; border-radius: 2px; margin-right: 6px; vertical-align: middle; }
  :global(.u-wrap) { font-family: Inter, sans-serif; }
</style>
