# Battlegrounds Companion

> Open-Source-, Linux-first-Companion für **Hearthstone Battlegrounds**.

**Sprachen:** [English](../../README.md) · [Deutsch](README.md)

Das ist **kein** Hearthstone-Deck-Tracker-Klon.  
Fokus: Spiel erkennen → Zustand sammeln → live analysieren → Overlay zeigen → lokal, nachvollziehbar, erweiterbar.

```bash
# Geplante UX
nix run github:fr4iser90/battlegrounds-companion
```

Hearthstone starten → Companion erkennt Battlegrounds → Overlay erscheint.

---

## Warum dieses Projekt?

[Hearthstone Deck Tracker (HDT)](https://hsdecktracker.net/) kann schon sehr viel. Wenn du nur „BG tracken + Overlay“ brauchst und HDT bei dir läuft: **nutze HDT**.

Dieses Projekt zielt auf eine andere Nische:

| Fokus | HDT | Dieser Companion |
| --- | --- | --- |
| Modus | Constructed + BG | **Battlegrounds zuerst** |
| Plattformen | Windows-first (sonst Wine/Proton) | **Native Linux / NixOS + cross-platform** |
| Overlay | Allgemeiner Tracker | **BG-optimiert, konfigurierbar** |
| Shop-Hilfe | Plugins / extern | **Pool, Triples, Draft, Tags, Meta-Comps** |
| Analyse | Plugins / Bob’s Buddy | **Gegner- & Combat-Tracking eingebaut** |
| Replays | Teilweise | **First-Class** |
| Erweiterbarkeit | Plugin-Ökosystem | **Offene Plugin-API von Anfang an** |
| Lizenz | Upstream prüfen | **MIT** |

Nur bauen, was HDT für Linux-/NixOS-BG-Spieler offenlässt — kein Feature-für-Feature-Remake.

---

## Status

**Pre-MVP / Dokumentationsphase.** Noch keine Gameplay-Pipeline.

Siehe (Englisch ist die Hauptquelle; DE folgt parallel):

- [Vision](VISION.md)
- [Architektur](ARCHITECTURE.md) → detailliert: [EN](../en/ARCHITECTURE.md)
- [MVP](MVP.md)
- [Roadmap](ROADMAP.md) → detailliert: [EN](../en/ROADMAP.md)
- [Compliance & Privatsphäre](COMPLIANCE.md) → detailliert: [EN](../en/COMPLIANCE.md)
- [Detection & Kartendaten](DETECTION.md) → detailliert: [EN](../en/DETECTION.md)
- [Contributing](../../CONTRIBUTING.md)

---

## Geplantes MVP (v0.1)

```text
Hearthstone erkennen
      ↓
Battlegrounds erkennen
      ↓
Live-Game-State lesen
      ↓
Overlay (Runde, Gold, HP, Taverne, Board, Shop)
      ↓
Match lokal speichern
      ↓
Einfache Match-History
```

---

## Privatsphäre

- Lokal zuerst
- Keine Battle.net-Zugangsdaten
- Keine Account-Auslese
- Telemetrie standardmäßig aus
- Klar dokumentieren, welche Spieldaten gelesen werden

Details: [COMPLIANCE.md](COMPLIANCE.md) und [EN](../en/COMPLIANCE.md).

---

## Hinweis zum Repository-Namen

Der GitHub-Remote heißt ggf. noch `HDT`. Produktname: **Battlegrounds Companion**. Rename ist geplant.

---

## Lizenz

[MIT](../../LICENSE) © 2026 Patrick B.
