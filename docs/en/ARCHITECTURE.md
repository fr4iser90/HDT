# Architecture

Status: **planned**. Implementation languages and packaging may adjust after the first detection spike.

## Design principles

1. **Local-first** — state and history live on the user’s machine.
2. **Event-driven** — modules subscribe to game events; they do not call each other in a spaghetti chain.
3. **Data-driven cards** — minions/heroes/mechanics come from versioned data, not hardcoded `if card == …`.
4. **Thin MVP surface** — ship detect → state → overlay → persist before plugins/dashboard polish.
5. **Replaceable collectors** — game-reading backends can change if ToS or tech constraints require it.

## Proposed layout

```text
battlegrounds-companion/
├── apps/
│   ├── desktop/          # tray / window / lifecycle
│   └── overlay/          # always-on-top BG HUD
├── core/
│   ├── game/             # process / mode detection
│   ├── state/            # normalized BG game state
│   ├── tracking/         # collectors / parsers
│   ├── analysis/         # combat, tribe inference, shop advisor (later)
│   ├── events/           # event bus + schemas
│   └── storage/          # local DB + replays
├── data/
│   ├── cards/
│   ├── heroes/
│   ├── tribes/
│   ├── mechanics/
│   ├── tags/             # eco / key / role labels per card
│   ├── comps/            # meta composition definitions
│   └── patches/
├── plugins/              # first-party examples
├── ui/                   # shared UI packages (later web dashboard)
├── tests/
├── docs/
│   ├── en/
│   └── de/
├── LICENSE
├── README.md
└── CONTRIBUTING.md
```

## Runtime shape

```text
┌─────────────────┐
│ Game collector  │ ──emits──▶ Event bus ──▶ State store
└─────────────────┘                │
                                   ├─▶ Overlay
                                   ├─▶ Storage / replays
                                   ├─▶ Analyzers
                                   └─▶ Plugins
```

## Event vocabulary (initial)

| Event | Meaning |
| --- | --- |
| `HEARTHSTONE_STARTED` / `HEARTHSTONE_STOPPED` | Process lifecycle |
| `MODE_CHANGED` | Lobby / BG / other |
| `MATCH_STARTED` / `MATCH_ENDED` | BG match boundaries |
| `HERO_OFFERED` / `HERO_SELECTED` | Draft options / choice resolved |
| `TURN_STARTED` / `TURN_ENDED` | Player turn boundaries |
| `SHOP_UPDATED` | Tavern offerings changed |
| `BOARD_UPDATED` | Own board changed |
| `CARD_PURCHASED` / `CARD_SOLD` / `CARD_TRIPLED` | Economy actions |
| `CARD_SEEN` | Minion observed (shop, board, discover) for pool accounting |
| `POOL_UPDATED` | Derived remaining counts changed |
| `TAVERN_UPGRADED` | Tier up |
| `COMBAT_STARTED` / `COMBAT_ENDED` | Fight phase |
| `PLAYER_DAMAGED` | HP change with source if known |
| `OPPONENT_SNAPSHOT` | Observed enemy board/hero/tavern |

Events carry timestamps, match IDs, and opaque payload schemas versioned in `core/events`.

## Game state (normalized)

Logical model (not final schema):

```text
Match
├── meta (start time, region if known, patch)
├── draft (offered heroes, pick)          ← Phase 4C
├── self (hero, hp, gold, tavern, board, hand/shop, triples)
├── opponents[] (hero, hp, tavern, last board, last seen turn)
├── pool[] (cardId → remaining / confidence)  ← Phase 4A
├── rounds[]
│    ├── shop / purchases / sales / upgrade / seen
│    └── combat (boards, damage expected/actual — later)
└── result (placement, duration)
```

Persistence targets:

- Relational/local DB for queryable history (e.g. SQLite)
- Append-only replay files for round-by-round scrubbing

## Card data

```json
{
  "id": "example_minion",
  "name": "Example Minion",
  "tier": 4,
  "tribes": ["Dragon"],
  "attack": 5,
  "health": 7
}
```

Patch folders under `data/patches/` pin which card set is active. The app never embeds patch logic as one-off string compares in core loops.

## Overlay

- Freely movable, hideable, scalable widgets
- Minimal default HUD: round, gold, HP, tavern, board, shop
- Later: opponents panel, pool browser, triple progress, hero draft, combat summary, plugin surfaces
- Advisor layers (post-MVP):
  - **Panel mode** — highlight/tag shop & board entries inside our UI (easy, portable)
  - **Positional mode** — markers over in-game tavern/board slots (optional; needs layout calibration)

## Pool, triples & hero draft (Phase 4)

```text
CARD_SEEN / shop / boards / discovers
        ↓
Pool engine  ──▶ POOL_UPDATED
        ↓
Triple progress (owned copies → golden)
        ↓
Overlay panels (pool browser, triple cues)

Hero offers ──▶ draft notes from data/heroes ──▶ pick UI ──▶ HERO_SELECTED
```

Pool sizes and hero notes are data-driven per patch. Incomplete sightings reduce confidence instead of faking exact counts.

## Shop advisor (Phase 5+)

```text
State (board + shop + pool)
        ↓
Tag engine (eco, key, tribe, …)     ← data/tags
        ↓
Comp matcher (fit %, missing)       ← data/comps
        ↓
Overlay / panel (user toggles)
```

Advisor emits suggestions as events (e.g. `SHOP_HINTS_UPDATED`) so plugins can reuse the same pipeline.

## Plugin API (post-MVP)

Plugins subscribe to events and may register overlay panels or analysis hooks.  
Core guarantees: event schemas, state snapshots, storage accessors — not direct game-memory handles.

## Tech direction (to validate)

| Layer | Likely choice | Why |
| --- | --- | --- |
| Collector / core | Rust (or C#) | Performance, packaging, safety |
| Overlay / desktop shell | Tauri or similar | Native Linux + web UI skills |
| Dashboard | TypeScript + React/Svelte | Fast iteration |
| Storage | SQLite + JSON replays | Local, inspectable |
| Packaging | Nix flake + native packages | NixOS-first install story |

Final stack is decided after a **detection spike** proves a ToS-compatible way to read BG state on Linux.

## Testing strategy

- Unit tests for state reducers and event schemas
- Fixture-based tests from recorded logs/replays (no live HS required in CI)
- Manual checklist for overlay on X11/Wayland
