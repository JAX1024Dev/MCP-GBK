use gbk_mcp::hooks::{self, HookKind};
use serde_json::{Value, json};
use std::{fs, io::Read, path::Path};

fn event(tool: &str, cwd: &Path, args: Value) -> Value {
    json!({"hook_event_name":"PreToolUse","cwd":cwd,"tool_name":tool,"tool_input":args})
}
fn call(root: &Path, kind: HookKind, event: Value) -> Option<Value> {
    let bytes = serde_json::to_vec(&event).unwrap();
    let mut output = Vec::new();
    hooks::run(root, kind, bytes.as_slice(), &mut output).unwrap();
    if output.is_empty() {
        None
    } else {
        Some(serde_json::from_slice(&output).unwrap())
    }
}
fn rejected(value: Option<Value>, expected: &str) {
    let value = value.expect("operation must be rejected");
    assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(
        value["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .contains(expected),
        "{value}"
    );
}

#[test]
fn routes_all_workspace_text_without_reading_or_creating_it() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("legacy.c"), [0xff, 0, 0x81]).unwrap();
    for (tool, replacement) in [
        ("Read", "read_file"),
        ("Edit", "edit_file"),
        ("Write", "write_file"),
        ("Grep", "grep"),
    ] {
        let args = if tool == "Grep" {
            json!({"path":"legacy.c"})
        } else {
            json!({"file_path":"legacy.c","content":"ignored"})
        };
        rejected(
            call(
                temp.path(),
                HookKind::PreToolUse,
                event(tool, temp.path(), args),
            ),
            replacement,
        );
    }
    rejected(
        call(
            temp.path(),
            HookKind::PreToolUse,
            event(
                "Write",
                temp.path(),
                json!({"file_path":"missing/parents/new.txt"}),
            ),
        ),
        "write_file",
    );
    assert!(!temp.path().join("missing").exists());
    assert_eq!(
        fs::read(temp.path().join("legacy.c")).unwrap(),
        [0xff, 0, 0x81]
    );
    // Even a huge sparse file is handled with metadata, not decoded or hashed.
    let large = fs::File::create(temp.path().join("large.txt")).unwrap();
    large.set_len(128 * 1024 * 1024).unwrap();
    rejected(
        call(
            temp.path(),
            HookKind::PreToolUse,
            event("Read", temp.path(), json!({"file_path":"large.txt"})),
        ),
        "read_file",
    );
}

#[test]
fn special_reads_are_case_insensitive_but_edits_remain_protected() {
    let temp = tempfile::tempdir().unwrap();
    for extension in ["PNG", "jpg", "JPEG", "gif", "WebP", "pdf", "IPYNB"] {
        let path = format!("asset.{extension}");
        fs::write(temp.path().join(&path), []).unwrap();
        assert!(
            call(
                temp.path(),
                HookKind::PreToolUse,
                event("Read", temp.path(), json!({"file_path":path}))
            )
            .is_none()
        );
        rejected(
            call(
                temp.path(),
                HookKind::PreToolUse,
                event("Write", temp.path(), json!({"file_path":path})),
            ),
            "write_file",
        );
    }
    fs::create_dir(temp.path().join("directory.pdf")).unwrap();
    rejected(
        call(
            temp.path(),
            HookKind::PreToolUse,
            event("Read", temp.path(), json!({"file_path":"directory.pdf"})),
        ),
        "read_file",
    );
    for path in ["unknown.bin", "file.svg", "README"] {
        rejected(
            call(
                temp.path(),
                HookKind::PreToolUse,
                event("Read", temp.path(), json!({"file_path":path})),
            ),
            "read_file",
        );
    }
}

#[test]
fn respects_outside_roots_and_resolves_relative_paths_from_event_cwd() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    let outside = temp.path().join("project-other");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    for tool in ["Read", "Write", "Edit", "Grep"] {
        let path = outside.join("new.txt");
        assert!(
            call(
                &root,
                HookKind::PreToolUse,
                event(tool, &root, json!({"path":path,"file_path":path}))
            )
            .is_none()
        );
    }
    rejected(
        call(
            &root,
            HookKind::PreToolUse,
            event("Read", temp.path(), json!({"file_path":"project/file.txt"})),
        ),
        "read_file",
    );
    assert!(
        call(
            &root,
            HookKind::PreToolUse,
            event(
                "Read",
                &root,
                json!({"file_path":"../project-other/file.txt"})
            )
        )
        .is_none()
    );
    rejected(
        call(
            &root,
            HookKind::PreToolUse,
            event("Grep", &root, json!({"pattern":"中文"})),
        ),
        "grep",
    );
    assert!(
        call(
            &root,
            HookKind::PreToolUse,
            event("Grep", &outside, json!({"pattern":"中文"}))
        )
        .is_none()
    );
    fs::create_dir(root.join("nested")).unwrap();
    rejected(
        call(
            &root,
            HookKind::PreToolUse,
            event("Grep", &root.join("nested"), json!({"path":".."})),
        ),
        "grep",
    );
}

#[test]
fn never_intercepts_mcp_glob_shell_or_notebook_tools() {
    let temp = tempfile::tempdir().unwrap();
    for name in [
        "Glob",
        "Bash",
        "PowerShell",
        "NotebookEdit",
        "mcp__gbk__read_file",
        "mcp__plugin_gbk-mcp_gbk__read_file",
        "OtherRead",
    ] {
        assert!(
            call(
                temp.path(),
                HookKind::PreToolUse,
                event(name, temp.path(), json!({}))
            )
            .is_none()
        );
    }
}

#[test]
fn context_covers_each_event_without_approving_tools() {
    let temp = tempfile::tempdir().unwrap();
    for kind in [HookKind::SessionStart, HookKind::SubagentStart] {
        let name = if kind == HookKind::SessionStart {
            "SessionStart"
        } else {
            "SubagentStart"
        };
        for source in ["startup", "resume", "clear", "compact", "fork"] {
            let response = call(
                temp.path(),
                kind,
                json!({"hook_event_name":name,"source":source,"agent_type":"Explore"}),
            )
            .unwrap();
            let output = &response["hookSpecificOutput"];
            assert_eq!(output["hookEventName"], name);
            let context = output["additionalContext"].as_str().unwrap();
            for rule in [
                "read_file",
                "expected_revision",
                "complete=false",
                "parent",
                "Shell",
            ] {
                assert!(context.contains(rule));
            }
            assert!(output.get("permissionDecision").is_none());
        }
    }
}

#[test]
fn malformed_paths_events_and_missing_roots_fail_closed() {
    let temp = tempfile::tempdir().unwrap();
    for value in [
        json!({}),
        json!({"hook_event_name":"SessionStart"}),
        event("Read", temp.path(), json!({})),
        event("Read", temp.path(), json!({"file_path":""})),
        event("Read", temp.path(), json!({"file_path":"x\0y"})),
        json!({"hook_event_name":"PreToolUse","tool_name":"Read","tool_input":{"file_path":"relative"}}),
    ] {
        rejected(
            call(temp.path(), HookKind::PreToolUse, value),
            "could not verify",
        );
    }
    for bytes in [b"".as_slice(), b"not JSON", b"{} trailing"] {
        let mut output = Vec::new();
        hooks::run(temp.path(), HookKind::PreToolUse, bytes, &mut output).unwrap();
        rejected(
            Some(serde_json::from_slice(&output).unwrap()),
            "could not verify",
        );
    }
    rejected(
        call(
            &temp.path().join("missing"),
            HookKind::PreToolUse,
            event("Read", temp.path(), json!({"file_path":"x"})),
        ),
        "could not verify",
    );
    let oversized = std::io::repeat(b' ').take(hooks::INPUT_BYTES + 1);
    let mut output = Vec::new();
    hooks::run(temp.path(), HookKind::PreToolUse, oversized, &mut output).unwrap();
    rejected(Some(serde_json::from_slice(&output).unwrap()), "64 MiB");
}

#[cfg(unix)]
#[test]
fn links_inside_or_pointing_into_workspace_are_not_a_bypass() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    let outside = temp.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    symlink(&outside, root.join("escape")).unwrap();
    symlink(&root, outside.join("enter")).unwrap();
    symlink(root.join("missing"), root.join("broken.pdf")).unwrap();
    for path in [
        root.join("escape/new.txt"),
        root.join("escape/../new.txt"),
        outside.join("enter/new.txt"),
        root.join("broken.pdf"),
    ] {
        rejected(
            call(
                &root,
                HookKind::PreToolUse,
                event("Read", &root, json!({"file_path":path})),
            ),
            "could not verify",
        );
    }
}

#[cfg(windows)]
#[test]
fn windows_case_unicode_spaces_long_paths_and_junctions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Project 中文 空格");
    fs::create_dir(&root).unwrap();
    let path = root.join(format!(
        "{}/{}/{}/中文.txt",
        "a".repeat(100),
        "b".repeat(100),
        "c".repeat(100)
    ));
    rejected(
        call(
            &root,
            HookKind::PreToolUse,
            event("Write", &root, json!({"file_path":path})),
        ),
        "write_file",
    );
    let mixed_case = root.join("file.txt").to_string_lossy().to_uppercase();
    rejected(
        call(
            &root,
            HookKind::PreToolUse,
            event("Read", &root, json!({"file_path":mixed_case})),
        ),
        "read_file",
    );
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let junction = root.join("junction");
    let result = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&outside)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    rejected(
        call(
            &root,
            HookKind::PreToolUse,
            event("Read", &root, json!({"file_path":junction.join("x")})),
        ),
        "could not verify",
    );
    fs::remove_dir(junction).unwrap();
}
