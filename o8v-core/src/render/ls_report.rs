// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Report type for `8v ls` — project and file listing.

use super::output::Output;

/// Result of a listing operation.
pub struct LsReport {
    pub projects: Vec<LsProjectEntry>,
    pub mode: LsMode,
    pub total_files: usize,
    pub files_filtered: usize,
    pub files_skipped_gitignore: usize,
    pub truncated: bool,
    pub shown: usize,
}

/// How the listing was requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LsMode {
    Projects,
    Tree,
    Files,
}

/// A project entry in the listing.
pub struct LsProjectEntry {
    pub name: String,
    pub stack: String,
    pub path: String,
    pub files: Vec<LsFileNode>,
}

/// A single file node in the listing.
pub struct LsFileNode {
    pub path: String,
    pub loc: Option<u64>,
    pub size: Option<u64>,
    pub permissions: Option<String>,
    pub is_symlink: bool,
    pub symlink_target: Option<String>,
    pub is_binary: bool,
    pub is_large: bool,
    pub no_access: bool,
}

impl super::Renderable for LsReport {
    fn render_plain(&self) -> Output {
        let mut out = String::new();

        if self.projects.is_empty() {
            return Output::new("no projects found".to_string());
        }

        match self.mode {
            LsMode::Tree => {
                for entry in &self.projects {
                    let stack_label = if entry.stack.is_empty() {
                        String::new()
                    } else {
                        format!("  [{}]", entry.stack)
                    };
                    out.push_str(&format!(
                        "{}/{}
",
                        entry.path, stack_label
                    ));
                    render_tree_files(&entry.files, &mut out, 1);
                }
                out.push('\n');
                let summary = format!("{} projects, {} files", self.projects.len(), self.shown);
                out.push_str(&summary);
                if self.truncated {
                    out.push_str(&format!(
                        "\nShowing {} of {} files (use --limit to increase)",
                        self.shown, self.total_files
                    ));
                }
                if self.files_filtered > 0 {
                    out.push_str(&format!(", {} files filtered", self.files_filtered));
                }
                if self.files_skipped_gitignore > 0 {
                    out.push_str(&format!(
                        ", {} files skipped (walker errors)",
                        self.files_skipped_gitignore
                    ));
                }
                out.push('\n');
            }
            LsMode::Files => {
                for entry in &self.projects {
                    for file in &entry.files {
                        if file.is_symlink {
                            let target = file.symlink_target.as_deref().unwrap_or("?");
                            out.push_str(&format!("{} → {}", file.path, target));
                        } else if file.no_access {
                            out.push_str(&format!("{}  [no access]", file.path));
                        } else if file.is_large {
                            out.push_str(&format!("{}  [large]", file.path));
                        } else if file.is_binary {
                            out.push_str(&format!("{}  [binary]", file.path));
                        } else if let Some(loc) = file.loc {
                            out.push_str(&format!("{}  {}", file.path, loc));
                        } else {
                            out.push_str(&file.path);
                        }
                        out.push('\n');
                    }
                }
                out.push('\n');
                out.push_str(&format!("{} files", self.shown));
                if self.truncated {
                    out.push_str(&format!(
                        "\nShowing {} of {} files (use --limit to increase)",
                        self.shown, self.total_files
                    ));
                }
                if self.files_filtered > 0 {
                    out.push_str(&format!(", {} files filtered", self.files_filtered));
                }
                if self.files_skipped_gitignore > 0 {
                    out.push_str(&format!(
                        ", {} files skipped (walker errors)",
                        self.files_skipped_gitignore
                    ));
                }
            }
            LsMode::Projects => {
                // Default mode: aligned columns (name, stack, path)
                let max_name = self
                    .projects
                    .iter()
                    .map(|p| p.name.len())
                    .max()
                    .unwrap_or(0);
                let max_stack = self
                    .projects
                    .iter()
                    .map(|p| p.stack.len())
                    .max()
                    .unwrap_or(0);
                for entry in &self.projects {
                    out.push_str(&format!(
                        "{:<width_name$}  {:<width_stack$}  {}\n",
                        entry.name,
                        entry.stack,
                        entry.path,
                        width_name = max_name,
                        width_stack = max_stack,
                    ));
                }
                if self.files_filtered > 0 {
                    out.push_str(&format!("\n{} files filtered", self.files_filtered));
                }
                if self.files_skipped_gitignore > 0 {
                    out.push_str(&format!(
                        "\n{} files skipped (walker errors)",
                        self.files_skipped_gitignore
                    ));
                }
            }
        }

        Output::new(out)
    }

    fn render_json(&self) -> Output {
        let mut total_loc: u64 = 0;
        let mut has_loc = false;

        let projects: Vec<serde_json::Value> = self
            .projects
            .iter()
            .map(|entry| {
                let files: Vec<serde_json::Value> = entry
                    .files
                    .iter()
                    .map(|f| {
                        if let Some(loc) = f.loc {
                            total_loc += loc;
                            has_loc = true;
                        }
                        let mut obj = serde_json::json!({
                            "path": f.path,
                        });
                        if let Some(loc) = f.loc {
                            obj["loc"] = serde_json::Value::Number(serde_json::Number::from(loc));
                        }
                        if let Some(size) = f.size {
                            obj["size"] = serde_json::Value::Number(serde_json::Number::from(size));
                        }
                        if let Some(ref perms) = f.permissions {
                            obj["permissions"] = serde_json::Value::String(perms.clone());
                        }
                        if let Some(ref target) = f.symlink_target {
                            obj["symlink_target"] = serde_json::Value::String(target.clone());
                        }
                        obj
                    })
                    .collect();

                serde_json::json!({
                    "name": entry.name,
                    "stack": entry.stack,
                    "path": entry.path,
                    "files": files,
                })
            })
            .collect();

        let mut output = serde_json::json!({
            "total_projects": self.projects.len(),
            "total_files": self.total_files,
            "shown": self.shown,
            "files_filtered": self.files_filtered,
            "files_skipped_gitignore": self.files_skipped_gitignore,
            "truncated": self.truncated,
            "projects": projects,
        });

        if has_loc {
            output["total_loc"] = serde_json::Value::Number(serde_json::Number::from(total_loc));
        }

        let s = match serde_json::to_string_pretty(&output) {
            Ok(s) => s,
            Err(e) => format!("{{\"error\": \"serialization failed: {}\"}}", e),
        };
        Output::new(format!("{}\n", s))
    }
}

/// Render file nodes grouped by directory with indentation (used in tree mode).
fn render_tree_files(files: &[LsFileNode], out: &mut String, _depth: usize) {
    // Sort files by path so directories group together
    let mut sorted: Vec<&LsFileNode> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));

    let mut current_dir: Option<String> = None;

    for file in sorted {
        let path = std::path::Path::new(&file.path);
        let parent = path.parent().and_then(|p| {
            let s = p.to_str()?;
            if s.is_empty() {
                None
            } else {
                Some(s.to_owned())
            }
        });

        // If the directory changed, emit a directory header
        if parent != current_dir {
            if let Some(ref dir) = parent {
                out.push_str(&format!("  {}/\n", dir));
            }
            current_dir = parent.clone();
        }

        // Display filename only; indent under directory if one exists
        let display = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&file.path);

        let indent = if current_dir.is_some() { "    " } else { "  " };
        let mut line = format!("{}{}", indent, display);

        if file.is_symlink {
            let target = file.symlink_target.as_deref().unwrap_or("?");
            line.push_str(&format!(" → {}", target));
        } else if file.no_access {
            line.push_str("  [no access]");
        } else if file.is_large {
            line.push_str("  [large]");
        } else if file.is_binary {
            line.push_str("  [binary]");
        } else if let Some(loc) = file.loc {
            line.push_str(&format!("  {}", loc));
        }

        line.push('\n');
        out.push_str(&line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderable;

    fn sample_projects() -> LsReport {
        LsReport {
            projects: vec![
                LsProjectEntry {
                    name: "o8v-core".to_string(),
                    stack: "rust".to_string(),
                    path: "o8v-core/".to_string(),
                    files: vec![],
                },
                LsProjectEntry {
                    name: "o8v-cli".to_string(),
                    stack: "rust".to_string(),
                    path: "o8v/".to_string(),
                    files: vec![],
                },
            ],
            mode: LsMode::Projects,
            total_files: 0,
            files_filtered: 0,
            files_skipped_gitignore: 0,
            truncated: false,
            shown: 0,
        }
    }

    #[test]
    fn plain_projects_aligned_columns() {
        let out = sample_projects().render_plain();
        let lines: Vec<&str> = out.as_str().trim().lines().collect();
        assert_eq!(lines.len(), 2);
        // Both projects have same stack width — columns should align
        assert!(lines[0].starts_with("o8v-core"));
        assert!(lines[0].contains("rust"));
        assert!(lines[0].contains("o8v-core/"));
        assert!(lines[1].starts_with("o8v-cli "));
        assert!(lines[1].contains("rust"));
        assert!(lines[1].contains("o8v/"));
    }

    #[test]
    fn plain_files_mode() {
        let r = LsReport {
            projects: vec![LsProjectEntry {
                name: "o8v-core".to_string(),
                stack: "rust".to_string(),
                path: "o8v-core/".to_string(),
                files: vec![LsFileNode {
                    path: "src/main.rs".to_string(),
                    loc: Some(100),
                    size: None,
                    permissions: None,
                    is_symlink: false,
                    symlink_target: None,
                    is_binary: false,
                    is_large: false,
                    no_access: false,
                }],
            }],
            mode: LsMode::Files,
            total_files: 1,
            files_filtered: 0,
            files_skipped_gitignore: 0,
            truncated: false,
            shown: 1,
        };
        let out = r.render_plain();
        assert!(out.as_str().contains("src/main.rs"));
        assert!(out.as_str().contains("1 files"));
    }

    #[test]
    fn json_valid() {
        let out = sample_projects().render_json();
        let v: serde_json::Value = serde_json::from_str(out.as_str()).unwrap();
        assert_eq!(v["total_projects"], 2);
        assert_eq!(v["projects"].as_array().unwrap().len(), 2);
        assert_eq!(v["projects"][0]["stack"], "rust");
    }

    #[test]
    fn tree_truncated_output_ends_with_newline() {
        let r = LsReport {
            projects: vec![LsProjectEntry {
                name: "o8v-core".to_string(),
                stack: "rust".to_string(),
                path: "o8v-core/".to_string(),
                files: vec![],
            }],
            mode: LsMode::Tree,
            total_files: 100,
            files_filtered: 0,
            files_skipped_gitignore: 0,
            truncated: true,
            shown: 50,
        };
        let out = r.render_plain();
        assert!(
            out.as_str().ends_with('\n'),
            "tree output must end with newline; got: {:?}",
            &out.as_str()[out.as_str().len().saturating_sub(8)..]
        );
    }

    // TRUNC-1: JSON must expose separate "shown" and "total_files" fields when truncated.
    #[test]
    fn json_truncated_fields_distinct() {
        let r = LsReport {
            projects: vec![LsProjectEntry {
                name: "o8v-core".to_string(),
                stack: "rust".to_string(),
                path: "o8v-core/".to_string(),
                files: vec![],
            }],
            mode: LsMode::Tree,
            total_files: 100,
            files_filtered: 0,
            files_skipped_gitignore: 0,
            truncated: true,
            shown: 50,
        };
        let out = r.render_json();
        let v: serde_json::Value = serde_json::from_str(out.as_str()).unwrap();
        assert_eq!(v["truncated"], true, "truncated must be true");
        assert_eq!(
            v["shown"], 50,
            "shown must equal the number of entries returned"
        );
        assert_eq!(
            v["total_files"], 100,
            "total_files must reflect the true scanned count, not the truncated shown count"
        );
    }

    #[test]
    fn plain_empty() {
        let r = LsReport {
            projects: vec![],
            mode: LsMode::Projects,
            total_files: 0,
            files_filtered: 0,
            files_skipped_gitignore: 0,
            truncated: false,
            shown: 0,
        };
        let out = r.render_plain();
        assert_eq!(out.as_str(), "no projects found");
    }
}
