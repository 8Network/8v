# Slice C3 — write semantics (Level 1)

## Why this slice exists

Two bugs in `8v write` that aren't closed by B1 (docs), B2 (error routing), B3 (search), or the capital-`E` follow-up. Source: `docs/findings/command-qa-write-2026-04-20.md`.

- **AF-4**: `\n`, `\t`, `\\` are expanded in `--append` / `--insert` / positional content arguments, but NOT in the `--find` pattern. Multi-line find/replace is impossible today. Symmetry is broken.
- **AF-1**: `--force` is required to OVERWRITE an existing file but not to CREATE a new one. The help text calls it "create mode only" which contradicts the behavior. Documentation lie.

Both surface as agent friction: an agent reading the help guesses wrong about `--force` semantics and wastes calls; an agent trying multi-line edits cannot use `--find`.

## Scope

- `--find` pattern expands the same escape sequences as content arguments (`\n` → newline, `\t` → tab, `\\` → backslash). No other changes to pattern semantics (not regex, not glob — still literal after expansion).
- Update help text (`8v write --help`) to describe actual `--force` semantics (`Overwrite an existing file without failing. Not required for creating a new file.`). No flag rename (breaking change, feature-freeze territory).

Out of scope:
- Regex `--find` patterns, multi-occurrence control (single vs all), case-insensitive matching — these are feature requests, not bug fixes.
- Renaming `--force` to `--overwrite` — breaking CLI change, deferred.
- Any change to `--replace` semantics.
- `--find` as a multi-file operation.

## What changes

- **AF-4**: one function (`unescape_content` in `o8v/src/commands/write.rs`) already exists for content args. Call it on `args.find` as well before comparing. ~1 line change.
- **AF-1**: help-text update on `--force` describing its actual meaning (`Overwrite an existing file without failing. Not required for creating a new file.`). ~2 lines.

## Why each change

- AF-4: the `unescape_content` function's comment already says `applies to all content arguments: --append, --insert, and positional` — the omission of `--find` is an inconsistency, not a design decision. Confirmed by the field ordering in `WriteArgs`.
- AF-1: empirical test shows `8v write /tmp/new "content"` succeeds without `--force` (AF-1 reproducer in write-QA). Help text is wrong.

## Counterexamples

1. **Literal `\n` in pattern (agent wants to find the two characters backslash-n, not a newline).** Escape expansion breaks this. Solution: `\\n` in the shell call → shell delivers `\n` → 8v expander sees `\n` → produces a newline (expansion, not literal). To get literal backslash-n: `\\\\n` in shell → shell delivers `\\n` → 8v sees `\\n` → produces literal `\n`. Consistent with content arg behavior today. Document.
2. **Binary file find.** Escape expansion doesn't help or hurt; binary bytes were already unsupported. Out of scope.
3. **Empty `--find` pattern after expansion.** `--find ""` is ambiguous. Reject (exit 2) unless it already has defined behavior. Measure first.
4. **`--force` on a symlink.** Should it follow or error? Existing behavior per write-QA: symlink rejection. Unchanged by this slice.
5. **Concurrent writer.** `--find/--replace` is read-modify-write. A concurrent writer can race. Not addressed here; write-QA §3 already notes atomicity is unverified. Flagged for a separate concern.

## Failing-first acceptance tests

- `write_find_expands_backslash_n_to_newline` (currently fails; find is treated as literal `\n`)
- `write_find_expands_backslash_t_to_tab`
- `write_find_empty_pattern_exits_2_with_error` (if not already)
- `write_help_text_for_force_describes_overwrite_semantics` (string match on `--help` output)

Each must fail on current binary before any code change.

## Gate

No implementation until founder reviews Level 1 + CE round comes back empty. Level 2 handles the single-line diff and help-text wording.

## Out of scope (explicitly)

- `--find` regex mode.
- `--replace-all` or count-controlled replacements.
- `--force` rename (feature freeze).
- Multi-file `--find`.
- `--find` across line boundaries in single-line mode (handled automatically by escape expansion).
- BR-38 remains orphaned; no standalone slice proposed yet; park for Phase 0 round 2.
