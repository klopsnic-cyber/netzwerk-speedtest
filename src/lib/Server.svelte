<script>
  // Gegenstelle: Zustand liegt in der Hauptseite, damit sie beim Wechsel der Ansicht weiterläuft.
  import { Radio, Power, TriangleAlert } from "@lucide/svelte";

  let { info, log, error, onstart, onstop } = $props();
</script>

<div class="srv">
  {#if info}
    <div class="radar" aria-hidden="true">
      <span></span><span></span><span></span>
      <div class="core"><Radio size={34} /></div>
    </div>
    <h2>Gegenstelle bereit</h2>
    <p class="muted">Auf dem Messgerät „Intern (LAN)“ wählen und diesen Code eingeben:</p>
    <div class="code num">{info.code}</div>
    <div class="addr">
      {#each info.addresses as a}<div><span class="muted">Adresse</span> <b class="num">{a}</b></div>{/each}
      <div><span class="muted">Port</span> <b class="num">{info.port} (TCP/UDP)</b> · <span class="muted">Name</span> <b>{info.name}</b></div>
    </div>
    <div class="log card">
      <h3>Verbindungen</h3>
      {#if !log.length}<p class="muted">Noch keine – wartet auf das Messgerät …</p>{/if}
      {#each log.slice(-8).reverse() as l}<div class="num">{l}</div>{/each}
    </div>
    <button class="danger" onclick={onstop}><Power size={16} /> Gegenstelle beenden</button>
  {:else}
    <div class="intro card">
      <Radio size={40} />
      <h2>Dieses Gerät als Gegenstelle</h2>
      <p class="muted">
        Für den internen Test (LAN) braucht das Messgerät einen Partner im Netz. Starte hier die Gegenstelle, z. B. auf
        einem Praxis-PC am anderen Ende der Strecke. Beim ersten Start fragt die Firewall nach – bitte „Zulassen“ wählen
        (privates Netzwerk).
      </p>
      <button class="primary" onclick={onstart}><Power size={16} /> Gegenstelle starten</button>
      {#if error}<p class="warn"><TriangleAlert size={15} /> {error}</p>{/if}
    </div>
  {/if}
</div>

<style>
  .srv { display: flex; flex-direction: column; align-items: center; gap: 14px; text-align: center; padding: 10px 0 30px; }
  .radar { position: relative; width: 170px; height: 170px; display: grid; place-items: center; margin-top: 10px; }
  .radar span { position: absolute; inset: 0; border-radius: 50%; border: 2px solid var(--down); opacity: 0; animation: pulse 3s ease-out infinite; }
  .radar span:nth-child(2) { animation-delay: 1s; }
  .radar span:nth-child(3) { animation-delay: 2s; }
  .core { width: 76px; height: 76px; border-radius: 50%; display: grid; place-items: center; color: var(--down); background: rgba(34, 211, 238, 0.1); border: 1px solid var(--down); box-shadow: var(--glow); }
  @keyframes pulse { 0% { transform: scale(0.45); opacity: 0.9; } 100% { transform: scale(1.15); opacity: 0; } }
  .code { font-size: 64px; font-weight: 700; letter-spacing: 0.25em; color: var(--down); text-shadow: 0 0 30px rgba(34, 211, 238, 0.45); padding-left: 0.25em; }
  .addr { display: flex; flex-direction: column; gap: 4px; }
  .log { width: min(520px, 100%); text-align: left; font-size: 13px; }
  .log h3 { margin-bottom: 8px; }
  .log p { margin: 0; }
  .intro { max-width: 560px; display: flex; flex-direction: column; align-items: center; gap: 12px; padding: 30px; color: var(--down); }
  .intro p { margin: 0; }
  .warn { color: var(--warn); display: flex; gap: 6px; align-items: center; margin: 0; }
</style>
