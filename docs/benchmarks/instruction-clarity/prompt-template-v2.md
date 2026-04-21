# Prompt Template v2 — Instruction Clarity Benchmark

**Changes from v1:** Q23 expanded to 3-axis rubric. Q25-Q33 added (output contracts,
behavioral dry-run, tool-gap surfacing, memorability). Q1-Q22, Q24 unchanged.
Do NOT edit the 33 questions. See `README.md` for fill-in protocol.

---

```
You are being tested on the clarity of an instruction document for a tool called `8v`. Answer all 33 questions honestly. **Do NOT read any file from the repo to verify answers.** Work purely from the text below. That is the point of the test — we want to know what an agent who only sees these instructions would understand, assume, guess, and miss.

---

# INSTRUCTIONS UNDER TEST

## Surface 1: CLAUDE.md block (injected by `8v init`)

{{SURFACE_1_CLAUDEMD}}

## Surface 2: MCP tool description string

{{SURFACE_2_INSTRUCTIONS_TXT}}

---

# QUESTIONS (answer all 33)

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
23. Rate overall clarity on three independent axes. For each axis: score 1–10 and one sentence explaining it.
    - Axis 1 — Input clarity: how well do the instructions explain what to pass in?
    - Axis 2 — Output clarity: how well do they explain what comes back on success?
    - Axis 3 — Failure-mode clarity: how well do they explain what happens when something goes wrong?
    - Composite mean: (axis1 + axis2 + axis3) / 3 (round to 2 decimal places).

24. If you had one minute to improve these instructions, what would you change?

## G. Output contracts (predict from text only — do not guess beyond what is stated)
25. For the inline Python fixture below, predict the exact output of each `8v` command. If the instructions do not contain enough information to predict the output with confidence, say so and quote the gap.

```python
# util.py  (4 lines)
def add(a, b):
    return a + b

result = add(1, 2)
```

    a. `8v read util.py` — what is returned?
    b. `8v read util.py:1-2` — what is returned?
    c. `8v search "add" util.py` — what is returned?
    d. `8v read util.py --full` — what is returned?

26. For `8v check .`: what exit code does it return when a lint error is found? What exit code on success? Where does the error text appear — stdout, stderr, or only in `--json`? Quote the instructions if they state this; otherwise name the gap.

27. For `8v write <path> --find "<old>" --replace "<new>"`: what happens if `<old>` appears zero times? What if it appears more than once? What is returned to the caller? Quote or name the gap.

28. For `8v read <path>` on a path that does not exist: what does the caller receive? An error on stdout? On stderr? A non-zero exit code? A structured JSON error? Quote or name the gap.

## H. Contract reasoning
29. An agent reads `8v check .` output and sees no output on stdout and no output on stderr, but the process exits with code 1. Based only on the instructions: is this expected behavior? What should the agent do next? Quote any relevant instruction text or name the gap.

## I. API coherence
30. Surface 1 and Surface 2 both describe `8v`. List every factual difference between them — commands, flags, examples, or behavior described differently. For each: which surface is more complete and why it matters to an agent that sees only one surface.

## J. Tool-gap surfacing
31. Describe a realistic coding task that you cannot complete using only `8v` commands as described in the instructions. What is the missing capability? What is the closest substitute and its cost?

## K. Behavioral dry-run
32. Walk through the following 5-step task using only `8v` commands. For each step: write the exact command, state your confidence (1–5), and flag any step where you would need to fall back to a native tool.

    1. Find all Go source files in the repo.
    2. Search for all usages of `http.Get` across those files, with 2 lines of context.
    3. Read the symbol map of the file with the most matches.
    4. Replace the word `http.Get` with `httpClient.Get` on a specific line in that file.
    5. Run the tests and confirm they pass.

## L. Memorability
33. Without re-reading the instructions above: write the exact syntax for `8v write` to insert a new line before line 42 of `main.rs`. Then write the syntax to replace lines 10–20 with multi-line content. State your confidence for each (1–5).

---

**Format your answer as numbered sections Q1 through Q33. Be concrete. Quotes for ambiguity questions. Exact commands for scenario questions. Do NOT add commentary outside the question structure.**

At the end, add: **Model & run ID:** {{MODEL_RUN_ID}}.
```
