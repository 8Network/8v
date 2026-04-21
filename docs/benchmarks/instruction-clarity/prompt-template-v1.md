# Prompt Template v1 — Instruction Clarity Benchmark

Before running: replace all three `{{...}}` placeholders with real values.
Do NOT edit the 24 questions. See `README.md` for fill-in protocol.

---

```
You are being tested on the clarity of an instruction document for a tool called `8v`. Answer all 24 questions honestly. **Do NOT read any file from the repo to verify answers.** Work purely from the text below. That is the point of the test — we want to know what an agent who only sees these instructions would understand, assume, guess, and miss.

---

# INSTRUCTIONS UNDER TEST

## Surface 1: CLAUDE.md block (v0.1.0, injected by `8v init`)

{{SURFACE_1_CLAUDEMD}}

## Surface 2: MCP tool description string

{{SURFACE_2_INSTRUCTIONS_TXT}}

---

# QUESTIONS (answer all 24)

## A. Understanding
1. Summarize 8v in 3 sentences: what, when, the two principles.
2. For each of these commands — `ls`, `read`, `search`, `write`, `check`, `fmt`, `test`, `build` — state the minimum-viable invocation and what it returns by default, in one line each. Mark any you can't answer for.
3. Name the two principles. Explain each in your own words. Give one concrete non-obvious example per principle.
4. When should you use 8v instead of native tools? When NOT?
5. How do you discover what flags a command supports?

## B. Ambiguity — quote it, show dual reading
6. List every phrase that could be interpreted more than one way. Quote exactly, then state each possible reading.
7. List every behavior the instructions imply but never state explicitly. (Error format, exit codes, failure behavior, shell-escape rules, unicode, JSON shape, what happens when path doesn't exist, output interleaving, etc.)
8. Any terms used without definition? (e.g., "symbol map", "stack", "progressive", "most Bash", "overhead", "schema tax", "compact mode")
9. Any contradictions between the two surfaces or within either?
10. When the docs say `8v read a.rs b.rs Cargo.toml`, does the output interleave? Concatenate? One symbol map per file? How do you know from the text?
11. `8v write <path>:<line> "<content>"` — does `<content>` get a trailing newline automatically? How are multi-line contents written? Do the surrounding quotes have shell-escape meaning?

## C. Concrete commands + confidence (1–5, plus one-line reasoning per scenario)
12. For each scenario give: (a) exact command, (b) confidence 1–5, (c) one-line reasoning. If you'd use a native tool instead, say so honestly.

    a. Read 5 files at once.
    b. Replace lines 10–20 of `foo.rs` with new content spanning 3 lines.
    c. Find all functions named `handle_*` across the repo.
    d. Append one line to `notes.md`.
    e. Symbol map for `bar.rs`, then read lines 100–150.
    f. Run tests and parse JSON output.
    g. Check whether a file exists before reading.
    h. Delete lines 50–60.
    i. Insert a new line before line 30.
    j. Search only Rust files, case-insensitive, for `TODO`.
    k. Find all files by name matching `*_test*.md`.
    l. Run lint + format-check + type-check with one command.
    m. Replace `old_name` with `new_name` across a multi-file refactor.
    n. Read just the symbols of 10 files in one call.
    o. Read lines 1–200 and lines 500–600 of `big.rs` in one call.

13. For each command (`ls`, `read`, `search`, `write`, `check`, `fmt`, `test`, `build`), mark whether the instructions teach it by **example**, **description-only**, or **not mentioned**.

## D. Behavioral prediction
14. Predict the three most likely mistakes you would make if you followed these instructions on real work. Be specific.
15. What would cause you to fall back to Bash/Read/Edit/Grep/Glob instead of 8v?
16. Which commands would you use most? Least? Why?
17. What's the first command you'd run in a new repo and why?
18. If `8v write` failed, would you trust the error to tell you what to do next?

## E. Missing / wished
19. What's missing that you wished was there? (Flags, examples, error guidance, glossary.)
20. If you had to teach 8v to someone using only these instructions, where would you hesitate?
21. Compared to native Bash+Read+Edit+Grep+Glob, what does 8v do better? Worse?
22. What one instruction edit would have the biggest positive impact on your ability to use 8v correctly?

## F. Overall
23. Rate overall clarity 1–10 and explain the rating.
24. If you had one minute to improve these instructions, what would you change?

---

**Format your answer as numbered sections Q1 through Q24. Be concrete. Quotes for ambiguity questions. Exact commands for scenario questions. Do NOT add commentary outside the question structure.**

At the end, add: **Model & run ID:** {{MODEL_RUN_ID}}.
```
