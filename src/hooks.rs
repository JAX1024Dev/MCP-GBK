//! Local, read-only Claude Code hook handlers. No MCP connection or runtime.
use crate::{
    error::{AppError, Result},
    workspace::{Workspace, is_link, path_is_within},
};
use clap::ValueEnum;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub const INPUT_BYTES: u64 = 64 * 1024 * 1024;
const SPECIAL_READ: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "pdf", "ipynb"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum HookKind {
    SessionStart,
    SubagentStart,
    PreToolUse,
}

impl HookKind {
    fn event(self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::SubagentStart => "SubagentStart",
            Self::PreToolUse => "PreToolUse",
        }
    }
}

// Ignore unrelated fields (including Write.content) without retaining their data.
#[derive(Deserialize)]
struct Event {
    hook_event_name: String,
    cwd: Option<PathBuf>,
    tool_name: Option<String>,
    tool_input: Option<ToolInput>,
}

#[derive(Deserialize)]
struct ToolInput {
    file_path: Option<String>,
    path: Option<String>,
}

fn deny(reason: impl AsRef<str>) -> Value {
    json!({"hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "deny",
        "permissionDecisionReason": reason.as_ref()
    }})
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::new("hook_input", message)
}

/// Read one bounded JSON event; a pre-tool failure always becomes a denial.
/// An empty output means no decision, never an approval or permission override.
pub fn run(root: &Path, kind: HookKind, input: impl Read, mut output: impl Write) -> Result<()> {
    let result = (|| {
        let mut bytes = Vec::new();
        input.take(INPUT_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > INPUT_BYTES {
            return Err(invalid("Hook input exceeds 64 MiB"));
        }
        evaluate(root, kind, &bytes)
    })();
    let value = match result {
        Ok(value) => value,
        Err(error) if kind == HookKind::PreToolUse => Some(deny(format!(
            "GBK protection could not verify this operation: {error}. Repair the GBK hook/MCP configuration; do not fall back to built-in text tools."
        ))),
        Err(error) => return Err(error),
    };
    if let Some(value) = value {
        serde_json::to_writer(&mut output, &value).map_err(|error| invalid(error.to_string()))?;
        output.write_all(b"\n")?;
    }
    Ok(())
}

/// Evaluate an event without opening or decoding the target file's contents.
pub fn evaluate(root: &Path, kind: HookKind, input: &[u8]) -> Result<Option<Value>> {
    let event: Event = serde_json::from_slice(input).map_err(|e| invalid(e.to_string()))?;
    if event.hook_event_name != kind.event() {
        return Err(invalid("Hook event does not match --hook"));
    }
    if kind == HookKind::PreToolUse {
        let tool = event
            .tool_name
            .as_deref()
            .ok_or_else(|| invalid("Missing tool_name"))?;
        if !matches!(tool, "Read" | "Edit" | "Write" | "Grep") {
            return Ok(None);
        }
    }
    let ws = Workspace::new(root)?;
    if kind != HookKind::PreToolUse {
        let context = format!(
            "GBK MCP protects text files under {}. Use its read_file, write_file, edit_file, glob and grep tools for workspace text operations (GBK and UTF-8). Built-in Read/Edit/Write/Grep are blocked there, except Read of images, PDFs and notebooks. Glob is available but prefer MCP glob. Read before modifying, pass revision as expected_revision, preserve exact CRLF/LF and specify encoding for ambiguous files. complete=false does not prove no matches. Shell tools remain available for builds/Git/programs, not for reading/searching/rewriting workspace text. If MCP is unavailable, repair its connection; do not bypass protection. Delegate file work only to agents with these MCP tools; an agent without them must return the task to its parent. The protected root stays fixed for this service; work outside it follows the client's existing permissions.",
            ws.root.display()
        );
        return Ok(Some(json!({"hookSpecificOutput": {
            "hookEventName": kind.event(), "additionalContext": context
        }})));
    }
    let tool = event.tool_name.as_deref().expect("checked tool name");
    let args = event
        .tool_input
        .ok_or_else(|| invalid("Missing tool_input"))?;
    let raw = if tool == "Grep" {
        args.path.as_deref().unwrap_or(".")
    } else {
        args.file_path
            .as_deref()
            .ok_or_else(|| invalid("Missing file_path"))?
    };
    if raw.is_empty() || raw.contains('\0') {
        return Err(invalid("Path must be nonempty and contain no NUL"));
    }
    let raw = Path::new(raw);
    let candidate = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        // Drive-relative/root-relative Windows paths depend on process state.
        if raw
            .components()
            .any(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
        {
            return Err(invalid("Ambiguous drive-relative path"));
        }
        let cwd = event
            .cwd
            .as_deref()
            .filter(|p| p.is_absolute())
            .ok_or_else(|| invalid("Relative paths require an absolute event cwd"))?;
        cwd.join(raw)
    };

    // Check original ancestors before canonicalization can hide a link escaping
    // the workspace. Root ancestors such as /var -> /private/var are OS aliases.
    for ancestor in candidate.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) if is_link(&meta) => {
                let located_inside = ancestor.ancestors().any(|p| {
                    p.canonicalize()
                        .is_ok_and(|p| path_is_within(&p, &ws.root) && path_is_within(&ws.root, &p))
                });
                let points_inside = ancestor
                    .canonicalize()
                    .is_ok_and(|p| path_is_within(&p, &ws.root));
                if located_inside || points_inside {
                    return Err(invalid(
                        "Links and junctions involving the protected root are refused",
                    ));
                }
            }
            Err(error)
                if !matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                return Err(error.into());
            }
            _ => {}
        }
    }
    let candidate = candidate
        .to_str()
        .ok_or_else(|| invalid("Path is not Unicode"))?;
    let resolved = match ws.resolve(candidate, true) {
        Ok(path) => path,
        Err(error) if error.code == "outside_root" => return Ok(None),
        Err(error) => return Err(error),
    };
    if tool == "Read" {
        let special = resolved
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| {
                SPECIAL_READ
                    .iter()
                    .any(|allowed| ext.eq_ignore_ascii_case(allowed))
            });
        if special && fs::metadata(&resolved)?.is_file() {
            return Ok(None);
        }
    }
    let replacement = match tool {
        "Read" => "read_file",
        "Edit" => "edit_file",
        "Write" => "write_file",
        "Grep" => "grep",
        _ => unreachable!(),
    };
    Ok(Some(deny(format!(
        "GBK protection: use the GBK MCP {replacement} tool for this workspace operation instead of built-in {tool}. This is a routing decision, not a file read or write. For existing files obtain revision from read_file before editing. If that MCP tool is missing, report the connection/tool-access problem to the user or parent agent; do not use Shell as a fallback."
    ))))
}
