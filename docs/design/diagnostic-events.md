# Design: Check Delta

## Requirement

A developer runs `8v check` and wants to know: is it getting better or worse? Did I introduce new problems? Did old problems go away?

## What We Require

Two snapshots. The previous check result and the current check result. Compare.

## Design

1. After `8v check`, store the CheckReport diagnostic summary to `~/.8v/last-check.json`
2. Before the next check, read `last-check.json` — that is the "before"
3. Run check — that is the "after"
4. Compare. New, fixed, unchanged.
5. Overwrite `last-check.json` with the current result

One file. No accumulation. No IDs. No merge. No finalize.

## What Gets Deleted

- `o8v/src/events.rs` — 700 lines, the entire EventWriter
- `o8v-events/` crate — SeriesJson, SeriesEntry, normalize_message, all helpers
- `~/.8v/events/` directory — per-run NDJSON logs nobody reads
- `~/.8v/series.json` — replaced by `last-check.json`
- SHA256 diagnostic ID computation
- File content caching for span extraction
- Log rotation logic

## What Remains

- The EventBus carries command lifecycle events (CommandStarted/CommandCompleted)
- StorageSubscriber writes those to `command-events.ndjson`
- Delta is computed by the check command itself: read previous, run current, compare

## Open Questions

1. What subset of CheckReport goes into `last-check.json`? Full report or just diagnostic identities?
2. How to identify "same diagnostic" across runs without SHA256? File + rule + message? File + line + rule?
3. Should `8v init` still run a baseline check? If so, it writes the first `last-check.json`.
