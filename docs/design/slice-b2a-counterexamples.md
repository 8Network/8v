# Slice B2a — counterexample attack list

Adversarial read of `slice-b2-decomposition.md` §B2a ("STDERR channel discipline"). Every attack below is a concrete scenario the implementation must handle. If any cannot be answered, B2a is not ready for Level 2.

Author's note: I drafted B2a and this list. Founder should review as fresh eyes — do not treat this as "already reviewed".

## Attacks

**A1. Piped-to-pager breaks stderr.**
Agent runs `8v search foo | less`. stderr bypasses the pager and interleaves with pager output or scrolls past. Is this the agent's problem or ours?
Proposed answer: ours. The instruction surfaces should document that errors go to stderr and agents should capture stderr separately. Not a B2a blocker; a doc note in B1.

**A2. JSON mode rule.**
B2a says "errors to stderr." But B2b says "JSON errors to stdout with stderr empty." What does B2a alone do when `--json` is set — does it move errors to stderr, contradicting B2b?
Resolution needed: B2a MUST preserve current stdout-on-json behavior for the path that will be formalized in B2b. I.e. B2a only moves **human-formatted** errors, not JSON. Document this in B2a.

> Resolved 2026-04-20: JSON errors stay on stdout per error-contract §2.3. B2a moves only human-formatted errors to stderr; `--json` path is untouched by B2a and formalized in B2b.

**A3. Pass-through commands' subprocess stderr.**
`8v test .` invokes `cargo test`. Cargo writes to both stdout and stderr. If we blindly redirect subprocess output to our stdout/stderr streams, we preserve cargo's split. If we unify, we destroy it. B2a says "pass-through commands keep subprocess stderr → process stderr." Verify that every pass-through command today actually does this — or B2a silently breaks cargo's native channel routing.

**A4. Progress/spinner output.**
Some commands emit "Running X..." or a progress bar. These are not errors. Are they on stdout, stderr, or a TTY escape? If they're on stderr today, do they still go there after B2a, or are they re-routed to stdout? Decision needed.
Proposed answer: progress stays on stderr. Stderr is "human side-channel" — errors + progress.

**A5. Test infrastructure assertions.**
Existing tests probably assert on stdout/stderr in specific ways. B2a will break some of them. Estimate: which tests need updating? Is that cost bounded? Level 2 must count them before implementation.

**A6. The event-log emitter.**
Events land in `~/.8v/events.ndjson`. Do errors also land there? If yes, do they get formatted twice (once to ndjson, once to stderr)? Does B2a change the format? Trace: when an error is produced, where does it emit first — ndjson, then stderr? Or both from the same call site?
Blocker: Level 2 must trace this path. The event log is our observability surface; corrupting it is worse than a printing bug.

> CLOSED 2026-04-20 — clean (traced event_bus, storage_subscriber, tracing subscriber; all error paths use tracing::* → stderr or return Result; no println!/print! in emit paths)

**A7. The MCP handler.**
`8v mcp` runs as a JSON-RPC server on stdin/stdout. It cannot print to stdout for human messages — that would corrupt the RPC stream. Does B2a's "errors to stderr" cover the MCP case? It should, trivially. Verify: every error in `o8v/src/mcp/` currently goes to stderr.

> Pre-L2 required: run a code survey of `o8v/src/mcp/` before Level 2 begins. Confirm every error-emit site goes to stderr. If any writes to stdout, that site corrupts the RPC stream and must be flagged as a separate bug — not fixed silently in B2a. Survey result must be in the L2 doc before implementation starts.

**A8. Single shared render function.**
If there's one `render::emit_error` function today used by all commands, B2a is a 1-file change. If each command rolls its own, B2a touches N files. Which is it? Level 2 must answer.

> Pre-L2 required: run a render-function topology survey before Level 2 begins. Search `o8v/src/` for all error-emit call sites (`eprintln!`, `writeln!(stderr`, any `render::` calls). Count distinct call sites. If N > 5, B2a's "scope" estimate in the decomposition doc may be materially wrong and the slice may need to be split further. Survey result must be in the L2 doc.

**A9. Partial failure on batch read.**
`8v read a.rs missing.rs` harvests per CE-2 resolution: each entry's error embeds inline under `=== label ===` on stdout. That's not an error — it's a result with partial content. B2a must NOT move these. Document: "per-entry batch errors stay on stdout as part of the result stream; process-level errors go to stderr."

**A10. Exit code carrier.**
Today some commands may be printing an error AND exiting 0. B2a alone doesn't fix that — B2c does. But if B2a's tests assert "stderr non-empty" without also asserting "exit non-zero," we ship a state where stderr has an error but `$?` says success. Transiently wrong. Decide: do we ship B2a before B2c?

**A11. Colored output.**
Red stderr in terminals. Do we preserve/add ANSI codes? B2a says "no shape change" — implicit "no color change". Confirm.

**A12. BrokenPipe on stderr.**
`main.rs` already handles BrokenPipe on stdout writes. Does stderr need the same? After B2a, more writes target stderr — BrokenPipe on stderr is now a real path. Level 2: route through same helper.

**A13. Locale / byte ordering.**
Error messages may contain non-ASCII (filenames with UTF-8). Writing to stderr under some terminals transcodes. Ensure stderr writes are raw bytes — no locale-based translation.

**A14. Testing in the same process.**
Some integration tests call `dispatch_command` directly (not via subprocess). They won't see stderr; the helper must be captured/swapped for test mode. Does test infra already support this? If not, Level 2 must add it or the tests are invalid.

**A15. Backward compatibility for scripts.**
Someone's CI script today does `8v check . 2>/dev/null` to suppress error text. After B2a, more output goes to stderr — same suppression still works, BUT if anyone parses stdout for error text, they silently lose the signal. Not our problem; CI scripts that rely on stdout-formatted errors are brittle. Document in the PR that this is a channel-routing change.

## What passes the gate

B2a is ready for Level 2 only when:
- A2 resolved (2026-04-20): JSON errors stay on stdout per error-contract §2.3; B2a scope is human-formatted errors only.
- A10 decides the B2a-before-B2c ship order.
- A5 has a bounded test-update count.
- A6, A7, and A8 each require a pre-L2 survey to be completed and documented before Level 2 starts — see their individual BLOCKED ON FOUNDER notes above. These surveys are entry conditions for Level 2, not answers needed at Level 1.

Everything else can be addressed in Level 2 design.

## What doesn't

A1, A4, A11, A13, A15 are "nice to document" but don't block.
A3, A9 are doc clarifications that update B2a's scope wording before ship.
A12, A14 are Level 2 implementation details.

## Verdict (reviewer's)

Gate: Ready for Level 2 after founder confirms slice order and CE reviewer assignment (remaining founder gates).
