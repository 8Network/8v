# MCP Parser — Multi-Positional Arg Bug (B-MCP-3) — Design Note

**Status:** draft, pre-review. Do NOT implement until reviewed.
**Author:** Claude, at Soheil's direction (autonomous loop, 2026-04-15).
**Scope:** `o8v/src/mcp/parse.rs`. No behavior change at the CLI layer.

---

## The problem

Entry 21 (learnings log, 2026-04-15) found that the MCP command `8v write src/main.rs:10 "    for i in start..=end {"` fails deterministically, while the identical command on the CLI succeeds. Root cause in `parse_mcp_command`:

```rust
let flag_start = parts[1..].iter().position(|p| p.starts_with('-')).map(|i|i+1);
match flag_start {
    Some(idx) => { let path = parts[1..idx].join(" "); ... }
    None =>     { let path = parts[1..].join(" ");    ... }  // BUG
}
```

The code assumes everything between the subcommand and the first flag is the path. For subcommands with **multiple positional arguments** (`write <path> [content]`, `search <pattern> [path]`), this logic collapses all positionals into a single space-joined `path` string.

Failing subcommand shapes today:

| Subcommand | Shape                                   | When MCP call fails |
|------------|------------------------------------------|---------------------|
| `write`    | `<path> [content]` + optional flags      | When content is given without any `--flag` present |
| `search`   | `<pattern> [path]` + optional flags      | When pattern + path both given without flags |

Counterintuitively, **adding a flag makes the command succeed**, because `flag_start = Some(idx)` limits the path-join to `parts[1..idx]`.

## Non-goals

- Changing CLI argv parsing. Only the MCP `command: String` → argv conversion is affected.
- New subcommands, new flags, new behavior. Fix only.
- Backwards-compatible handling of "weird" inputs. We want strict, correct parsing.

## Design

Let clap do the parsing. The pre-clap heuristic for "path grouping" is redundant once shlex has tokenized. Remove it; pass the shlex-split tokens directly to clap.

### Current pipeline

```
command: String
   ↓ shlex::split()
parts: Vec<String>
   ↓ strip leading "8v" token
parts: &[String]
   ↓ manual "everything before first flag = path" join
args: Vec<String> (len 2: subcommand + path-or-joined-path)
   ↓ resolve_command_path (mutates args[1])
args: Vec<String>
   ↓ clap::Cli::try_parse_from
Command
```

### Proposed pipeline

```
command: String
   ↓ shlex::split()
parts: Vec<String>
   ↓ strip leading "8v" token
parts: &[String]
   ↓ for subcommands with a positional path (all except search/ls/run):
     resolve the FIRST positional token against containment_root, in place
parts: &[String]   (length unchanged)
   ↓ clap::Cli::try_parse_from(["8v", subcommand, parts...])
Command
```

Key differences:
1. **No join.** Each shlex token stays a token.
2. **Path resolution is per-positional-arg, not per-concatenated-path.** For `write src/main.rs:10 "content"`, only `src/main.rs:10` is path-resolved; `"content"` is left alone.
3. **clap handles positional routing.** `write` has `path: String, content: Option<String>` — clap maps tokens to fields correctly.

### Implementation sketch

```rust
pub(super) fn parse_mcp_command(
    command: &str,
    containment_root: &o8v_fs::ContainmentRoot,
) -> Result<Command, String> {
    // existing: null check, size check
    let parts = shlex::split(command).ok_or_else(|| /* unmatched quotes */)?;
    let parts: Vec<String> = match parts.first().map(String::as_str) {
        Some("8v") => parts[1..].to_vec(),
        _ => parts,
    };
    if parts.is_empty() { return Err("error: empty command".into()); }

    let subcommand = parts[0].as_str();

    // Resolve first-positional path for subcommands that take one.
    // Subcommands that skip: search (first positional is pattern, not path),
    // ls (path is optional/default), run (first positional is command string).
    let needs_path_resolve = !matches!(subcommand, "search" | "ls" | "run");
    let mut parts = parts;
    if needs_path_resolve && parts.len() > 1 {
        // First positional (parts[1]) is the path — resolve in place.
        super::path::resolve_single_path(&mut parts[1], containment_root)?;
    }

    let argv: Vec<&str> = std::iter::once("8v")
        .chain(parts.iter().map(String::as_str))
        .collect();

    Cli::try_parse_from(argv)
        .map(|c| c.command)
        .map_err(parse_error)
}
```

`resolve_single_path` is a new helper — a thin version of `resolve_command_path` that mutates one string instead of probing args for the "path index." One positional, one resolve call.

### What about `search`?

`8v search "foo bar" somedir` tokenizes to `["search", "foo bar", "somedir"]`. Clap's `SearchCommand` has `pattern: String` + `path: Option<String>`. That maps cleanly — `pattern = "foo bar"`, `path = "somedir"`. No path-join needed.

If the agent supplies a path like `"some dir with spaces"` with spaces, shlex preserves the quotes → single token. Works.

### What about `run`?

`8v run "echo hello world"` tokenizes to `["run", "echo hello world"]`. Clap's `RunCommand` has `command: String` positional. Maps to `command = "echo hello world"`. Works.

## Test plan

1. **Unit — regression of B-MCP-3.**
   ```rust
   parse_mcp_command("write src/main.rs:10 \"hello world\"", &root)
       .expect("must parse");
   ```
   Must succeed; previously fails.

2. **Unit — flag-present still works.**
   ```rust
   parse_mcp_command("write src/main.rs --find \"foo\" --replace \"bar\"", &root);
   ```
   Must still succeed (currently works; verify no regression).

3. **Unit — search with two positionals.**
   ```rust
   parse_mcp_command("search \"foo bar\" src/", &root);
   ```
   Must parse with `pattern="foo bar"` and `path="src/"`.

4. **Integration — fix-test benchmark CV drops.**
   Re-run `experiment_fix_test` N=6 after this lands. Expect 8v-arm CV to drop from 17.5% to ~10% (no more retry tails), and mean cost to drop ~$0.01-0.02.

5. **All existing parse.rs tests** (`cargo test -p o8v --test mcp*`) must keep passing unchanged.

## Open questions (review needed)

1. **Is `resolve_command_path` used anywhere that relies on the current "join + probe" behavior?**
   Grep for callers. If only `parse_mcp_command` uses it, we can delete it; otherwise we keep it and add `resolve_single_path` alongside.

2. **What about subcommands where the path is *not* the first positional?**
   Currently none exist. If future subcommand has `<flag-value> <path>`, the "first-positional-is-path" assumption breaks. Acceptable given "one thing at a time" rule — we cross that bridge when we add such a subcommand.

3. **Handling of paths that happen to look like flags?**
   A file literally named `--foo` would confuse clap's default parsing. shlex preserves it as one token; clap would treat it as a flag and error. Acceptable edge case — consistent with how CLI behaves. Document but don't work around.

## Not in this change

- The fixture pre-flight gate (separate design, docs/design/fixture-preflight-gate.md).
- The structured-benchmark-report pipeline.
- Any new subcommand behavior.

## Risk

Low. The change *removes* heuristics in favor of letting clap — which already owns the argv → Command mapping for CLI — do the same for MCP. The only risk is subcommands with non-standard arg shapes, enumerated above (search, ls, run) and handled by the `needs_path_resolve` gate.

## Rollout

1. Land the fix.
2. Re-run Entry 17's fix-test N=6 to confirm CV drop.
3. Update Entry 17 with post-B-MCP-3 numbers (or write Entry 22).
4. Audit other MCP calls in Entry 17's NDJSON for failures the new parser avoids.

---

When Soheil approves, implement. Until then, B-MCP-3 is live and costs ~$0.03-0.05 per affected run (single-digit percent of fix-test variance).

---

## Review findings (2026-04-15)

Adversarial agent review surfaced gaps. Address before implementation:

1. **`resolve_single_path` does not exist.** Read `o8v/src/mcp/path.rs` to either extract a single-string helper from `resolve_command_path`, or adapt the existing function to mutate `args[1]` in place. Pick one before writing the fix.
2. **Leading-dash content trips clap's `--` terminator.** `8v write foo.rs:10 "-- comment"` will fail. Add `.allow_hyphen_values(true)` to the `content` field of `WriteCommand` (and any other command whose trailing positional may begin with `-`).
3. **Edge-case tests required, not optional:**
   - empty content: `8v write foo.rs:10 ""`
   - content equal to a known flag: `8v write foo.rs:10 "--insert"`
   - `path:line:col`: `8v read foo.rs:10:5`
   - subcommand whose first positional is missing entirely
4. **CV-drop is a prediction, not a gate.** The "CV drops to ~10%" claim in test #4 cannot be the success criterion. Define an explicit acceptance criterion for the parser fix that does not depend on benchmark variance.
