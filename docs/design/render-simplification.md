# Design — Render layer simplification

**Status:** DRAFT — awaiting review.
**Author:** Claude (per founder direction, 2026-04-16).
**Owner:** Soheil.

## Problem

The render layer has three audiences (`Agent`, `Human`, `Machine`) and a
`Renderable` trait with three methods (`render_plain`, `render_human`,
`render_json`). Around this sit `Audience`, `Caller::{Cli,Mcp}`, and the
`_8V_AGENT` env var — routing logic that decides which of the three
methods to call.

Reality of the 20 implementers:

- **4 types differentiate human vs plain:** `CheckReport`, `FmtReport`,
  `HooksReport`, `UpgradeReport`.
- **13 types are dead delegations** — `render_human()` is literally
  `self.render_plain()`: `TestReport`, `BuildReport`, `SearchReport`,
  `ReadReport`, `RunReport`, `WriteReport`, `LsReport`, plus all six
  streaming event types (`TestEvent`, `BuildEvent`, `RunEvent`,
  `StreamCheckEvent`, `FmtEvent`, `UpgradeEvent`).

So the three-audience abstraction is carrying its weight for 4 types out
of 20. For the other 16, `Audience::Human` and `Audience::Agent` produce
byte-identical output. The routing layer (`Caller`, `_8V_AGENT`,
`Audience`) exists to distinguish cases that are not distinguished.

## Goal

Keep the one thing that matters — a human running `8v check .` in the
terminal gets colored, aligned output — and delete the rest.

## Proposal

### Two outputs on the trait

```rust
pub trait Renderable {
    fn render_plain(&self) -> Output;
    fn render_json(&self) -> Output;
}
```

That's it. Humans and agents both get `render_plain`. It's text. It's
the same text. No routing needed.

### Pretty terminal output for the 4 exceptions

`CheckReport`, `FmtReport`, `HooksReport`, `UpgradeReport` keep their
current pretty-printer functions (`check_human::render_check_human`
etc.). These are called **directly from the CLI dispatch** when the
command runs on a TTY, not through the trait.

```rust
// in o8v-cli check command
if caller.is_cli() && io::stdout().is_terminal() {
    print!("{}", check_human::render(&report));
} else {
    print!("{}", report.render_plain());
}
```

Four commands know they have a pretty renderer. Nobody else needs to.

### What gets deleted

- `Renderable::render_human()` — gone from trait.
- `Audience::Human` variant — gone.
- 13 dead-delegation impls — gone.
- `Caller::{Cli,Mcp}` routing that exists to pick an audience — gone
  (the CLI just checks `is_terminal()` directly where it matters).
- `_8V_AGENT` env var plumbing — gone. Nothing reads it in agent mode
  that it didn't already produce identically.
- The precedence ladder in `output_format.rs` (`--json > --human >
  --plain > _8V_AGENT > default`) collapses to `--json > default-plain`.

### What stays

- `--json` flag (unchanged).
- `render_plain` + `render_json` on every type.
- Pretty terminal output for `check`, `fmt`, `hooks`, `upgrade`.
- `ReadReport` progressive variants (`Symbols` / `Range` / `Full`) —
  unrelated to audience, still good design, untouched.

## Migration plan

1. **Inline the 4 pretty renderers into their CLI commands.** Call
   `check_human::render_check_human` directly from the check command
   based on `is_terminal()`. Do the same for `fmt`, `hooks`, `upgrade`.
2. **Remove `render_human` from the `Renderable` trait.** Delete the 4
   non-trivial impls (they're now called directly) and the 13 dead
   delegations.
3. **Remove `Audience::Human` and collapse the router.**
   `render(item, audience)` becomes two call sites: `item.render_plain()`
   or `item.render_json()`.
4. **Remove `Caller::{Cli,Mcp}` and `_8V_AGENT` plumbing.** The only
   decisions left are `--json` flag and `is_terminal()` for the 4
   pretty-output commands.
5. **Run the full test suite.** Any test asserting `render_human ==
   render_plain` gets deleted (the assertion is now trivially true).

Each step is one commit. Each leaves the binary working.

## Breaking changes

Internal to this workspace only. The CLI output and `--json` shape are
unchanged. No external consumers.

## Non-goals

- Changing what any command prints. Output bytes should be identical
  before and after for every (command, flags) combination.
- Streaming event compaction (dropping progress spinners, collapsing
  "Compiling X" lines). Separate concern, separate doc if we want it.
- Touching `ReadReport`'s variants or any other progressive-disclosure
  design. Unrelated.

## Open questions

None blocking.

## Review checklist

- [ ] Founder has read this doc
- [ ] Agrees two outputs (plain, json) are enough
- [ ] Agrees the 4 pretty renderers move to CLI-side dispatch
- [ ] Agrees `_8V_AGENT` env var goes away
- [ ] Migration plan (5 steps, one commit each) approved
