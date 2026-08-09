# Compliance, Privatsphäre & Sicherheit

Das Projekt bleibt **spiel-sicher, lokal und konservativ** gegenüber Blizzard-Regeln.

Kein Rechtsrat. Vor jeder Collector-Methode ToS erneut prüfen.

## Harte Regeln

1. Keine Battle.net-Zugangsdaten
2. Keine Input-Automation / kein Auto-Play
3. Nur beobachten/lesen — innerhalb erlaubter Mechanismen
4. Daten standardmäßig lokal; Upload nur explizit opt-in
5. Telemetrie aus / besser: erst gar nicht
6. Transparent dokumentieren, was gelesen wird

## Vor dem Implementieren eines Collectors

Kurz schriftlich festhalten (ADR / dieses Dokument):

- Welche Datenquelle?
- Offiziell/user-sichtbar vs. invasiv?
- Was sagen aktuelle Blizzard-/HS-Regeln?
- Fallback, falls Methode unzulässig oder kaputt ist?

**Bevorzugt:** akzeptierte Tracker-Ansätze (z. B. lokale Logs, soweit anwendbar) statt Memory-/Packet-Hacks — außer eine Review rechtfertigt etwas anderes und die ToS erlauben es.

## Open-Source-Vorsicht

Code, der verbotene Techniken demonstriert, kann User und Projekt gefährden.  
Wenn nur ein unzulässiger Weg funktioniert: **stopp und neu denken** — nicht stillschweigend shippen.

Vollständige Tabelle und Changelog-Pflicht: [EN COMPLIANCE](../en/COMPLIANCE.md)
