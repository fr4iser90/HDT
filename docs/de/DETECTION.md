# Detection & Kartendaten (Prior Art)

Wie Tracker wie [HDT](https://github.com/HearthSim/Hearthstone-Deck-Tracker) an Karten- und Livedaten kommen — und was wir nutzen dürfen.

Kein Rechtsrat. Details und Tabellen: **[EN DETECTION](../en/DETECTION.md)**.

## Kurz

| Thema | Praxis bei HearthSim/HDT | Für uns |
| --- | --- | --- |
| Kartendaten | Game `CardDefs` → [HearthstoneJSON](https://hearthstonejson.com/) → Libs wie [HearthDb](https://github.com/HearthSim/HearthDb) (MIT) | JSON/`data/` selbst pflegen; MIT-Libs ok |
| Live-Match | `log.config` → **`Power.log`** parsen | Eigenen Parser / MIT ([python-hslog](https://github.com/HearthSim/python-hslog)) |
| Collection o.ä. | teils **Memory** | Nur nach ToS-ADR; kein HDT-Code |
| HDT-Source | **All Rights Reserved** | **Nicht kopieren** |

## Harte Regel

```text
❌ HDT-Code/Assets übernehmen
✅ MIT-HearthSim-Libs + Attribution
✅ HearthstoneJSON / eigene data/
✅ Eigenes Power.log → Event-Pipeline
```

Phase-1-Spike: unter NixOS/Proton beweisen, dass BG-Felder im Log stehen.
