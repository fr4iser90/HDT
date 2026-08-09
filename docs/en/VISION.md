# Vision

## One-liner

An open-source **Hearthstone Battlegrounds companion** that runs well on Linux/NixOS, stays local-first, and is built to be extended by the community.

## Not this

- Not “HDT, but rewritten”
- Not a full Constructed deck tracker
- Not a cloud analytics product that needs your Battle.net login
- Not a six-month architecture exercise before a working overlay

## This

- Battlegrounds as the primary (and initially only) mode
- Reliable game/state detection on Linux
- A configurable live overlay
- Local match history, replays, and BG-specific stats
- Built-in opponent tracking and combat analysis (with uncertainty, not fake certainty)
- **Minion pool**, **triple progress**, and optional **hero draft** notes
- Optional **shop/board advisor** layers: meta comps, key-card cues, economy tags — hints, not autopilot
- A modular core + plugin API
- Documentation and UX in **English** first; **German** second; more languages later

## Product promise

```text
Install → start Hearthstone → play Battlegrounds → useful overlay + local history
```

If that loop is not excellent, nothing else matters.

## Differentiation

The interesting question is not “Can we rebuild HDT?”  
It is “What do we do **better or differently** for BG on Linux?”

Target answers:

1. **Native Linux / NixOS first-class support**
2. **BG-only UX** (no Constructed noise)
3. **Analysis-first** (history, replays, combat, opponent boards)
4. **Core BG tools** (pool left, triples, hero draft)
5. **Shop intelligence** (meta comps, key pieces, eco/synergy highlights — configurable, data-driven)
6. **Truly open** (MIT, clear contribution path, data-driven card DB)
7. **Honest uncertainty** in inferences (pool counts, build guesses, combat odds, meta fit)

## Success criteria (12 months, aspirational)

- A NixOS user can install and run the companion without Wine hacks for the companion itself
- Overlay is useful in live BG games without constant babysitting
- Matches are stored locally and reviewable
- External contributors can ship a plugin without forking the core
- Compliance posture is documented and conservative

## Explicit non-goals (near term)

- Ranked Constructed / Arena / Duels parity with HDT
- Mobile apps
- Mandatory online accounts
- Autopilot coaching that dictates every buy/sell (highlights and comps are optional tools, not a playbot)
- Competing on every HDT plugin under the sun
