# Roadmap

Order is intentional: **reliability before cleverness**.

## Phase 0 — Documentation (current)

- Product positioning vs HDT
- Architecture sketch
- MVP boundaries
- Compliance / privacy stance
- EN + DE docs skeleton

## Phase 1 — Detection spike

- Evaluate legal/technical options for reading BG state on Linux
- Pick one collector strategy (or a safe fallback path)
- Document limitations honestly

**Exit:** written decision + proof that state can be obtained for at least the MVP fields.

## Phase 2 — Core loop (MVP v0.1)

See [MVP.md](MVP.md).

## Phase 3 — Opponent tracking

- Snapshots of enemy boards when observed
- Last-seen turn, HP, tavern
- Uncertainty-aware “likely build / tribe” labels (never fake 100%)

## Phase 4 — Pool, triples & hero draft

Core Battlegrounds tracker features that belong **before** fancy meta coaching.  
Depends on reliable card IDs from the MVP loop (and opponent snapshots where relevant).

### 4A — Minion pool (“cards left”)

Track how many copies of each minion remain in the shared pool for the current lobby.

| Need | Detail |
| --- | --- |
| Inputs | Patch pool sizes, cards seen in shops, boards, discovers, triples, deaths as available |
| Output | Per-card remaining count (and “unknown / lower bound” when data is incomplete) |
| UI | Pool browser + quick “how many left?” on hover/focus of shop/board cards |
| Honesty | Never invent exact counts when the collector missed a sighting — show confidence |

### 4B — Triple tracker

| Need | Detail |
| --- | --- |
| Progress | Copies owned / toward golden for relevant minions |
| Events | `CARD_TRIPLED`, discover-from-triple when observable |
| UI | Compact triple progress on board/hand; optional alerts when a shop card completes a triple |
| Tie-in | Feed pool engine (two copies leave the pool when a golden is created, etc., per BG rules encoded in data) |

### 4C — Hero draft assistant

At lobby / hero-pick time only:

| Need | Detail |
| --- | --- |
| Data | Versioned hero notes / tiers under `data/heroes` (and optional season modifiers) |
| UI | Overlay for the offered heroes: short notes, tier, tribe lean — **toggleable** |
| Rules | Suggestions only; no autopilot; works offline from local data |
| After pick | Collapse to normal match HUD; store chosen hero on the match record |

**Exit for Phase 4:** in a real lobby you can (1) see remaining pool counts with documented gaps, (2) see triple progress, (3) get optional hero-pick notes — without meta-comp highlighting yet.

## Phase 5 — Shop & board advisor

High-value BG UX, but **after** pool/triples exist so comps can reason about scarcity.

Ship in slices (do not skip ahead to fancy positional UI):

| Slice | What | Notes |
| --- | --- | --- |
| A — Tags | Mark economy minions, discovery, tribe pieces, “key card”, etc. in the **companion panel** | Pure data + rules; no screen geometry |
| B — Meta comps | Match owned board (+ shop + pool) against versioned composition definitions | Show fit %, missing pieces, next buys as **suggestions** with uncertainty |
| C — Positional tavern/board markers | Optional markers aligned over in-game minion slots | Hard: resolution, UI scale, Wayland; only when slot layout is known |

Design rules:

- All comps/tags live under `data/` (patch-versioned) — community can update without forking core
- Every highlight layer is **toggleable** (eco only, keys only, meta only, off)
- Never invent cards; never force a single “correct” buy
- Prefer panel highlights first; positional overlays are an enhancement, not the MVP of this phase

Example composition stub (illustrative):

```json
{
  "id": "undead_example",
  "patch": "32.x",
  "name": "Example Undead",
  "core": ["card_a", "card_b"],
  "support": ["card_c"],
  "economy": ["card_eco_1"],
  "notes": "Flexible midgame; prioritize triples of card_a"
}
```

## Phase 6 — Combat history

- Store both boards and damage dealt/taken
- Optional expected damage / win chance when a model exists
- Post-fight summary in overlay or history

## Phase 7 — Replays

- Round-by-round replay files as a first-class artifact
- Scrubber: previous / play / next round

## Phase 8 — Statistics

- Placement distribution, top-4 rate, hero performance
- Tribe frequency, upgrade timing aggregates
- “Last 100 games” style summaries

## Phase 9 — Plugin system

- Stable event schemas
- Example plugins (stats panel, draft helper, custom highlight rules)
- Isolation rules (no raw memory APIs for plugins)

## Phase 10 — Web dashboard

- Local HTTP API
- Match history, replays, settings, logs in the browser

## Phase 11 — Community packaging

- Flake UX (`nix run …`)
- Flatpak / other distro packages as capacity allows
- Contributor onboarding for `data/`, pool tables, heroes, and meta-comp updates after patches

## Parking lot (maybe never)

- Full Constructed tracker
- Mobile companion
- Mandatory accounts / social network
- Auto-playing or input injection of any kind
- Mandatory “always on” coach that cannot be disabled
