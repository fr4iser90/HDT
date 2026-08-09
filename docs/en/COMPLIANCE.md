# Compliance, privacy & safety

This project must stay **play-safe, local-first, and legally conservative**.

Nothing here is legal advice. Maintainers and contributors must re-check Blizzard/Hearthstone terms before implementing or changing any game-reading method.

## Hard rules

1. **No Battle.net credentials** — the companion never asks for account passwords or tokens to log into Blizzard services as the user.
2. **No input automation** — do not click, send keys, or otherwise play the game for the user.
3. **No cheating aids that interact with the client** — read/observe only, within allowed mechanisms.
4. **Local-first data** — match data stays on disk by default; any future upload is explicit opt-in.
5. **Telemetry off by default** — preferably absent in early versions.
6. **Transparency** — document exactly what is read (logs, files, APIs, etc.) in user-facing docs.

## Before implementing a collector

Complete a short written review in this file or a linked ADR:

- What data source is used?
- Is it official / user-visible (e.g. local logs) or invasive (memory, packets)?
- What do current Blizzard/Hearthstone ToS and game guidelines say about that method?
- What is the fallback if the method is disallowed or breaks?

**Prefer** methods that are commonly accepted for trackers (such as reading local `Power.log` / related logs via `log.config`) over memory scraping or packet interception — unless a future review explicitly justifies otherwise and ToS allow it.

Background on how HDT/HearthSim approach this, and **what we may legally reuse**: [DETECTION.md](DETECTION.md).

## Privacy principles

| Data | Policy |
| --- | --- |
| Game state / boards | Local storage for history & replays |
| Battle.net email/password | Never collected |
| Hardware IDs for tracking | Not used |
| Crash reports | Opt-in only, if ever added |
| Analytics | Off / absent by default |

## Open source caution

Publishing code that demonstrates banned techniques can still create risk for users and for the project.  
If a spike shows only a disallowed path works, **stop and redesign** — do not “ship it quietly”.

## User expectations

The README and install docs must state:

- This is an unofficial third-party project, not affiliated with Blizzard.
- Hearthstone™ and related marks belong to their owners.
- Using third-party tools is the user’s responsibility under Blizzard’s terms.

## Changelog duty

When the collector strategy changes, update:

1. This document
2. The user-facing “What we read” section in the README or a dedicated privacy page
3. The detection ADR / spike notes
