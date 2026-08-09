# MVP (v0.1)

Ship the smallest loop that is already useful in a live Battlegrounds game.

## Goal

Prove: **detect → read state → overlay → save match**.

If this is unreliable, do not expand features.

## In scope

1. **Detect Hearthstone** running on the local machine (Linux first).
2. **Detect Battlegrounds** mode / match start & end.
3. **Live state** (best-effort, documented confidence):
   - Round
   - Gold
   - Player HP
   - Tavern tier
   - Own board (minion identities + stats when available)
   - Shop offerings when available
4. **Overlay** with those fields; basic move / hide / scale.
5. **Local persistence** of per-match snapshots.
6. **Match history list** (start time, hero if known, placement if known, path to replay/raw log).

## Out of scope for v0.1

- Opponent build inference
- Combat win% / damage distributions
- Full replay scrubber UI
- Plugin API
- Web dashboard
- Community / cloud features
- Constructed modes
- Meta decks / composition matching
- Shop or board highlight overlays (eco tags, key-card cues, positional tavern markers)
- Minion pool remaining counts
- Triple progress UI
- Hero draft assistant
- Fancy tribe/build coaching

## Acceptance criteria

- [ ] Companion starts without crashing when HS is not running
- [ ] When HS + BG match starts, overlay shows within a short, documented delay
- [ ] Round / gold / HP / tavern update during a real match (manual verify)
- [ ] Board and shop update when the collector can see them; gaps are visible as “unknown”, not invented
- [ ] Match end writes a local record
- [ ] History UI or CLI can list the last N matches
- [ ] No Battle.net credentials requested
- [ ] Telemetry remains off / absent
- [ ] Linux install path documented (Nix preferred)

## Delivery slices

| Slice | Outcome |
| --- | --- |
| 0 — Docs | Vision, architecture, MVP, compliance (this phase) |
| 1 — Detection spike | Reliable HS + BG detection approach chosen & ToS-checked |
| 2 — State pipe | Normalized state + events from live or recorded input |
| 3 — Overlay | Usable HUD for the MVP fields |
| 4 — Persist | SQLite/JSON match write + history |
| 5 — Polish | Install docs, crash resilience, Wayland/X11 notes |

## Definition of done for “MVP complete”

A NixOS user following the README can install the companion, play one BG game, see the overlay update, and find that game in local history afterward.
