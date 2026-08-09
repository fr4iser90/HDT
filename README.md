# Battlegrounds Companion

> Open-source, Linux-first companion for **Hearthstone Battlegrounds**.

**Languages:** [English](README.md) · [Deutsch](docs/de/README.md)

This is **not** a Hearthstone Deck Tracker clone.  
It is a focused Battlegrounds tool: detect the game → collect state → analyze live → show a configurable overlay → keep everything local, inspectable, and extensible.

```bash
# Target UX (planned)
nix run github:fr4iser90/battlegrounds-companion
```

Start Hearthstone → companion detects Battlegrounds → overlay appears.

---

## Why this exists

[Hearthstone Deck Tracker (HDT)](https://hsdecktracker.net/) already covers a lot. If you only need “track BG + show an overlay” and HDT works on your setup, **use HDT**.

This project exists for a different niche:

| Focus | HDT | This companion |
| --- | --- | --- |
| Mode | Constructed + BG | **Battlegrounds-first** |
| Platforms | Windows-first (Wine/Proton elsewhere) | **Native Linux / NixOS + cross-platform** |
| Overlay | General tracker UI | **BG-optimized, fully configurable** |
| Shop help | Plugins / external | **Pool, triples, draft, tags, meta comps** |
| Analysis | Plugins / Bob’s Buddy | **Built-in combat & opponent tracking** |
| Replays | Partial | **First-class** |
| Extensibility | Plugin ecosystem | **Open plugin API from day one** |
| License | Check upstream terms | **MIT** |

Build only what HDT leaves open for Linux / NixOS Battlegrounds players — not a feature-for-feature remake.

---

## Status

**Pre-MVP / documentation phase.** No gameplay pipeline yet.

See:

- [Vision](docs/en/VISION.md)
- [Architecture](docs/en/ARCHITECTURE.md)
- [MVP](docs/en/MVP.md)
- [Roadmap](docs/en/ROADMAP.md)
- [Compliance & privacy](docs/en/COMPLIANCE.md)
- [Detection & card data](docs/en/DETECTION.md)
- [Contributing](CONTRIBUTING.md)

---

## Planned MVP (v0.1)

```text
Detect Hearthstone
      ↓
Detect Battlegrounds
      ↓
Read live game state
      ↓
Show overlay (round, gold, HP, tavern, board, shop)
      ↓
Persist match locally
      ↓
Basic match history
```

Everything else (opponent inference, combat odds, replays UI, plugins, web dashboard) comes **after** this loop works reliably.

---

## High-level architecture (planned)

```text
Core (game detection + state + events + DB)
  ├── Overlay (desktop)
  ├── Local API
  │     └── Web dashboard (later)
  └── Plugin API
```

Components talk through an **event bus** (`TURN_STARTED`, `CARD_PURCHASED`, `COMBAT_ENDED`, …), not hard-wired update chains.

Card/hero data lives in versioned data files — not hardcoded minion names in application logic.

---

## Privacy (non-negotiable)

- Local-first
- No Battle.net credentials
- No account scraping
- Telemetry **off** by default
- Clear docs on which game data is read

Before any memory / packet / automation approach is implemented, Blizzard/Hearthstone terms are reviewed and documented in [COMPLIANCE.md](docs/en/COMPLIANCE.md).

---

## Repository note

The GitHub remote may still be named `HDT`. The product name is **Battlegrounds Companion**. A rename of the remote/repo is planned once the docs settle.

---

## License

[MIT](LICENSE) © 2026 Patrick B.
