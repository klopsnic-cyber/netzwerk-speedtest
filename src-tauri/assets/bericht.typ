// Netzwerk-Messbericht. Daten kommen als JSON-Text in sys.inputs.daten (aufbereitet in report.rs).
#let d = json(bytes(sys.inputs.daten))

#let ink = rgb("#0B0F17")
#let muted = rgb("#5B6478")
#let line-c = rgb("#D8DEE9")
#let soft = rgb("#F3F5F9")
#let c-down = rgb("#0891B2")
#let c-up = rgb("#7C3AED")
#let c-ping = rgb("#D97706")
#let col = (good: rgb("#059669"), warn: rgb("#D97706"), bad: rgb("#DC2626"))
#let sym = (good: "✓", warn: "!", bad: "×")
#let word = (good: "Gut", warn: "Eingeschränkt", bad: "Kritisch")

#set document(title: "Netzwerk-Messbericht " + d.customer, author: d.company)
#set text(font: "Inter", size: 9.5pt, fill: ink, lang: "de")
#set par(leading: 0.62em)
#set page(
  paper: "a4",
  margin: (x: 18mm, top: 24mm, bottom: 20mm),
  header: context {
    if counter(page).get().first() > 1 {
      set text(8pt, fill: muted)
      grid(columns: (1fr, auto), [*#d.company* · Netzwerk-Messbericht · #d.customer], [#d.date])
      v(-4pt)
      line(length: 100%, stroke: 0.5pt + line-c)
    }
  },
  footer: context {
    set text(7.5pt, fill: muted)
    grid(columns: (1fr, auto),
      [Bericht #d.report_id · Netzwerk-Speedtest #d.app_version · Messwerte sind Momentaufnahmen],
      [Seite #counter(page).display("1 / 1", both: true)])
  },
)

// ---------- Bausteine ----------
#let badge(r, big: false) = box(
  fill: col.at(r).lighten(85%), stroke: 0.6pt + col.at(r), radius: 4pt,
  inset: if big { (x: 10pt, y: 6pt) } else { (x: 5pt, y: 2pt) },
  text(fill: col.at(r).darken(15%), weight: "semibold", size: if big { 12pt } else { 8pt })[#sym.at(r) #word.at(r)],
)

#let kachel(m, n) = box(
  width: 100%, height: 42pt, fill: soft, radius: 5pt, inset: 7pt, stroke: (left: 2.5pt + col.at(m.rating)),
  [
    #text(7pt, fill: muted, upper(m.label)) \
    #v(1fr)
    #box(text(if n > 5 { 13pt } else { 16pt }, weight: "bold", number-width: "tabular", m.value)) #box(text(7.5pt, fill: muted, m.unit))
  ],
)

#let befund(f) = grid(
  columns: (14pt, 1fr), column-gutter: 6pt, row-gutter: 3pt,
  text(fill: col.at(f.rating), weight: "bold", sym.at(f.rating)),
  [*#f.title* \ #text(fill: muted, f.advice)],
)

// Liniendiagramm: series = ((pts: ((t, v), …), color, label), …)
#let diagramm(series, unit, w: 174mm, hoehe: 42mm) = {
  let all = series.map(s => s.pts).flatten().chunks(2)
  if all.len() == 0 { return [] }
  let tmax = calc.max(..all.map(p => p.at(0)), 1)
  let vmax = calc.max(..all.map(p => p.at(1)), 0.001) * 1.12
  let x(t) = t / tmax * w
  let y(v) = hoehe - v / vmax * hoehe
  box(width: w, height: hoehe + 12pt, {
    for i in range(5) {
      let v = vmax * i / 4
      place(dx: 0pt, dy: y(v), line(length: w, stroke: 0.4pt + line-c))
      place(dx: 1pt, dy: y(v) - 8pt, text(6.5pt, fill: muted, str(calc.round(v, digits: if vmax < 10 { 1 } else { 0 })).replace(".", ",") + " " + unit))
    }
    for s in series {
      if s.pts.len() > 1 {
        let p = s.pts.map(p => (x(p.at(0)), y(p.at(1))))
        place(curve(stroke: 1.4pt + s.color, curve.move(p.first()), ..p.slice(1).map(q => curve.line(q))))
      }
    }
    place(dx: 0pt, dy: hoehe + 3pt, text(6.5pt, fill: muted)[0 s])
    place(dx: w - 14pt, dy: hoehe + 3pt, text(6.5pt, fill: muted)[#calc.round(tmax) s])
    place(dx: w / 2 - 40pt, dy: hoehe + 3pt, text(7pt, {
      for s in series [#box(width: 8pt, height: 2pt, fill: s.color, baseline: -2pt) #s.label #h(8pt)]
    }))
  })
}

// ---------- Seite 1: Zusammenfassung ----------
#grid(columns: (1fr, auto),
  [#text(9pt, fill: muted, upper(d.company)) \ #text(22pt, weight: "bold")[Netzwerk-Messbericht]],
  align(right, text(9pt, fill: muted)[#d.date \ Techniker: #d.technician]),
)
#v(4pt)
#text(13pt, weight: "semibold", d.customer)
#v(10pt)

#box(width: 100%, fill: col.at(d.rating).lighten(90%), radius: 8pt, inset: 14pt, stroke: 0.8pt + col.at(d.rating).lighten(40%), [
  #grid(columns: (auto, 1fr), column-gutter: 12pt, align: horizon,
    badge(d.rating, big: true),
    text(13pt, weight: "semibold", d.summary))
  #v(4pt)
  #text(fill: muted)[#d.points Messpunkt(e) · #d.scopes]
])

#v(12pt)
== Wichtigste Maßnahmen
#v(4pt)
#if d.top.len() == 0 [Keine Auffälligkeiten – das Netzwerk ist für den Praxisbetrieb geeignet.]
#for f in d.top { befund(f); v(5pt) }

#v(10pt)
== Übersicht der Messpunkte
#v(4pt)
#table(
  columns: (1.4fr, 1.5fr, 0.75fr, 0.7fr, 0.55fr, 0.75fr, 1.15fr),
  stroke: (x, y) => (bottom: 0.5pt + line-c),
  inset: (x: 5pt, y: 6pt),
  fill: (x, y) => if y == 0 { soft },
  table.header(..("Raum / Ort", "Verbindung", "Download", "Upload", "Ping", "Art", "Bewertung").map(h => text(7.5pt, weight: "semibold", fill: muted, h))),
  ..d.rows.map(r => (r.room, r.medium, r.down, r.up, r.ping, r.scope, badge(r.rating))).flatten(),
)
#text(7.5pt, fill: muted)[Download/Upload in Mbit/s (Median), Ping in ms.]

#if d.plan != none [
  #v(10pt)
  == Internet im Vergleich zum Tarif
  #v(4pt)
  #for b in d.plan [
    #grid(columns: (28mm, 1fr, 38mm), column-gutter: 8pt, align: horizon,
      text(8.5pt, b.label),
      box(width: 100%, height: 9pt, fill: soft, radius: 3pt,
        box(width: calc.min(b.pct, 100) * 1%, height: 9pt, fill: col.at(b.rating), radius: 3pt)),
      text(8.5pt)[*#b.pct %* von #b.plan Mbit/s])
    #v(3pt)
  ]
]

// ---------- je Messpunkt eine Seite ----------
#for p in d.pages [
  #pagebreak()
  #grid(columns: (1fr, auto), align: horizon,
    [#text(15pt, weight: "bold", p.heading) \ #text(fill: muted, p.subtitle)],
    badge(p.rating, big: true))
  #v(8pt)
  #grid(columns: (1fr,) * p.metrics.len(), column-gutter: 6pt, ..p.metrics.map(m => kachel(m, p.metrics.len())))
  #v(10pt)
  #text(weight: "semibold")[Durchsatz im Verlauf]
  #v(2pt)
  #diagramm(((pts: p.down, color: c-down, label: "Download"), (pts: p.up, color: c-up, label: "Upload")), "Mbit/s")
  #v(6pt)
  #text(weight: "semibold")[Antwortzeit (Ping) im Verlauf]
  #v(2pt)
  #diagramm(((pts: p.ping, color: c-ping, label: "Ping"),), "ms", hoehe: 26mm)
  #v(8pt)
  #grid(columns: (1fr, 1fr), column-gutter: 14pt,
    [
      #text(weight: "semibold")[Bewertung]
      #v(3pt)
      #for f in p.findings { befund(f); v(3pt) }
    ],
    [
      #text(weight: "semibold")[Messbedingungen]
      #v(3pt)
      #table(columns: (auto, 1fr), stroke: none, inset: (x: 0pt, y: 2.5pt), column-gutter: 8pt,
        ..p.info.map(kv => (text(fill: muted, kv.at(0)), kv.at(1))).flatten())
      #for w in p.warnings [#text(8pt, fill: col.warn)[⚠ #w] \ ]
      #if p.notes != "" [#v(3pt) #text(8pt)[*Notiz:* #p.notes]]
    ],
  )
]

// ---------- Anhang ----------
#pagebreak()
== So wurde gemessen
#v(4pt)
*Intern (LAN):* Auf einem zweiten Gerät im Netz läuft dasselbe Programm als Gegenstelle. Zuerst wird die Antwortzeit (Ping) per UDP gemessen, dann Download und Upload mit mehreren parallelen TCP-Verbindungen. Die ersten zwei Sekunden (Anlaufphase) werden verworfen; angegeben ist der Median der Sekundenwerte.

*Extern (Internet):* Gemessen wird gegen die Messserver von Cloudflare (nächstgelegenes Rechenzentrum). Zusätzlich wird die Antwortzeit während Download und Upload gemessen: Steigt sie stark an, stocken bei voller Leitung Video und Telefonie („Bufferbloat“).

*Kabel/WLAN:* Die Messung läuft fest über die ausgewählte Verbindung, auch wenn Kabel und WLAN gleichzeitig verbunden sind.

#v(8pt)
== Begriffe
#table(columns: (32mm, 1fr), stroke: (x, y) => (bottom: 0.5pt + line-c), inset: (x: 4pt, y: 5pt),
  [*Mbit/s*], [Megabit pro Sekunde – wie viele Daten pro Sekunde übertragen werden. 100 Mbit/s ≈ 12 Megabyte pro Sekunde.],
  [*Download / Upload*], [Empfangen bzw. Senden von Daten aus Sicht des Messgeräts.],
  [*Ping*], [Antwortzeit in Millisekunden (ms). Je kleiner, desto flüssiger reagieren Programme, Telefonie und Video.],
  [*Jitter*], [Schwankung der Antwortzeit. Hohe Werte führen zu abgehackter Sprache und ruckelndem Video.],
  [*Paketverlust*], [Anteil verlorener Datenpakete. Schon 1 % kann Verbindungsabbrüche verursachen.],
  [*WLAN-Signal (dBm)*], [Empfangsstärke: −50 sehr gut, −60 gut, −70 schwach, darunter unzuverlässig.],
)
#v(8pt)
#text(8pt, fill: muted)[Die Messwerte geben den Zustand zum Messzeitpunkt wieder und können je nach Tageszeit und Netzauslastung schwanken. Es wurden keine Patientendaten verarbeitet.]
