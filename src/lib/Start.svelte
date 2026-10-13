<script>
  import { onMount } from "svelte";
  import { Cable, Wifi, Globe, Network, Play, Search, LoaderCircle, TriangleAlert, Radio } from "@lucide/svelte";
  import { api, de, errText } from "./api.js";

  let { settings, onstart } = $props();

  let scope = $state("external");
  let kind = $state("ethernet");
  let ifaces = $state([]);
  let chosen = $state({ ethernet: "", wifi: "" });
  let server = $state("");
  let code = $state("");
  let found = $state([]);
  let searching = $state(false);
  // svelte-ignore state_referenced_locally (Formular übernimmt bewusst nur den Startwert)
  let customer = $state(settings.customer || "");
  let room = $state("");
  // svelte-ignore state_referenced_locally (Formular übernimmt bewusst nur den Startwert)
  let planDown = $state(settings.plan_down ?? "");
  // svelte-ignore state_referenced_locally (Formular übernimmt bewusst nur den Startwert)
  let planUp = $state(settings.plan_up ?? "");
  let error = $state("");
  let inited = false; // Vorauswahl nur beim ersten Laden
  let searched = false; // automatische Suche nur einmal

  const ofKind = (k) => ifaces.filter((i) => i.kind === k);
  const pick = (k) => ofKind(k).find((i) => i.name === chosen[k]) || ofKind(k).find((i) => i.ipv4) || ofKind(k)[0];
  let current = $derived(pick(kind));
  let ready = $derived(!!current?.ipv4 && (scope === "external" || (server.trim() && /^\d{4}$/.test(code.trim()))));

  async function refresh() {
    try {
      ifaces = await api.interfaces();
      // Erststart: verbundene Standard-Schnittstelle vorauswählen
      if (!inited) {
        const def = ifaces.find((i) => i.is_default && i.ipv4) || ifaces.find((i) => i.ipv4);
        if (def && def.kind !== "other") kind = def.kind;
        inited = true;
      }
    } catch (e) {
      error = errText(e);
    }
  }

  async function search() {
    searching = true;
    try {
      found = await api.discover();
      if (found.length === 1 && !server) server = found[0].addr;
    } finally {
      searching = false;
    }
  }

  function start() {
    error = "";
    const num = (v) => (v === "" || v == null ? null : Number(String(v).replace(",", ".")) || null);
    onstart(
      {
        scope,
        iface: current.name,
        server: server.trim(),
        code: code.trim(),
        streams: settings.streams || 6,
        secs: settings.secs || 10,
      },
      {
        customer: customer.trim(),
        room: room.trim(),
        technician: settings.technician || "",
        notes: "",
        plan_down: scope === "external" ? num(planDown) : null,
        plan_up: scope === "external" ? num(planUp) : null,
      },
    );
  }

  onMount(() => {
    refresh();
    const t = setInterval(refresh, 3000);
    return () => clearInterval(t);
  });

  $effect(() => {
    if (scope === "internal" && !searched) {
      searched = true;
      search();
    }
  });

  function signalBars(rssi) {
    if (rssi == null) return 0;
    return rssi > -55 ? 4 : rssi > -65 ? 3 : rssi > -72 ? 2 : 1;
  }
</script>

<div class="start">
  <section>
    <h3>Was soll gemessen werden?</h3>
    <div class="choice two">
      <button class="opt" class:on={scope === "external"} onclick={() => (scope = "external")}>
        <Globe size={26} />
        <div><b>Internet</b><span>Geschwindigkeit des Anschlusses</span></div>
      </button>
      <button class="opt" class:on={scope === "internal"} onclick={() => (scope = "internal")}>
        <Network size={26} />
        <div><b>Intern (LAN)</b><span>zwischen zwei Geräten im Netz</span></div>
      </button>
    </div>
  </section>

  <section>
    <h3>Über welche Verbindung?</h3>
    <div class="choice two">
      {#each [["ethernet", "Netzwerkkabel", Cable], ["wifi", "WLAN", Wifi]] as [k, title, Icon]}
        {@const i = pick(k)}
        <button class="opt conn" class:on={kind === k} onclick={() => (kind = k)}>
          <Icon size={26} />
          <div class="grow">
            <b>{title}</b>
            {#if !i}
              <span class="off">kein Adapter gefunden</span>
            {:else if !i.ipv4}
              <span class="off">nicht verbunden</span>
            {:else if k === "wifi"}
              <span>
                {i.wifi?.ssid || "verbunden"}{i.wifi?.band ? ` · ${i.wifi.band}` : ""}{i.wifi?.channel ? ` · Kanal ${i.wifi.channel}` : ""}
              </span>
            {:else}
              <span>{i.link_mbps ? (i.link_mbps >= 1000 ? `${de(i.link_mbps / 1000)} Gbit/s` : `${i.link_mbps} Mbit/s`) : "verbunden"} · {i.ipv4}</span>
            {/if}
          </div>
          {#if k === "wifi" && i?.wifi?.rssi_dbm != null}
            <div class="bars" title="{i.wifi.rssi_dbm} dBm">
              {#each [1, 2, 3, 4] as b}<i class:lit={b <= signalBars(i.wifi.rssi_dbm)} style:height="{b * 4 + 2}px"></i>{/each}
              <small class="num">{i.wifi.rssi_dbm}</small>
            </div>
          {:else if i?.ipv4}
            <span class="dot"></span>
          {/if}
        </button>
      {/each}
    </div>
    {#if ofKind(kind).length > 1}
      <select bind:value={chosen[kind]} class="adapter">
        {#each ofKind(kind) as i}<option value={i.name}>{i.label}{i.ipv4 ? ` – ${i.ipv4}` : " – nicht verbunden"}</option>{/each}
      </select>
    {/if}
    {#if current && !current.ipv4}
      <p class="hint"><TriangleAlert size={15} /> {kind === "wifi" ? "WLAN ist nicht verbunden." : "Kein Netzwerkkabel verbunden."} Bitte verbinden oder die andere Verbindung wählen.</p>
    {:else if current && !current.is_default}
      <p class="hint info">Die Messung läuft fest über {current.label}, auch wenn {kind === "wifi" ? "ein Kabel" : "WLAN"} gleichzeitig verbunden ist.</p>
    {/if}
    {#if kind === "wifi" && current?.ipv4 && !current.wifi?.ssid}
      <p class="hint info">WLAN-Name unbekannt – macOS zeigt ihn nur mit Ortungsdienste-Freigabe (Systemeinstellungen → Datenschutz → Ortungsdienste).</p>
    {/if}
  </section>

  {#if scope === "internal"}
    <section>
      <h3>Gegenstelle</h3>
      <p class="muted small">Auf einem zweiten Gerät im Netz dieses Programm öffnen und „Gegenstelle“ starten. Dort steht der Code.</p>
      <div class="row wrap">
        {#each found as f}
          <button class="srv" class:on={server === f.addr} onclick={() => (server = f.addr)}><Radio size={15} /> {f.name} <small class="muted">{f.addr.split(":")[0]}</small></button>
        {/each}
        <button class="ghost" onclick={search} disabled={searching}>
          {#if searching}<LoaderCircle size={15} class="spin" /> Suche …{:else}<Search size={15} /> Gegenstellen suchen{/if}
        </button>
      </div>
      <div class="grid2">
        <div><label for="srv">IP-Adresse der Gegenstelle</label><input id="srv" placeholder="z. B. 192.168.1.20" bind:value={server} /></div>
        <div><label for="code">Code (4 Ziffern)</label><input id="code" class="num" inputmode="numeric" maxlength="4" placeholder="0000" bind:value={code} /></div>
      </div>
    </section>
  {:else}
    <section>
      <h3>Gebuchter Tarif <span class="opt-l">optional</span></h3>
      <div class="grid2">
        <div><label for="pd">Download (Mbit/s)</label><input id="pd" inputmode="decimal" placeholder="z. B. 250" bind:value={planDown} /></div>
        <div><label for="pu">Upload (Mbit/s)</label><input id="pu" inputmode="decimal" placeholder="z. B. 40" bind:value={planUp} /></div>
      </div>
    </section>
  {/if}

  <section>
    <h3>Wo wird gemessen?</h3>
    <div class="grid2">
      <div><label for="cu">Praxis / Kunde</label><input id="cu" placeholder="z. B. Praxis Dr. Müller" bind:value={customer} /></div>
      <div><label for="ro">Raum / Messpunkt</label><input id="ro" placeholder="z. B. Anmeldung" bind:value={room} /></div>
    </div>
  </section>

  {#if error}<p class="hint"><TriangleAlert size={15} /> {error}</p>{/if}

  <div class="go">
    <button class="primary big" disabled={!ready} onclick={start}><Play size={20} /> Test starten</button>
  </div>
</div>

<style>
  .start { max-width: 860px; margin: 0 auto; display: flex; flex-direction: column; gap: 22px; padding-bottom: 30px; }
  section { display: flex; flex-direction: column; gap: 10px; }
  .choice { display: grid; gap: 12px; }
  .two { grid-template-columns: 1fr 1fr; }
  .opt {
    justify-content: flex-start; text-align: left; padding: 16px 18px; border-radius: var(--r);
    background: rgba(255, 255, 255, 0.025); color: var(--muted); gap: 14px;
  }
  .opt b { display: block; color: var(--text); font-size: 15px; font-weight: 600; }
  .opt span { font-size: 12.5px; }
  .opt.on { border-color: var(--down); color: var(--down); background: rgba(34, 211, 238, 0.07); box-shadow: 0 0 0 1px var(--down), var(--glow); }
  .off { color: var(--warn); }
  .dot { width: 9px; height: 9px; border-radius: 50%; background: var(--good); box-shadow: 0 0 10px var(--good); }
  .bars { display: flex; align-items: flex-end; gap: 3px; }
  .bars i { width: 4px; background: #2a3550; border-radius: 1px; }
  .bars i.lit { background: var(--good); }
  .bars small { margin-left: 6px; font-size: 11px; color: var(--muted); }
  .adapter { max-width: 420px; }
  .hint { display: flex; gap: 8px; align-items: center; margin: 0; color: var(--warn); font-size: 13px; }
  .hint.info { color: var(--muted); }
  .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
  .row.wrap { flex-wrap: wrap; gap: 8px; }
  .srv.on { border-color: var(--down); color: var(--down); }
  .small { font-size: 12.5px; margin: 0; }
  .opt-l { text-transform: none; letter-spacing: 0; font-weight: 400; margin-left: 6px; }
  .go { display: flex; justify-content: center; margin-top: 6px; }
  .big { font-size: 17px; padding: 14px 34px; border-radius: 14px; }
  :global(.spin) { animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
