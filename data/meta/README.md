# Hero tier data

Local file: `hero_tiers.json` (or `~/.config/bgc/hero_tiers.json`, or `BGC_HERO_TIERS`).

## Where rankings come from

| Source | What it is | Notes |
| --- | --- | --- |
| **[Firestone](https://github.com/Zero-to-Heroes/firestone)** community stats | Placement / top-4 / pick rates from real games | They publish static JSON for experiments; **credit them** and ask on Discord before shipping in a public app |
| **HSReplay / HearthSim** BG stats | Large sample win rates | Check their terms / API access |
| **Manual / patch notes** | Curated S–D list | Fine for a stub; keep `source` + `updated` filled in |

Do **not** scrape random tier-list websites without permission. Prefer a cached JSON we control, refreshed intentionally.
