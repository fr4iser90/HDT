# Battlegrounds card index

Compact card data under `data/cards/` is derived from [HearthstoneJSON](https://hearthstonejson.com/) (`cards.json`).

| File | Purpose |
| --- | --- |
| `bg_enUS.json` | Filtered BG cards (names, stats, mechanics, techLevel, …) |

```bash
# Refresh from HearthstoneJSON (needs network)
cargo run -p bgc-cli -- cards sync

cargo run -p bgc-cli -- cards status
cargo run -p bgc-cli -- cards get BG35_814
```

Do **not** copy HDT proprietary card tables. HearthstoneJSON / HearthDb-style sources are the intended path (see `docs/en/DETECTION.md`).
