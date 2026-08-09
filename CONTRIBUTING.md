# Contributing

Thanks for considering a contribution.

**Languages:** project docs and default UI strings are **English**. German is the second language; more locales can follow under `docs/<locale>/`.

## Before you code

1. Read [Vision](docs/en/VISION.md), [MVP](docs/en/MVP.md), and [Compliance](docs/en/COMPLIANCE.md).
2. Prefer issues/discussion for anything that touches **game detection** or ToS-sensitive collectors.
3. Keep PRs small and aligned with the current phase (right now: documentation and later the MVP loop).

## What we want

- Fixes and clarity in docs
- Card/hero/patch data updates (once `data/` exists)
- Tests and fixtures for parsers/state
- Overlay/UX improvements within MVP scope
- Plugins **after** the plugin API exists

## What we will reject

- Account credential handling
- Input injection / auto-play
- Undocumented invasive collectors
- Scope creep that blocks the MVP loop
- Constructed-tracker parity work in early phases

## Commit style

Short, imperative messages focused on **why** when it is not obvious.

## Pull requests

- Describe the problem and the approach
- Link related issues
- Note any ToS/privacy impact explicitly
- Update EN docs; update DE when you can (EN must not lag)

## License

By contributing, you agree your contributions are licensed under the [MIT License](LICENSE).
