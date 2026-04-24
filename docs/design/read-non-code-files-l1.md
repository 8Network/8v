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
violation. If an agent can satisfy a read via native tools, it will. Once an agent routes around 8v
for one read, the session is split: the agent now holds file content obtained outside 8v's
progressive model and the batch affordance is lost.

The immediate trigger: agents under test (Claude Code, Codex) are observed falling back to native
`Read` for any non-`.rs`/`.py`/`.go` file. The fallback is silent.

---

## §2 File type taxonomy

Three categories. The boundary criterion: what can an AI agent do with the output?

**Category 1 — Text-like non-code.** SVG, Markdown, CSV, TOML, YAML, JSON, plain `.txt`, `.html`,
`.xml`. Valid UTF-8. The agent reads the raw text. Symbol map is empty → fall back to full content.

**Category 2 — Readable binary.** PDF, PNG, JPG, GIF, WebP, TIFF, BMP, ICO. Not valid UTF-8. A
multimodal agent can process them given base64 + MIME. Without base64 delivery from 8v, the agent
leaves 8v.

**Category 3 — Opaque binary.** ZIP, EXE, DLL, `.so`, `.o`, Wasm, `.class`, `.jar`. No meaningful
agent representation. Structured error only.

---

## §3 Governing principle

**Classification drives behavior; the user supplies no flag.** If a file is a readable binary,
`8v read` returns the binary content — period. The progressive principle (minimum useful default +
flags for detail) does not apply here because "metadata-only" is not useful: `8v ls --loc` already
surfaces size, and the agent called `read` precisely to obtain content. Requiring a `--binary` flag
is the kind of opt-in the project rejects ("enforce, don't instruct"). vast-tools got this right
with automatic dispatch; the remaining improvement is the MCP layer.

---

## §4 Design

### 4.1 Classification

New `o8v-core::mime` module:

- `FileKind { Text, ReadableBinary, OpaqueBinary }`
- `detect_kind(ext: &str) -> FileKind` — case-insensitive, unknown → `Text`.
- `mime_for_ext(ext: &str) -> Option<&'static str>` — populated for images, PDFs, archives.

SVG classified as `Text` (XML). Readable-binary set: pdf, png, jpg, jpeg, gif, webp, bmp, tiff, tif,
ico. Opaque: zip, tar, gz, tgz, bz2, xz, 7z, rar, exe, dll, so, dylib, a, o, wasm, class, jar.

### 4.2 Byte-level safe read

`o8v-fs::safe_read_bytes(path, root, config) -> Result<Vec<u8>, FsError>` mirrors the `safe_read`
guard pipeline (canonicalize, containment, pre-open type check, open, TOCTOU re-check on fd, size
check against `max_file_size`) and returns raw bytes. No BOM handling.

### 4.3 `ReadReport` shape

One new variant:

```rust
BinaryContent { path: String, mime_type: String, size_bytes: u64, base64: String }
```

Opaque binaries return `Err(String)` with MIME + size. No `BinaryMeta` variant — metadata-only is
not a read result.

### 4.4 CLI behavior

- `8v read path` on readable binary → `BinaryContent` always. No flag.
- `8v read path` on opaque binary → structured error naming MIME and size.
- Plain render for `BinaryContent`:
  ```
  {path}: {mime}, {size} bytes
  base64: {b64}
  ```
- `--json` returns the `BinaryContent` variant directly.
- Text-like files with empty symbol map (SVG, Markdown, TOML, YAML, JSON, etc.) fall back to
  `ReadReport::Full` — raw text, which is what the agent wants. No flag.
- Size ceiling: the existing `FsConfig::max_file_size` (10 MB) is the real gate. No new limit.

### 4.5 MCP behavior — where 8v goes beyond vast-tools

vast-tools serialized everything to JSON text and wrapped in one `TextContent` block. rmcp 1.3.0
supports `Content::image(b64, mime)` and `Content::resource(ResourceContents)` natively. 8v uses
them.

- MCP tool return type changes from `Result<String, String>` to `Result<Vec<Content>, String>`
  (`IntoContents` for `Vec<Content>` is provided by rmcp).
- `handle_command` returns a typed value the MCP layer maps to content blocks:
  - `ReadReport::BinaryContent` with MIME `image/*` → `Content::image(base64, mime)`.
  - `ReadReport::BinaryContent` with MIME `application/pdf` → try `Content::resource(EmbeddedResource)`;
    requires a round-trip verification that Claude renders `EmbeddedResource` for PDFs. If it does
    not, fall back to `Content::text` with `"{path}: {mime}, {size} bytes\nbase64: {b64}"`.
  - `ReadReport::Multi` with mixed entries → one `Content` block per entry in the vec (text blocks
    for symbol maps/ranges/full text, image/resource blocks for binaries).
  - Everything else → single `Content::text` as today.
- Output cap (`DEFAULT_OUTPUT_CAP = 50_000`) applies only to text streams. Binary content blocks
  are exempt — the cap exists to prevent dumping large rendered source; image/resource blocks are
  opaque payloads the client handles. The 10 MB `max_file_size` is the effective ceiling.

### 4.6 Scope of change

- `read.rs`: drop `binary: bool` arg; always return `BinaryContent` for readable binaries.
- `read_report.rs`: delete `BinaryMeta` variant.
- `mcp/mod.rs` + `mcp/handler.rs`: new return type; branch on `ReadReport` variant to emit correct
  content blocks.
- Tests:
  - CLI: `8v read image.png` returns base64 without a flag.
  - CLI: `8v read archive.zip` returns structured error.
  - CLI: `8v read icon.svg` returns raw XML (empty-symbol-map fallback).
  - MCP round-trip: PNG → `ImageContent`; PDF → `EmbeddedResource` (or text fallback if Claude
    does not render); mixed batch of `.rs` + `.png` → two content blocks.

---

## §5 Non-goals

- Text extraction from PDFs. The multimodal model does that. 8v delivers bytes.
- Image transforms (thumbnail, downscale). Out of scope; Phase 0.
- `--binary` / `--no-binary` flags. Rejected (§3).
- Schema version field. Additive variant; existing consumers ignore unknown variants.
