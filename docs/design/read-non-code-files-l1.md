# Design: `8v read` non-code file support (Level 1)

## §1 Problem statement

**PDF** — `8v read diagram.pdf` crashes: `file contains invalid UTF-8`. The agent receives no
content and no structured error. It falls back to Claude Code's native `Read` tool, which handles
PDFs natively. The "8v is the only tool" contract is broken.

**PNG / JPG** — Same crash. The agent falls back to native `Read` (multimodal). Same contract
break. No structured error means the agent cannot distinguish "binary file" from a bug.

**SVG** — `8v read icon.svg` returns an empty symbol map: `(no symbols found)`. No content is
delivered. The agent either calls `--full` (a retry turn) or falls back to native. SVG is XML — the
content is directly readable — but 8v treats it as a no-symbol code file and gives nothing useful.

**Why this matters for AI agents specifically.** Every fallback to a native tool is a contract
violation. If an agent can satisfy a read via native tools, it will, because native tools are
already loaded and their schemas are eagerly available. Once an agent routes around 8v for one
read, the session is split: the agent now holds file content obtained outside 8v's progressive
model and the batch affordance is lost. Token cost and retry count both rise. The benchmark already
shows that "discovery phase mandatory" is the dominant cost driver — a second discovery layer via
native tools compounds that.

The immediate trigger: agents under test (Claude Code, Codex) are observed falling back to native
`Read` for any non-`.rs`/`.py`/`.go` file. The fallback is silent — no log entry, no user signal.

---

## §2 File type taxonomy

Three categories. The boundary criterion is: what can an AI agent do with the output?

**Category 1 — Text-like non-code**

SVG, Markdown, CSV, TOML, YAML, JSON, plain `.txt`, shell scripts without a recognized extension,
`.html`, `.xml`.

These are valid UTF-8. The agent can read the raw text and act on it: parse a CSV, inspect a
config, read a diagram's XML. They have no symbol map worth producing, but `--full` already handles
them. The only fix needed is graceful handling — no crash, no silent empty map — plus a hint when
the symbol map is empty (already designed in `read-empty-symbol-map-hint.md`).

Justified as one category because the agent's action is the same for all: read the text.

**Category 2 — Readable binary**

PDF, PNG, JPG, GIF, WebP, TIFF, and SVG when stored as binary (rare but possible).

These are not valid UTF-8. The agent cannot read the raw bytes as text. However, a multimodal AI
agent *can* process them if given base64 + MIME type — the same contract Claude Code's native Read
uses for images and PDFs. Without base64 delivery from 8v, the agent must leave 8v to read these
files. That is the contract break.

Justified as one category because the agent's action is the same: decode base64 + use MIME to
determine how to process (render as image, parse as PDF, etc.).

**Category 3 — Opaque binary**

ZIP, EXE, DLL, `.so`, `.o`, Wasm, compiled Java `.class`, compiled Python `.pyc`.

No meaningful representation for an AI agent. Base64 of a ZIP is useless — the agent cannot
decompress it, inspect the archive entries, or act on the bytes. Delivering base64 here costs
tokens with zero agent utility. The correct response is a structured error: file type, MIME, size,
and an explicit message that the file is not readable by this tool.

Justified as one category because the correct agent action is the same: stop, report the
limitation, let the human decide.

---

## §3 Design options

### Option A — Extend output schema (add `binary_base64` + `mime_type` fields)

Mirrors what vast-tools `fs.read` did. Every read response gains two optional fields.
Text files: both null. Readable binary: `mime_type` set, `binary_base64` set. Opaque binary:
`mime_type` set, `binary_base64` null (or omit with a structured error field).

**Example output (PNG, `--json`):**
```json
{
  "mime_type": "image/png",
  "binary_base64": "iVBORw0KGgoAAAANSUhEUgAA...",
  "size_bytes": 14823
}
```

**Plain text output (PNG, no flags):**
```
icon.png (14.5 KB, image/png)

  [binary — base64 content omitted in plain output; use --json to retrieve]
```

Token cost: base64 of a 1 MB PNG ≈ 1.37 MB ≈ ~340K tokens. A 14 KB PNG ≈ ~5K tokens. Cost is
proportional to file size and always paid, even if the agent only needs metadata.

Agent can do: pass base64 to multimodal model, show image, extract text from PDF.
Agent cannot do: avoid paying the token cost when it only needs to know "is this a PNG and how
large?"

---

### Option B — Progressive: metadata-first, base64 behind `--binary`

Default `8v read image.png` returns metadata only. Base64 content requires `--binary`.

**Example output (PNG, default):**
```
icon.png
  type:  image/png
  size:  14.5 KB
  hint:  use `8v read icon.png --binary` to retrieve base64 content
```

**Example output (PNG, `--binary`, `--json`):**
```json
{
  "mime_type": "image/png",
  "size_bytes": 14823,
  "binary_base64": "iVBORw0KGgoAAAANSUhEUgAA..."
}
```

**Example output (opaque binary, default):**
```
archive.zip
  type:  application/zip
  size:  2.3 MB
  error: opaque binary — not readable by 8v read
```

Token cost (default): ~40 tokens regardless of file size. Token cost (`--binary`): same as Option A
— proportional to file size, paid on demand.

Agent can do (default): determine MIME, decide whether to request base64 or report to user.
Agent can do (`--binary`): same as Option A.
Agent cannot do: avoid the explicit `--binary` flag when it always needs the content — costs one
extra round-trip vs Option A for cases where base64 is always needed.

---

### Option C — Minimal: structured error only, no base64

No base64 delivery at all. Non-code files that cannot produce a symbol map return a structured
error with MIME type and size. Text-like non-code still works via `--full`. Readable binaries get
the same structured error as opaque binaries, with a note that native tools can read them.

**Example output (PNG, default):**
```
icon.png
  type:  image/png
  size:  14.5 KB
  error: binary file — use a multimodal tool to read images
```

**Example output (SVG, `--full`):**
```xml
<svg xmlns="http://www.w3.org/2000/svg" ...>
  ...
</svg>
```

Token cost: ~40 tokens for any binary. No base64 ever.

Agent can do: get a clean signal instead of a crash; route to native tool for images/PDFs with
knowledge of the MIME type.
Agent cannot do: read images or PDFs inside 8v — the agent must still leave 8v for Category 2
files. The contract break is not resolved, only made less violent (structured error vs crash).

---

## §4 Recommendation

**Option B.**

It satisfies the "8v is the only tool" contract for Category 2 files (base64 available via
`--binary`) while keeping the default output token-minimal — consistent with 8v's progressive
principle. Option A always pays the base64 token cost even when the agent only needs to know the
MIME type; Option C leaves the contract break intact for images and PDFs.

---

## §5 Open questions (require founder input before implementation)

1. **SVG classification.** SVG is XML text and readable as-is. Should `8v read icon.svg` return
   the raw XML (Category 1, text-like) or base64 via `--binary` (Category 2, readable binary)?
   The distinction matters because SVGs are sometimes used as code artifacts and sometimes as
   opaque image assets. Current behavior (empty symbol map) satisfies neither.

2. **Max file size gate for `--binary`.** vast-tools used 20 MB. For 8v, a 20 MB PDF ≈ 27 MB
   base64 ≈ ~7M tokens — likely exceeding context limits and defeating the purpose. Is 20 MB the
   right gate, or should it be lower (e.g. 5 MB)?

3. **`--json` schema versioning.** New fields (`mime_type`, `size_bytes`, `binary_base64`) can be
   added as optional to the existing schema without a version bump — existing consumers ignore
   unknown fields. Or a `schema_version` field can be introduced now as a forward-compatibility
   hook. Which is preferred?
