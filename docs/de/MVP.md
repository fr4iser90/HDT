# MVP (v0.1)

Der kleinste Loop, der in einer echten Battlegrounds-Partie schon nützlich ist.

## Ziel

Beweisen: **erkennen → State lesen → Overlay → Match speichern**.

Wenn das unzuverlässig ist: keine Feature-Ausweitung.

## Im Scope

1. **Hearthstone** lokal erkennen (Linux zuerst).
2. **Battlegrounds**-Modus / Match Start & Ende.
3. **Live-State** (best effort, dokumentierte Konfidenz):
   - Runde, Gold, HP, Tavernenstufe
   - Eigenes Board
   - Shop, wenn sichtbar
4. **Overlay** mit Verschieben / Ausblenden / Skalieren (Basis).
5. **Lokales Speichern** von Match-Snapshots.
6. **Match-History** (Zeit, Held falls bekannt, Platzierung falls bekannt).

## Nicht im Scope für v0.1

- Gegner-Build-Inferenz
- Combat-Win%
- Voller Replay-Scrubber
- Plugin-API
- Web-Dashboard
- Community/Cloud
- Constructed
- Meta-Decks / Comp-Matching
- Shop-/Board-Highlights (Eco, Key-Cards, Positions-Marker)
- Minion-Pool / verbleibende Karten
- Triple-Tracker
- Hero-Draft-Assistent
- Tribe-/Build-Coaching

## Abnahmekriterien

- Companion startet stabil, auch wenn HS nicht läuft
- Bei BG-Match erscheint das Overlay in dokumentierter Zeit
- Runde / Gold / HP / Taverne aktualisieren sich in einer echten Partie
- Unbekanntes bleibt „unknown“ — nichts erfinden
- Match-Ende schreibt einen lokalen Datensatz
- History listet die letzten N Matches
- Keine Battle.net-Credentials
- Keine Telemetrie
- Linux-/Nix-Install dokumentiert

## Definition of Done

Ein NixOS-User kann laut README installieren, eine BG-Partie spielen, das Overlay sehen und das Match danach in der lokalen History finden.

Vollständige Checklisten/Slices: [EN MVP](../en/MVP.md)
