# Design: WorkspaceRoot — The Trust Boundary

## Problem

`ProjectRoot` does two jobs:
1. "This is a detected project" (Rust, Python, Go, etc.)
2. "This is the containment boundary for file access" (via `as_containment_root()`)

These are different scopes. A monorepo has one workspace and ten projects. The trust boundary is the workspace, not any single project.

Four commands (read, write, search, ls) don't use ProjectRoot at all — they create their own `ContainmentRoot` from `std::env::current_dir()`. This is disconnected from the workspace that `resolve_workspace()` already resolved.

`resolve_workspace()` walks up to find `.git/` or `.8v/` — that's workspace detection, not project detection. But it returns `ProjectRoot`.

## Separation

**WorkspaceRoot** — the trust boundary. The folder the user trusts. Contains a `ContainmentRoot`. One per session. All file I/O is contained within this.

**ProjectRoot** — a detected project within the workspace. "This is a Rust project at `workspace/backend/`." Many possible per workspace. Used by build/test/check/fmt for project-specific operations.

## Types

```rust
/// The trust boundary — all file I/O is contained within this root.
///
/// For CLI: resolved from CWD (walks up to .git/ or .8v/).
/// For MCP: the client-provided root directory.
///
/// Goes into CommandContext Extensions. Commands get it from there.
pub struct WorkspaceRoot {
    containment: ContainmentRoot,
}

impl WorkspaceRoot {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, FsError> {
        let containment = ContainmentRoot::new(path)?;
        Ok(Self { containment })
    }

    pub fn containment(&self) -> &ContainmentRoot { &self.containment }
    pub fn as_path(&self) -> &Path { self.containment.as_path() }

    /// Resolve a relative path within the workspace.
    pub fn resolve(&self, path: &str) -> Result<PathBuf, String> {
        let abs = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.containment.as_path().join(path)
        };
        // Containment check happens at the safe_* call site, not here.
        Ok(abs)
    }
}
```

`ProjectRoot` stays as-is but loses the "trust boundary" responsibility. It's detected within the workspace.

## Context

```rust
// build_context puts WorkspaceRoot in Extensions.
extensions.insert(workspace_root);  // trust boundary
extensions.insert(storage);         // ~/.8v/
extensions.insert(bus);             // event bus

// ProjectRoot is NOT in context — it's per-command, per-path-argument.
```

## resolve_workspace Changes

Today:
```rust
pub fn resolve_workspace(path) -> Result<(ProjectRoot, StorageDir, Config), Error>
```

After:
```rust
pub fn resolve_workspace(path) -> Result<(WorkspaceRoot, StorageDir, Config), Error>
```

Workspace detection (walk up to `.git/` or `.8v/`) returns `WorkspaceRoot`.
Project detection (`o8v_project::detect_all`) happens per-command when needed.

## Command Changes

### read.rs — before
```rust
fn read_to_report(args: &Args) -> Result<ReadReport, String> {
    let abs_path = resolve_path(&file_path)?;
    let root = std::env::current_dir()...ContainmentRoot::new(&cwd)...;  // CWD bypass
    let file = safe_read(&abs_path, &root, &config)?;
}
```

### read.rs — after
```rust
fn read_to_report(args: &Args, ctx: &CommandContext) -> Result<ReadReport, String> {
    let ws = ctx.extensions.get::<WorkspaceRoot>()
        .ok_or("8v: no workspace")?;
    let abs_path = ws.resolve(&file_path)?;
    let file = safe_read(&abs_path, ws.containment(), &config)?;
}
```

### build.rs — no change needed
```rust
// build uses the path argument to detect the project, not WorkspaceRoot.
// It runs external processes, not file I/O through safe_*.
let root = ProjectRoot::new(&abs_path)?;
let projects = detect_all(&root);
```

### MCP handler — provides WorkspaceRoot from client roots
```rust
// MCP client tells us which directories to trust.
let root_path = get_root_directory(&client).await;
let workspace = WorkspaceRoot::new(root_path)?;
// This goes into context instead of CLI's CWD-based workspace.
```

## What Lives Where

| Type | Crate | Purpose |
|------|-------|---------|
| ContainmentRoot | o8v-fs | Security primitive. Every safe_* call needs one. |
| WorkspaceRoot | o8v-workspace | Trust boundary. One per session. In context. |
| ProjectRoot | o8v-project | Detected project. Per-command. NOT in context. |
| StorageDir | o8v-workspace | ~/.8v/ storage. In context. |

## Migration Order

1. Create `WorkspaceRoot` type in o8v-workspace
2. Change `resolve_workspace()` to return `WorkspaceRoot` instead of `ProjectRoot`
3. Update `build_context()` to put `WorkspaceRoot` in Extensions (instead of `ProjectRoot`)
4. Update commands: read/write/search/ls get `WorkspaceRoot` from context
5. Update commands: build/test/check/fmt create `ProjectRoot` from path argument
6. Remove `ProjectRoot` from context Extensions
7. Update MCP to provide `WorkspaceRoot` from client roots

## Open Questions

1. Where does `WorkspaceRoot` live? `o8v-workspace` makes sense by name. But `o8v-workspace` depends on `o8v-fs` (for ContainmentRoot), which is correct.

2. Should `WorkspaceRoot` be in `o8v-fs` instead? It's fundamentally a containment concept. Counter: it's a workspace concept that USES containment.

3. What if there's no workspace? (No `.git/`, no `.8v/`.) Today `resolve_workspace` fails. Should `build_context` fall back to CWD as the trust boundary?
