# Requirement Before Refactor

## The Process

When you inherit complex code, the instinct is to refactor it — move it, restructure it, make it cleaner. But refactoring preserves assumptions. If the assumptions were wrong, the refactored code is still wrong, just tidier.

Before touching implementation, go back to zero:

1. **What does the user need?** Not what the code does. What the user actually needs.
2. **What is the minimum required to deliver that?** No bias from what exists. Fresh.
3. **Does the existing code match?** Usually it overshoots. Sometimes it misses entirely.

The gap between "what exists" and "what's required" is where you find the lines to delete.

## Example: Check Delta

EventWriter — 700 lines. SHA256 diagnostic IDs. Series accumulation. File caching. Log rotation. Merge logic. Finalize lifecycle. A parallel event system that bypassed the EventBus.

We were about to migrate it onto the EventBus. Design a DiagnosticEvent type. Build a SeriesSubscriber. Keep the SHA256 IDs. Keep the series merge. Keep everything — just move it to a better home.

Same complexity, different address.

**Step 1: What does the user need?**

A developer runs `8v check` and wants to know: is it getting better or worse?

**Step 2: What is the minimum required?**

Two snapshots. The previous check result and the current one. Compare.

**Step 3: Does the existing code match?**

No. It accumulated diagnostics forever, computed SHA256 IDs for deduplication, tracked first_seen timestamps, counted run appearances, maintained per-run logs nothing ever read, rotated those logs at 500 files. The delta was also broken — series.json only grew, so "fixed" was always zero.

**Result:** Delete 700 lines. Store last result. Compare with current. Done.
