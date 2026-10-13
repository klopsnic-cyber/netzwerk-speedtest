<script>
  // Tacho mit logarithmischer Skala, Leuchtbogen und Partikeln an der Bogenspitze.
  // Zeichnet nur, solange sich etwas bewegt; während der Messung auf 30 fps gedrosselt,
  // damit die Darstellung der Messung keine Rechenzeit wegnimmt.
  import { onMount } from "svelte";

  let { value = 0, phase = null, big = "", unit = "", caption = "", reduce = false, measuring = false } = $props();

  const COLORS = { download: ["#22d3ee", "#0ea5c4"], upload: ["#a78bfa", "#7c3aed"], ping: ["#fbbf24", "#d97706"] };
  const BASE = [0, 1, 5, 10, 50, 100, 250, 500, 1000];

  let canvas;
  let shown = 0; // angezeigter (geglätteter) Wert
  let maxSeen = 0;
  let particles = [];
  let raf = 0;
  let last = 0;

  const ticks = () => (maxSeen > 1000 ? [...BASE, 2500, 5000, 10000] : BASE);

  function frac(v) {
    const t = ticks();
    if (v <= 0) return 0;
    for (let i = 0; i < t.length - 1; i++) {
      if (v < t[i + 1]) return (i + (v - t[i]) / (t[i + 1] - t[i])) / (t.length - 1);
    }
    return 1;
  }

  const START = Math.PI * 0.75;
  const SWEEP = Math.PI * 1.5;

  function draw(now) {
    raf = 0;
    const fps = measuring ? 30 : 60;
    if (now - last < 1000 / fps - 2) {
      raf = requestAnimationFrame(draw);
      return;
    }
    const dt = Math.min(0.1, (now - last) / 1000 || 0.016);
    last = now;

    const target = Math.max(0, value || 0);
    maxSeen = Math.max(maxSeen, target);
    shown = reduce ? target : shown + (target - shown) * Math.min(1, dt * 6);

    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth, h = canvas.clientHeight;
    if (canvas.width !== w * dpr) { canvas.width = w * dpr; canvas.height = h * dpr; }
    const ctx = canvas.getContext("2d");
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);

    const cx = w / 2, cy = h * 0.54, r = Math.min(w, h * 1.15) * 0.4;
    const [c1, c2] = COLORS[phase] || COLORS.download;

    // Spur
    ctx.lineCap = "round";
    ctx.lineWidth = 14;
    ctx.strokeStyle = "#1a2233";
    ctx.beginPath(); ctx.arc(cx, cy, r, START, START + SWEEP); ctx.stroke();

    // Skala
    const t = ticks();
    ctx.font = "500 11px Inter";
    ctx.textAlign = "center"; ctx.textBaseline = "middle";
    t.forEach((v, i) => {
      const a = START + SWEEP * (i / (t.length - 1));
      const x1 = cx + Math.cos(a) * (r - 18), y1 = cy + Math.sin(a) * (r - 18);
      const x2 = cx + Math.cos(a) * (r - 26), y2 = cy + Math.sin(a) * (r - 26);
      ctx.strokeStyle = "#2a3550"; ctx.lineWidth = 2;
      ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(x2, y2); ctx.stroke();
      ctx.fillStyle = "#5d6a85";
      const lx = cx + Math.cos(a) * (r - 42), ly = cy + Math.sin(a) * (r - 42);
      ctx.fillText(v >= 1000 ? `${v / 1000}G` : String(v), lx, ly);
    });

    // Wertbogen mit Leuchten
    const f = phase === "ping" ? 0 : frac(shown);
    const end = START + SWEEP * f;
    if (f > 0.002) {
      const g = ctx.createLinearGradient(cx - r, cy, cx + r, cy);
      g.addColorStop(0, c2); g.addColorStop(1, c1);
      ctx.strokeStyle = g; ctx.lineWidth = 14;
      if (!reduce) { ctx.shadowColor = c1; ctx.shadowBlur = 22; }
      ctx.beginPath(); ctx.arc(cx, cy, r, START, end); ctx.stroke();
      ctx.shadowBlur = 0;
      // Spitze
      const tx = cx + Math.cos(end) * r, ty = cy + Math.sin(end) * r;
      ctx.fillStyle = "#ffffff";
      ctx.beginPath(); ctx.arc(tx, ty, 4, 0, Math.PI * 2); ctx.fill();

      // Partikel: strömen von der Spitze nach außen, Tempo und Dichte folgen dem Durchsatz
      if (!reduce && measuring) {
        const spawn = Math.min(4, 1 + f * 4);
        for (let i = 0; i < spawn && particles.length < 60; i++) {
          const a = end + (Math.random() - 0.5) * 0.25;
          const sp = 30 + f * 140 + Math.random() * 40;
          particles.push({ x: tx, y: ty, vx: Math.cos(a) * sp + (Math.random() - 0.5) * 30, vy: Math.sin(a) * sp + (Math.random() - 0.5) * 30, life: 1 });
        }
      }
    }
    if (particles.length) {
      ctx.globalCompositeOperation = "lighter";
      particles = particles.filter((p) => (p.life -= dt * 1.4) > 0);
      for (const p of particles) {
        p.x += p.vx * dt; p.y += p.vy * dt;
        ctx.fillStyle = c1;
        ctx.globalAlpha = p.life * 0.8;
        ctx.beginPath(); ctx.arc(p.x, p.y, 1.2 + p.life * 1.6, 0, Math.PI * 2); ctx.fill();
      }
      ctx.globalAlpha = 1;
      ctx.globalCompositeOperation = "source-over";
    }

    const moving = Math.abs(target - shown) > 0.01 * Math.max(1, target) || particles.length > 0 || measuring;
    if (moving && !document.hidden) raf = requestAnimationFrame(draw);
  }

  function kick() {
    if (!raf && canvas) raf = requestAnimationFrame(draw);
  }

  $effect(() => {
    // bei neuem Wert, Phase oder Zustand neu zeichnen
    void value; void phase; void measuring; void reduce;
    if (phase === null) maxSeen = 0;
    kick();
  });

  onMount(() => {
    const ro = new ResizeObserver(kick);
    ro.observe(canvas);
    const vis = () => kick();
    document.addEventListener("visibilitychange", vis);
    return () => { ro.disconnect(); document.removeEventListener("visibilitychange", vis); cancelAnimationFrame(raf); };
  });
</script>

<div class="gauge">
  <canvas bind:this={canvas}></canvas>
  <div class="center">
    <div class="big num" style:color={phase ? COLORS[phase][0] : "var(--text)"}>{big}</div>
    <div class="unit">{unit}</div>
    {#if caption}<div class="cap">{caption}</div>{/if}
  </div>
</div>

<style>
  .gauge { position: relative; width: 100%; height: 100%; min-height: 300px; }
  canvas { position: absolute; inset: 0; width: 100%; height: 100%; }
  .center {
    position: absolute; left: 0; right: 0; top: 54%; transform: translateY(-50%);
    text-align: center; pointer-events: none;
  }
  .big { font-size: clamp(44px, 7vw, 84px); font-weight: 650; letter-spacing: -0.03em; line-height: 1; transition: color 0.4s; }
  .unit { color: var(--muted); font-size: 15px; margin-top: 6px; }
  .cap { color: var(--muted); font-size: 12px; margin-top: 14px; text-transform: uppercase; letter-spacing: 0.12em; }
</style>
