# Detection & card data (prior art)

How established trackers (especially [Hearthstone Deck Tracker](https://github.com/HearthSim/Hearthstone-Deck-Tracker)) obtain game/card information — and what this MIT project may reuse.

This is **not legal advice**. Re-check licenses and Blizzard terms before implementation.

## Two different problems

| Problem | What it answers |
| --- | --- |
| **Card database** | What is card `BG_123`? Name, tier, tribes, stats, text |
| **Live detection** | What is happening *right now* in my match? |

HDT solves both. We should too — but with clean licensing.

---

## How card information is obtained

HearthSim does **not** invent card stats by scraping the HDT UI.

Typical pipeline in the ecosystem:

```text
Hearthstone game files (CardDefs / extractable defs)
        ↓
HearthSim hs-data / build extraction
        ↓
HearthstoneJSON API  (api.hearthstonejson.com)
        ↓
Libraries (e.g. HearthDb) embed or download CardDefs / cards.json
```

- **[HearthstoneJSON](https://hearthstonejson.com/)** exposes build-versioned JSON (`cards.json`, locales, enums) derived from game data.
- **[HearthDb](https://github.com/HearthSim/HearthDb)** (MIT) loads `CardDefs` (from HearthstoneJSON) and provides card IDs, enums, deckstrings for .NET.
- **[python-hearthstone](https://github.com/HearthSim/python-hearthstone)** (MIT) covers CardDefs, enums, and related parsing in Python.

**For our companion:** prefer consuming **HearthstoneJSON** (and/or MIT libraries) into our own `data/` — do not vendor HDT’s internal card tables by copying HDT source.

Image art from HearthstoneJSON’s art API is usable with their stated request to re-host rather than hotlink at scale; contact them for commercial use.

---

## How live detection works

### 1. Power.log (primary, preferred)

Hearthstone can write detailed game events to local log files (notably **`Power.log`**) when logging is enabled via a **`log.config`** in the Hearthstone data directory.

HDT documents this setup in its wiki: [Setting up the log.config](https://github.com/HearthSim/Hearthstone-Deck-Tracker/wiki/Setting-up-the-log.config).

HearthSim maintainers have stated publicly that in-game card knowledge comes from these logs (see [HDT#3252](https://github.com/HearthSim/Hearthstone-Deck-Tracker/issues/3252)).

Rough flow:

```text
Enable logging (log.config)
        ↓
Tail / parse Power.log (and related channels)
        ↓
Map entities + tags → normalized game state
        ↓
Overlay / stats / pool accounting
```

MIT-friendly parsers exist, e.g. **[python-hslog](https://github.com/HearthSim/python-hslog)** (MIT) for deserializing `Power.log`.

On Linux/Proton the same idea applies if the Wine prefix still produces Blizzard log files — paths differ; the spike must locate them.

### 2. Process / window detection

Find the Hearthstone process and window for “is the game running?” and overlay placement. Standard OS APIs; not HDT-specific IP.

### 3. Memory reading (HDT also uses this for some things)

For collection sync and some data not fully exposed in logs, HDT historically used **in-memory** reads ([same issue thread](https://github.com/HearthSim/Hearthstone-Deck-Tracker/issues/3252)). Related code lives around projects like `HearthWatcher` inside the HDT tree.

**Our stance (see [COMPLIANCE.md](COMPLIANCE.md)):** prefer logs first. Treat memory/packet approaches as last resort, only after an explicit ToS review and ADR. Do **not** copy HDT memory-scanner code.

---

## What we may use from HearthSim / HDT

| Asset | License / status | Our use |
| --- | --- | --- |
| **[Hearthstone-Deck-Tracker](https://github.com/HearthSim/Hearthstone-Deck-Tracker)** source | **All Rights Reserved** (README) | **Do not copy** code, assets, or substantial structure into this MIT repo. Reading for education is fine; reimplementation must be independent. Contributors to HDT also sign a [CLA](https://github.com/HearthSim/Hearthstone-Deck-Tracker/blob/master/CONTRIBUTING.md) — their contributions are not a free-for-all for other projects. |
| HDT wiki (log.config, FAQ) | Informational | Use as **documentation of Blizzard’s logging mechanism**, not as a license to paste HDT code |
| **HearthDb** | MIT | OK to depend on / learn from (with attribution) |
| **python-hearthstone** | MIT | OK |
| **python-hslog** | MIT | OK (or reimplement against the same public log format) |
| **HearthstoneJSON** API | Public data API; be a good citizen (caching, re-host art, contact for commercial) | OK as card/data source |
| Ideas: “parse Power.log”, “pool from seen cards” | Not copyrightable as ideas | OK — write our own code |

### Hard rule for this repo

```text
❌  Copy-paste from Hearthstone-Deck-Tracker
❌  Vendor HDT binaries / decompile
✅  Use MIT HearthSim libraries with LICENSE attribution
✅  Fetch card defs from HearthstoneJSON / game extracts into data/
✅  Implement our own Power.log → events pipeline
✅  Document Linux/Proton log paths ourselves
```

---

## Recommended path for Battlegrounds Companion

1. **Cards:** HearthstoneJSON → versioned files under `data/` (BG filter in our tooling).
2. **Live state:** enable/`log.config`-style logging → parse `Power.log` → our event bus.
3. **Spike (Phase 1):** prove BG fields (round, gold, HP, tavern, board, shop) appear in logs under NixOS/Proton.
4. **Only if gaps remain:** ADR for alternative collectors; never silent HDT code reuse.

## Related links

- [HDT repository](https://github.com/HearthSim/Hearthstone-Deck-Tracker)
- [HearthSim](https://hearthsim.info/)
- [How we process replays (Power.log context)](https://hearthsim.info/blog/how-we-process-replays/)
- [HearthstoneJSON](https://hearthstonejson.com/)
