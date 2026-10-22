use gbk_mcp::{control::Control, encoding, server::GbkServer, types::*, workspace::Workspace};
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Cursor, Read},
    time::{Duration, Instant},
};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

fn control() -> Control {
    Control::new(CancellationToken::new(), 30000)
}
fn fixture() -> (TempDir, GbkServer) {
    let temp = tempfile::tempdir().unwrap();
    let server = GbkServer::new(Workspace::new(temp.path()).unwrap(), 4);
    (temp, server)
}
async fn run(server: &GbkServer, tool: &str, args: Value) -> Value {
    server
        .execute(tool, args, CancellationToken::new())
        .await
        .unwrap()
}
fn gbk(text: &str) -> Vec<u8> {
    encoding::encode(text, Encoding::Gbk).unwrap()
}

#[test]
fn exact_tool_surface_and_annotations() {
    let tools = GbkServer::tools();
    assert_eq!(
        tools.iter().map(|t| t.name.as_ref()).collect::<Vec<_>>(),
        ["read_file", "write_file", "edit_file", "glob", "grep"]
    );
    for tool in tools {
        assert_eq!(
            tool.annotations.unwrap().read_only_hint,
            Some(matches!(tool.name.as_ref(), "read_file" | "glob" | "grep"))
        );
        assert_eq!(
            tool.input_schema.get("additionalProperties"),
            Some(&json!(false))
        );
    }
}

#[test]
fn strict_detection_cases() {
    for (bytes, choice, encoding, status, bom) in [
        (
            gbk("中文\r\n"),
            EncodingChoice::Auto,
            Encoding::Gbk,
            EncodingStatus::Detected,
            false,
        ),
        (
            "中\n".as_bytes().to_vec(),
            EncodingChoice::Auto,
            Encoding::Utf8,
            EncodingStatus::Detected,
            false,
        ),
        (
            b"ascii\n".to_vec(),
            EncodingChoice::Auto,
            Encoding::Utf8,
            EncodingStatus::Ascii,
            false,
        ),
        (
            vec![0xc2, 0xa3],
            EncodingChoice::Auto,
            Encoding::Gbk,
            EncodingStatus::Ambiguous,
            false,
        ),
        (
            vec![0xc2, 0xa3],
            EncodingChoice::Utf8,
            Encoding::Utf8,
            EncodingStatus::Explicit,
            false,
        ),
        (
            [b"\xef\xbb\xbf".as_slice(), "中".as_bytes()].concat(),
            EncodingChoice::Auto,
            Encoding::Utf8,
            EncodingStatus::Detected,
            true,
        ),
    ] {
        let (info, revision) =
            encoding::inspect(&mut Cursor::new(&bytes), choice, &control()).unwrap();
        assert_eq!(
            (info.encoding, info.status, info.bom),
            (encoding, status, bom)
        );
        assert_eq!(revision, blake3::hash(&bytes).to_hex().to_string());
    }
    for bytes in [
        vec![0x81],
        vec![0x81, 0x30, 0x81, 0x30],
        vec![0xff],
        vec![b'a', 0, b'b'],
    ] {
        assert!(
            encoding::inspect(&mut Cursor::new(bytes), EncodingChoice::Auto, &control()).is_err()
        );
    }
    assert!(encoding::encode("emoji 😀", Encoding::Gbk).is_err());
    assert_eq!(encoding::encode("€", Encoding::Gbk).unwrap(), vec![0x80]);
}

struct Chunks {
    cursor: Cursor<Vec<u8>>,
    chunk: usize,
}
impl Read for Chunks {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = buf.len().min(self.chunk);
        self.cursor.read(&mut buf[..n])
    }
}

#[test]
fn streaming_decoder_preserves_split_characters_and_rejects_invalid_tails() {
    for codec in [Encoding::Gbk, Encoding::Utf8] {
        let expected = "中文\r\nA€中\n".repeat(4000);
        let bytes = encoding::encode(&expected, codec).unwrap();
        for chunk in [1, 2, 3, 7, 8192] {
            let info = EncodingInfo {
                encoding: codec,
                status: EncodingStatus::Explicit,
                bom: false,
                candidates: vec![codec],
            };
            let mut reader = encoding::StrictReader::new(
                Chunks {
                    cursor: Cursor::new(bytes.clone()),
                    chunk,
                },
                &info,
                control(),
            );
            let mut actual = String::new();
            reader.read_to_string(&mut actual).unwrap();
            assert_eq!(actual, expected);
        }
    }
    let info = EncodingInfo {
        encoding: Encoding::Gbk,
        status: EncodingStatus::Explicit,
        bom: false,
        candidates: vec![Encoding::Gbk],
    };
    for bytes in [vec![0x81], vec![0x81, 0x30, 0x81, 0x30]] {
        let mut reader = encoding::StrictReader::new(Cursor::new(bytes), &info, control());
        assert!(reader.read_to_end(&mut Vec::new()).is_err());
    }
}

#[tokio::test]
async fn gbk_edit_preserves_untouched_bytes_crlf_and_no_final_newline() {
    let (temp, server) = fixture();
    let before = gbk("前缀€\r\n原文\r\n尾部");
    fs::write(temp.path().join("中文.c"), &before).unwrap();
    let read = run(&server, "read_file", json!({"path":"中文.c"})).await;
    assert_eq!(read["content"], "前缀€\r\n原文\r\n尾部");
    let args = json!({"path":"中文.c", "expected_revision":read["revision"], "edits":[{"old_string":"原文", "new_string":"修改"}], "dry_run":true});
    let dry = run(&server, "edit_file", args.clone()).await;
    assert_eq!(dry["applied"], false);
    assert!(dry["diff"].as_str().unwrap().contains("修改"));
    assert_eq!(fs::read(temp.path().join("中文.c")).unwrap(), before);
    let mut args = args;
    args["dry_run"] = json!(false);
    let edited = run(&server, "edit_file", args).await;
    assert_eq!(
        fs::read(temp.path().join("中文.c")).unwrap(),
        gbk("前缀€\r\n修改\r\n尾部")
    );
    assert_eq!(
        edited["revision"],
        run(&server, "read_file", json!({"path":"中文.c"})).await["revision"]
    );
}

#[tokio::test]
async fn create_defaults_utf8_and_explicit_gbk_and_dry_run_has_no_file() {
    let (temp, server) = fixture();
    run(
        &server,
        "write_file",
        json!({"path":"new.txt","content":"中文"}),
    )
    .await;
    assert_eq!(
        fs::read(temp.path().join("new.txt")).unwrap(),
        "中文".as_bytes()
    );
    run(
        &server,
        "write_file",
        json!({"path":"old.txt","content":"中文","encoding":"gbk"}),
    )
    .await;
    assert_eq!(fs::read(temp.path().join("old.txt")).unwrap(), gbk("中文"));
    run(
        &server,
        "write_file",
        json!({"path":"preview.txt","content":"中","dry_run":true}),
    )
    .await;
    assert!(!temp.path().join("preview.txt").exists());
    let e = server
        .execute(
            "write_file",
            json!({"path":"old.txt","content":"replacement"}),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, "revision_required");
}

#[tokio::test]
async fn nested_creation_and_long_paths_do_not_need_extra_tools() {
    let (temp, server) = fixture();
    let path = format!(
        "{}/{}/{}/{}/中文.c",
        "a".repeat(75),
        "b".repeat(75),
        "c".repeat(75),
        "d".repeat(75)
    );
    run(
        &server,
        "write_file",
        json!({"path":path,"content":"中文","encoding":"gbk","dry_run":true}),
    )
    .await;
    assert!(!temp.path().join("a".repeat(75)).exists());
    run(
        &server,
        "write_file",
        json!({"path":path,"content":"中文","encoding":"gbk"}),
    )
    .await;
    let read = run(&server, "read_file", json!({"path":path})).await;
    assert_eq!(read["content"], "中文");
}

#[tokio::test]
async fn gbk_alias_bytes_outside_edit_are_not_canonicalized() {
    let (temp, server) = fixture();
    let bytes = [b"prefix:".as_slice(), &[0xa3, 0xa0], &gbk("中文"), b":tail"].concat();
    fs::write(temp.path().join("x"), &bytes).unwrap();
    let read = run(&server, "read_file", json!({"path":"x","encoding":"gbk"})).await;
    run(&server,"edit_file",json!({"path":"x","encoding":"gbk","expected_revision":read["revision"],"edits":[{"old_string":"中文","new_string":"修改"}]})).await;
    assert_eq!(
        fs::read(temp.path().join("x")).unwrap(),
        [b"prefix:".as_slice(), &[0xa3, 0xa0], &gbk("修改"), b":tail"].concat()
    );
}

#[tokio::test]
async fn explicit_gbk_overrides_bom_looking_bytes_and_can_commit() {
    let (temp, server) = fixture();
    let prefix = [0xef, 0xbb, 0xbf, b'A'];
    fs::write(
        temp.path().join("x"),
        [prefix.as_slice(), &gbk("中文")].concat(),
    )
    .unwrap();
    let read = run(&server, "read_file", json!({"path":"x","encoding":"gbk"})).await;
    assert_eq!(read["encoding"]["bom"], false);
    run(&server,"edit_file",json!({"path":"x","encoding":"gbk","expected_revision":read["revision"],"edits":[{"old_string":"中文","new_string":"修改"}]})).await;
    assert_eq!(
        fs::read(temp.path().join("x")).unwrap(),
        [prefix.as_slice(), &gbk("修改")].concat()
    );
}

#[cfg(windows)]
#[tokio::test]
async fn occupied_windows_target_fails_without_deleting_original() {
    use std::os::windows::fs::OpenOptionsExt;
    let (temp, server) = fixture();
    let path = temp.path().join("locked.c");
    fs::write(&path, gbk("中文")).unwrap();
    let read = run(&server, "read_file", json!({"path":"locked.c"})).await;
    let _locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .unwrap();
    assert!(
        server
            .execute(
                "write_file",
                json!({"path":"locked.c","content":"修改","expected_revision":read["revision"]}),
                CancellationToken::new()
            )
            .await
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), gbk("中文"));
}

#[tokio::test]
async fn ambiguity_and_ascii_require_explicit_encoding_for_non_ascii_writes() {
    let (temp, server) = fixture();
    fs::write(temp.path().join("amb.txt"), [0xc2, 0xa3]).unwrap();
    let read = run(&server, "read_file", json!({"path":"amb.txt"})).await;
    assert_eq!(read["encoding"]["status"], "ambiguous");
    assert_eq!(read["content"], "拢");
    let args = json!({"path":"amb.txt","expected_revision":read["revision"],"edits":[{"old_string":"拢","new_string":"中"}]});
    assert_eq!(
        server
            .execute("edit_file", args.clone(), CancellationToken::new())
            .await
            .unwrap_err()
            .code,
        "encoding_required"
    );
    let mut args = args;
    args["encoding"] = json!("gbk");
    run(&server, "edit_file", args).await;
    assert_eq!(fs::read(temp.path().join("amb.txt")).unwrap(), gbk("中"));
    fs::write(temp.path().join("ascii"), b"abc").unwrap();
    let read = run(&server, "read_file", json!({"path":"ascii"})).await;
    let args = json!({"path":"ascii","expected_revision":read["revision"],"content":"中"});
    assert_eq!(
        server
            .execute("write_file", args, CancellationToken::new())
            .await
            .unwrap_err()
            .code,
        "encoding_required"
    );
}

#[tokio::test]
async fn conflict_unmappable_duplicate_and_overlap_leave_file_unchanged() {
    let (temp, server) = fixture();
    let bytes = gbk("中文 中文");
    fs::write(temp.path().join("x"), &bytes).unwrap();
    let read = run(&server, "read_file", json!({"path":"x"})).await;
    for (edits, code) in [
        (
            json!([{"old_string":"中文","new_string":"新"}]),
            "ambiguous_match",
        ),
        (
            json!([{"old_string":"中文 中文","new_string":"😀"}]),
            "unmappable_character",
        ),
        (
            json!([{"old_string":"中文 中文","new_string":"新"},{"old_string":"中文","new_string":"新","replace_all":true}]),
            "overlapping_edits",
        ),
    ] {
        let e = server
            .execute(
                "edit_file",
                json!({"path":"x","expected_revision":read["revision"],"edits":edits}),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(e.code, code);
        assert_eq!(fs::read(temp.path().join("x")).unwrap(), bytes);
    }
    fs::write(temp.path().join("x"), gbk("外部修改")).unwrap();
    assert_eq!(
        server
            .execute(
                "write_file",
                json!({"path":"x","content":"中文","expected_revision":read["revision"]}),
                CancellationToken::new()
            )
            .await
            .unwrap_err()
            .code,
        "conflict"
    );
}

#[tokio::test]
async fn bom_and_multiple_original_version_edits() {
    let (temp, server) = fixture();
    fs::write(temp.path().join("x"), b"\xef\xbb\xbfabc\r\ndef").unwrap();
    let read = run(&server, "read_file", json!({"path":"x"})).await;
    run(&server,"edit_file",json!({"path":"x","expected_revision":read["revision"],"edits":[{"old_string":"abc","new_string":"中文"},{"old_string":"def","new_string":"末尾"}]})).await;
    assert_eq!(
        fs::read(temp.path().join("x")).unwrap(),
        [b"\xef\xbb\xbf".as_slice(), "中文\r\n末尾".as_bytes()].concat()
    );
}

#[tokio::test]
async fn grep_mixed_encodings_modes_context_and_file_globs() {
    let (temp, server) = fixture();
    fs::create_dir(temp.path().join("nested")).unwrap();
    fs::write(
        temp.path().join("nested/a.c"),
        gbk("before\n中文匹配\nafter\n"),
    )
    .unwrap();
    fs::write(temp.path().join("b.c"), "中\n中文匹配\n").unwrap();
    fs::write(temp.path().join("other.txt"), gbk("中文匹配")).unwrap();
    let result = run(&server, "grep", json!({"pattern":"中文匹配","glob":"*.c"})).await;
    assert_eq!(result["complete"], true);
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
    for item in result["items"].as_array().unwrap() {
        assert_eq!(item["line"], 2);
        assert!(item["text"].as_str().unwrap().contains("中文匹配"));
    }
    let files = run(
        &server,
        "grep",
        json!({"pattern":"中文","output_mode":"files","glob":"*.c"}),
    )
    .await;
    assert_eq!(files["items"].as_array().unwrap().len(), 2);
    let count = run(
        &server,
        "grep",
        json!({"pattern":"中文","output_mode":"count","path":"nested/a.c"}),
    )
    .await;
    assert_eq!(count["items"][0]["matching_lines"], 1);
    let context = run(
        &server,
        "grep",
        json!({"pattern":"匹配","context":1,"path":"nested/a.c"}),
    )
    .await;
    assert_eq!(context["items"].as_array().unwrap().len(), 3);
    fs::write(
        temp.path().join("windows.c"),
        gbk("before\r\n中文匹配\r\nafter\r\n"),
    )
    .unwrap();
    let anchored = run(
        &server,
        "grep",
        json!({"pattern":"^中文匹配$","path":"windows.c"}),
    )
    .await;
    assert_eq!(anchored["complete"], true);
    assert_eq!(anchored["items"].as_array().unwrap().len(), 1);
    assert_eq!(anchored["items"][0]["line"], 2);
}

#[tokio::test]
async fn glob_and_grep_honor_ignore_hidden_and_filters() {
    let (temp, server) = fixture();
    fs::create_dir(temp.path().join("skip")).unwrap();
    for path in ["a.c", "skip/b.c", ".hidden.c"] {
        fs::write(temp.path().join(path), "needle").unwrap();
    }
    fs::write(temp.path().join(".gitignore"), "skip/\n").unwrap();
    let result = run(&server, "glob", json!({"pattern":"**/*.c"})).await;
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    let result = run(
        &server,
        "glob",
        json!({"pattern":"**/*.c","include_hidden":true,"respect_ignore":false}),
    )
    .await;
    assert_eq!(result["items"].as_array().unwrap().len(), 3);
    let result = run(&server, "grep", json!({"pattern":"needle","glob":"*.c"})).await;
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn limits_invalid_files_and_timeouts_never_claim_complete() {
    let (temp, server) = fixture();
    for i in 0..50 {
        fs::write(temp.path().join(format!("{i}.txt")), "needle\n").unwrap();
    }
    let result = run(&server, "grep", json!({"pattern":"needle","max_results":1})).await;
    assert_eq!(result["complete"], false);
    assert_eq!(result["stop_reason"], "result_limit");
    fs::write(temp.path().join("bad"), [0x81]).unwrap();
    let result = run(
        &server,
        "grep",
        json!({"pattern":"not-present","path":"bad"}),
    )
    .await;
    assert_eq!(result["complete"], false);
    assert!(result["items"].as_array().unwrap().is_empty());
    fs::write(temp.path().join("huge"), vec![b'x'; 8 * 1024 * 1024]).unwrap();
    let result = run(
        &server,
        "grep",
        json!({"pattern":"none","path":"huge","timeout_ms":1}),
    )
    .await;
    assert_eq!(result["complete"], false);
    assert_eq!(result["stop_reason"], "timeout");
}

#[tokio::test]
async fn output_size_and_read_pagination_are_bounded() {
    let (temp, server) = fixture();
    fs::write(temp.path().join("x"), "a\n".repeat(1000)).unwrap();
    let result = run(
        &server,
        "read_file",
        json!({"path":"x","offset":10,"limit":3}),
    )
    .await;
    assert_eq!(result["content"], "a\na\na\n");
    assert_eq!(result["next_offset"], 13);
    fs::write(temp.path().join("x"), "x\t".repeat(40000)).unwrap();
    let result = run(&server, "read_file", json!({"path":"x"})).await;
    assert!(result.to_string().len() < OUTPUT_BYTES);
    assert_eq!(result["line_truncated"], true);
    let result = run(&server, "grep", json!({"path":"x","pattern":"x"})).await;
    assert!(result.to_string().len() < OUTPUT_BYTES);
    assert_eq!(result["items"][0]["line_truncated"], true);
    fs::write(
        temp.path().join("late"),
        format!("{}needle_tail\n", "x".repeat(10000)),
    )
    .unwrap();
    let result = run(
        &server,
        "grep",
        json!({"path":"late","pattern":"needle_tail"}),
    )
    .await;
    assert!(
        result["items"][0]["text"]
            .as_str()
            .unwrap()
            .contains("needle_tail")
    );
    assert!(result["items"][0]["text_start_utf8_byte"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn cancelled_scan_releases_workers_promptly() {
    let (temp, server) = fixture();
    fs::write(
        temp.path().join("big"),
        b"ascii text\n".repeat(4 * 1024 * 1024),
    )
    .unwrap();
    let token = CancellationToken::new();
    let trigger = token.clone();
    let pending = server.execute("grep", json!({"path":"big","pattern":"absent"}), token);
    let cancel = async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        trigger.cancel();
    };
    let start = Instant::now();
    let (result, _) = tokio::join!(pending, cancel);
    assert!(start.elapsed() < Duration::from_millis(500));
    if let Ok(result) = result {
        assert_eq!(result["complete"], false);
        assert_eq!(result["stop_reason"], "cancelled");
    } else {
        assert_eq!(result.unwrap_err().code, "cancelled");
    }
    let read = run(&server, "glob", json!({"pattern":"*"})).await;
    assert_eq!(read["complete"], true);
}

#[tokio::test]
async fn paths_readonly_and_symlinks_are_rejected() {
    let (temp, server) = fixture();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("x"), "outside").unwrap();
    assert_eq!(
        server
            .execute(
                "read_file",
                json!({"path":outside.path().join("x")}),
                CancellationToken::new()
            )
            .await
            .unwrap_err()
            .code,
        "outside_root"
    );
    fs::write(temp.path().join("x"), "text").unwrap();
    let read = run(&server, "read_file", json!({"path":"x"})).await;
    let mut perms = fs::metadata(temp.path().join("x")).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(temp.path().join("x"), perms).unwrap();
    assert_eq!(
        server
            .execute(
                "write_file",
                json!({"path":"x","content":"new","expected_revision":read["revision"]}),
                CancellationToken::new()
            )
            .await
            .unwrap_err()
            .code,
        "permission_denied"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};
        fs::set_permissions(temp.path().join("x"), fs::Permissions::from_mode(0o600)).unwrap();
        symlink(temp.path().join("x"), temp.path().join("link")).unwrap();
        assert_eq!(
            server
                .execute(
                    "read_file",
                    json!({"path":"link"}),
                    CancellationToken::new()
                )
                .await
                .unwrap_err()
                .code,
            "symlink_not_allowed"
        );
        fs::hard_link(temp.path().join("x"), temp.path().join("hard")).unwrap();
        assert_eq!(server.execute("edit_file",json!({"path":"x","expected_revision":read["revision"],"edits":[{"old_string":"text","new_string":"changed"}]}),CancellationToken::new()).await.unwrap_err().code,"hard_link_not_allowed");
    }
}
