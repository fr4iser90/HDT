# Architektur

Geplant. Sprachen und Packaging können sich nach dem Detection-Spike ändern.

## Prinzipien

1. **Local-first**
2. **Event-getrieben**
3. **Datengetriebene Karten** (keine hardcodierten Minion-Namen in der Core-Logik)
4. **Dünnes MVP** vor Plugins/Dashboard
5. **Austauschbare Collector** (falls ToS/Technik es erzwingen)

## Grobe Struktur

```text
apps/desktop + apps/overlay
core/ (game, state, tracking, analysis, events, storage)
data/ (cards, heroes, tribes, mechanics, patches)
plugins/
ui/
tests/
docs/en + docs/de
```

## Runtime

```text
Collector ──▶ Event-Bus ──▶ State Store
                    ├── Overlay
                    ├── Storage / Replays
                    ├── Analyzer
                    └── Plugins
```

## Tech-Richtung (zu validieren)

| Schicht | Wahrscheinlich | Warum |
| --- | --- | --- |
| Collector / Core | Rust (oder C#) | Performance, Packaging |
| Overlay / Desktop | Tauri o. ä. | Linux + Web-UI |
| Dashboard | TypeScript + React/Svelte | Schnelle Iteration |
| Storage | SQLite + JSON-Replays | Lokal, prüfbar |
| Packaging | Nix Flake | NixOS-first |

Post-MVP: Phase 4 Pool/Triples/Hero-Draft; Phase 5 Shop-Advisor (`data/tags`, `data/comps`) — Panel vor Positions-Overlays.

Stack-Entscheidung erst nach einem **ToS-kompatiblen Detection-Spike**.

Details, Event-Liste und State-Modell: [EN ARCHITECTURE](../en/ARCHITECTURE.md)
