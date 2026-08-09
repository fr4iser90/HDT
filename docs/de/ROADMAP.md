# Roadmap

Reihenfolge: **Zuverlässigkeit vor Cleverness**.

| Phase | Inhalt |
| --- | --- |
| 0 | Dokumentation (aktuell) |
| 1 | Detection-Spike + ToS-Check |
| 2 | Core-Loop / MVP v0.1 |
| 3 | Gegner-Tracking |
| 4 | **Minion-Pool, Triples, Hero Draft** |
| 5 | Shop- & Board-Advisor (Tags → Meta-Comps → optionale Positions-Marker) |
| 6 | Combat History |
| 7 | Replays |
| 8 | Statistiken |
| 9 | Plugin-System |
| 10 | Web-Dashboard |
| 11 | Community-Packaging (`nix run`, Distro-Pakete) |

### Phase 4 (Kurz)

| Slice | Feature |
| --- | --- |
| 4A | Minion-Pool („noch wie viele im Pool?“) mit Konfidenz bei Lücken |
| 4B | Triple-Tracker (Fortschritt → Golden, Shop-Hinweise) |
| 4C | Hero-Draft-Assistent (Tier/Notes aus `data/heroes`, abschaltbar) |

Kommt **vor** Meta-Coaching, damit Advisor später auch Knappheit im Pool nutzen kann.

**Advisor-Regel:** zuerst Markierungen im Companion-Panel, erst danach Overlays über Ingame-Tavernen-Slots. Alles datengetrieben unter `data/tags` + `data/comps`, Layer abschaltbar.

Parking lot: voller Constructed-Tracker, Mobile, Pflicht-Accounts, Auto-Play / Input-Injection, erzwungener Coach — **nein**.

Ausführlich: [EN ROADMAP](../en/ROADMAP.md)
