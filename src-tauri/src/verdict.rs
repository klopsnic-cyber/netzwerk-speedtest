//! Ampelbewertung mit Klartext-Empfehlungen (Schwellwerte siehe PLAN.md, Abschnitt 3).
//! Der schlechteste Einzelwert bestimmt die Gesamtfarbe.

use crate::model::{Finding, IfaceInfo, Measurement, Rating, Scope, Verdict};
use crate::report::de;

fn grade(v: f64, good: f64, warn: f64, higher_is_better: bool) -> Rating {
    let (g, w) = if higher_is_better { (v >= good, v >= warn) } else { (v < good, v < warn) };
    if g { Rating::Good } else if w { Rating::Warn } else { Rating::Bad }
}

fn f(rating: Rating, title: impl Into<String>, advice: impl Into<String>) -> Finding {
    Finding { rating, title: title.into(), advice: advice.into() }
}

pub fn evaluate(m: &Measurement) -> Verdict {
    let r = &m.results;
    let i: &IfaceInfo = &m.environment.iface;
    let wifi = i.kind == "wifi";
    let internal = m.config.scope == Scope::Internal;
    let down = r.download.median_mbps;
    let up = r.upload.median_mbps;
    let slower = down.min(up);
    let mut out: Vec<Finding> = Vec::new();
    let mut metrics = std::collections::BTreeMap::new();
    let mut tag = |key: &str, out: &Vec<Finding>| {
        if let Some(l) = out.last() {
            metrics.entry(key.to_string()).and_modify(|r: &mut Rating| *r = (*r).clone().max(l.rating.clone())).or_insert(l.rating.clone());
        }
    };

    // --- Durchsatz -------------------------------------------------------
    if internal && !wifi {
        if let Some(link) = i.link_mbps {
            let ratio = slower / link as f64;
            out.push(match grade(ratio, 0.9, 0.7, true) {
                // mehr als die Kabelrate geht nur, wenn die Daten das Kabel nie verlassen
                _ if ratio > 1.2 => f(Rating::Warn, "Messung lief nicht über das Netzwerk",
                    "Der Durchsatz liegt über der Kabelrate – die Gegenstelle läuft vermutlich auf diesem Gerät. Gegenstelle auf einem anderen PC starten."),
                Rating::Good => f(Rating::Good, format!("LAN-Durchsatz {:.0} % der Kabelrate", ratio * 100.0), "Die Kabelverbindung wird voll ausgenutzt."),
                rt => f(rt, format!("LAN-Durchsatz nur {:.0} % der Kabelrate", ratio * 100.0),
                    "Gegenstelle, Switch und Kabel prüfen; auch eine ausgelastete oder schwache Gegenstelle bremst die Messung."),
            });
            if link < 1000 {
                out.push(f(Rating::Warn, format!("Kabelverbindung nur {link} Mbit/s"),
                    "Kabel (mind. Cat 5e), Netzwerkdose und Switch-Port prüfen – Gigabit sollte heute Standard sein."));
            }
        } else {
            out.push(match grade(slower, 900.0, 500.0, true) {
                Rating::Good => f(Rating::Good, "LAN-Durchsatz gut", "Gigabit-Niveau erreicht."),
                rt => f(rt, format!("LAN-Durchsatz {slower:.0} Mbit/s"), "Kabel, Switch-Port und Gegenstelle prüfen."),
            });
        }
    } else if internal && wifi {
        let rating = match i.wifi.as_ref().and_then(|w| w.tx_rate_mbps) {
            Some(phy) => grade(slower / phy, 0.5, 0.3, true),
            None => grade(slower, 200.0, 50.0, true),
        };
        out.push(match rating {
            Rating::Good => f(Rating::Good, format!("WLAN-Durchsatz {slower:.0} Mbit/s"), "Für Praxisanwendungen ausreichend."),
            rt => f(rt, format!("WLAN-Durchsatz nur {slower:.0} Mbit/s"),
                "Access Point näher platzieren oder zusätzlichen Access Point einplanen; für feste Arbeitsplätze Kabel bevorzugen."),
        });
    } else {
        match (m.context.plan_down, m.context.plan_up) {
            (Some(pd), pu) if pd > 0.0 => {
                let rd = down / pd;
                let mut rating = grade(rd, 0.9, 0.6, true);
                if let Some(pu) = pu.filter(|x| *x > 0.0) {
                    rating = rating.max(grade(up / pu, 0.9, 0.6, true));
                }
                // Über WLAN liegt ein Rückstand oft am WLAN, nicht am Anschluss -> höchstens Gelb
                if wifi && rating == Rating::Bad {
                    rating = Rating::Warn;
                }
                out.push(match rating {
                    Rating::Good => f(Rating::Good, format!("Internet erreicht {:.0} % des Tarifs", rd * 100.0), "Der gebuchte Tarif kommt an."),
                    rt => f(rt, format!("Internet erreicht nur {:.0} % des Tarifs", rd * 100.0),
                        if wifi { "Zum Vergleich per Kabel am Router messen; bleibt der Wert niedrig, Störung beim Provider melden." }
                        else { "Mehrmals zu verschiedenen Zeiten messen; bleibt der Wert niedrig, Störung beim Provider melden." }),
                });
            }
            _ => {
                let rating = grade(down, 100.0, 25.0, true).max(grade(up, 20.0, 5.0, true));
                out.push(match rating {
                    Rating::Good => f(Rating::Good, "Internet-Bandbreite gut", "Ausreichend für Videosprechstunde, Cloud-Dienste und TI."),
                    rt => f(rt, format!("Internet {down:.0}/{up:.0} Mbit/s"),
                        "Für Videosprechstunde und Cloud-Dienste knapp; Tarif prüfen bzw. Upgrade beim Provider anfragen."),
                });
            }
        }
    }

    for x in out.clone() {
        tag("throughput", &vec![x]); // Durchsatz-Befunde (inkl. Kabelrate) -> schlechteste zählt
    }

    // --- Laufzeit, Jitter, Verlust ----------------------------------------
    let l = &r.latency;
    let (lg, lw, what) = match (internal, wifi) {
        (true, false) => (2.0, 10.0, "im Kabelnetz"),
        (true, true) => (10.0, 30.0, "im WLAN"),
        _ => (25.0, 60.0, "ins Internet"),
    };
    out.push(match grade(l.median_ms, lg, lw, false) {
        Rating::Good => f(Rating::Good, format!("Ping {} ms", de(l.median_ms)), format!("Antwortzeit {what} ist gut.")),
        rt => f(rt, format!("Ping {} ms {what} erhöht", de(l.median_ms)),
            if internal { "Netz auf Überlast, Schleifen oder fehlerhafte Komponenten prüfen." }
            else { "Hohe Antwortzeit stört Videosprechstunde und TI; Anschlussart/Provider prüfen." }),
    });
    tag("ping", &out);
    out.push(match grade(l.jitter_ms, 5.0, 20.0, false) {
        Rating::Good => f(Rating::Good, format!("Jitter {} ms", de(l.jitter_ms)), "Gleichmäßige Antwortzeiten."),
        rt => f(rt, format!("Jitter {} ms", de(l.jitter_ms)), "Schwankende Antwortzeiten stören Telefonie und Video; WLAN-Störungen oder Netzlast prüfen."),
    });
    tag("jitter", &out);
    out.push(match grade(l.loss_pct, 0.0001, 0.5, false) {
        Rating::Good => f(Rating::Good, "Kein Paketverlust", "Verbindung stabil."),
        rt => f(rt, format!("Paketverlust {} %", de(l.loss_pct)), "Verluste führen zu Abbrüchen; Kabel, WLAN-Signal und Router prüfen."),
    });
    tag("loss", &out);

    // --- Laufzeit unter Last (Bufferbloat, nur extern aussagekräftig) ------
    if !internal {
        let loaded = r.loaded_down_ms.unwrap_or(0.0).max(r.loaded_up_ms.unwrap_or(0.0));
        if loaded > 0.0 {
            let grow = (loaded - l.median_ms).max(0.0);
            out.push(match grade(grow, 30.0, 100.0, false) {
                Rating::Good => f(Rating::Good, "Router bleibt unter Last reaktionsschnell", format!("+{grow:.0} ms unter Last.")),
                rt => f(rt, format!("Ping steigt unter Last um {grow:.0} ms"),
                    "Bei voller Leitung stocken Video und Telefonie; am Router QoS/SQM aktivieren bzw. Bandbreite für Updates begrenzen."),
            });
            tag("bufferbloat", &out);
        }
    }

    // --- WLAN-Signal ------------------------------------------------------
    if let Some(w) = i.wifi.as_ref() {
        if let Some(rssi) = w.rssi_dbm {
            out.push(match grade(rssi as f64, -60.0, -70.0, true) {
                Rating::Good => f(Rating::Good, format!("WLAN-Signal stark ({rssi} dBm)"), "Guter Empfang am Messort."),
                rt => f(rt, format!("WLAN-Signal schwach ({rssi} dBm)"), "Access Point näher platzieren, Hindernisse prüfen oder zusätzlichen Access Point einplanen."),
            });
            tag("signal", &out);
        }
        if w.band.as_deref() == Some("2,4 GHz") {
            out.push(f(Rating::Warn, "WLAN im 2,4-GHz-Band", "5 GHz bevorzugen: schneller und weniger gestört (Bandsteering am Access Point aktivieren)."));
        }
    }

    let rating = out.iter().map(|x| x.rating.clone()).max().unwrap_or(Rating::Good);
    let summary = match rating {
        Rating::Good => "Gut – für den Praxisbetrieb geeignet",
        Rating::Warn => "Eingeschränkt – Verbesserung empfohlen",
        Rating::Bad => "Kritisch – Handlungsbedarf",
    }
    .to_string();
    // Schlechteste zuerst
    out.sort_by(|a, b| b.rating.cmp(&a.rating));
    Verdict { rating, summary, findings: out, metrics }
}
