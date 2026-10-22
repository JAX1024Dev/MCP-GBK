use crate::{
    control::Control,
    encoding,
    error::{AppError, Result},
    types::*,
    workspace::Workspace,
};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Cursor, Read, Write},
    path::Path,
    time::Duration,
};
use tempfile::NamedTempFile;

fn load(
    path: &Path,
    choice: EncodingChoice,
    ctl: &Control,
) -> Result<(Vec<u8>, String, EncodingInfo, String)> {
    let (file, metadata) = crate::workspace::open_file(path)?;
    if metadata.len() > MAX_FILE_BYTES {
        return Err(AppError::new(
            "file_too_large",
            "Editing is limited to 32 MiB; read_file and grep support streaming reads",
        ));
    }
    let mut reader = file.take(MAX_FILE_BYTES + 1);
    let mut bytes = Vec::new();
    let mut block = [0; 65536];
    loop {
        ctl.check()?;
        let n = reader.read(&mut block)?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&block[..n]);
    }
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(AppError::new(
            "file_too_large",
            "File grew beyond the 32 MiB edit limit",
        ));
    }
    let (info, revision) = encoding::inspect(&mut Cursor::new(&bytes), choice, ctl)?;
    let text = encoding::decode(&bytes, &info)?;
    Ok((bytes, text, info, revision))
}

pub fn read_file(ws: &Workspace, req: ReadRequest, ctl: &Control) -> Result<Value> {
    let offset = req.offset.unwrap_or(1);
    let limit = req.limit.unwrap_or(200);
    if offset == 0 || limit == 0 || limit > 10000 {
        return Err(AppError::new(
            "invalid_argument",
            "offset must be >=1 and limit must be 1..10000",
        ));
    }
    let path = ws.resolve(&req.path, false)?;
    let (mut file, before) = crate::workspace::open_file(&path)?;
    let (info, revision) = encoding::inspect(&mut file, req.encoding, ctl)?;
    let mut reader =
        BufReader::with_capacity(65536, encoding::StrictReader::new(file, &info, ctl.clone()));
    let mut content = String::new();
    let mut line = Vec::new();
    let mut number = 1;
    let mut returned = 0;
    let mut truncated = false;
    let mut line_truncated = false;
    let mut eof = false;
    let mut budget = OUTPUT_BYTES - 8192;
    loop {
        ctl.check()?;
        if returned == limit {
            truncated = !reader.fill_buf()?.is_empty();
            eof = !truncated;
            break;
        }
        line.clear();
        let n = (&mut reader)
            .take((LINE_BYTES + 1) as u64)
            .read_until(b'\n', &mut line)?;
        if n == 0 {
            eof = true;
            break;
        }
        if n > LINE_BYTES {
            return Err(AppError::new(
                "line_too_long",
                "Decoded line exceeds 1 MiB; use grep with a narrower pattern",
            ));
        }
        if number >= offset {
            let text = std::str::from_utf8(&line)
                .map_err(|e| AppError::new("invalid_encoding", e.to_string()))?;
            let cost = serde_json::to_string(text)
                .expect("string serialization")
                .len();
            if cost > budget {
                if returned == 0 {
                    let mut end = (budget / 6).min(text.len());
                    while !text.is_char_boundary(end) {
                        end -= 1;
                    }
                    content.push_str(&text[..end]);
                    returned += 1;
                    number += 1;
                    line_truncated = true;
                }
                truncated = true;
                break;
            }
            budget -= cost;
            content.push_str(text);
            returned += 1;
        }
        number += 1;
    }
    // Re-opening catches atomic external replacements, while metadata checks catch
    // common in-place modifications during a multi-pass read.
    let current = fs::metadata(&path)?;
    if current.len() != before.len() || current.modified().ok() != before.modified().ok() {
        return Err(AppError::new(
            "conflict",
            "File changed while reading; retry",
        ));
    }
    Ok(
        json!({"path": ws.relative(&path), "encoding": info, "revision": revision,
        "content": content, "start_line": offset, "returned_lines": returned,
        "truncated": truncated, "line_truncated": line_truncated,
        "next_offset": if truncated { Some(number) } else { None }, "eof": eof,
        "notice": if line_truncated { Some("The last returned line is incomplete; grep can retrieve matching excerpts.") } else { None }}),
    )
}

fn verify_revision(actual: &str, expected: Option<&str>) -> Result<()> {
    match expected {
        None => Err(AppError::new(
            "revision_required",
            "Read the file and provide expected_revision before overwriting it",
        )),
        Some(value) if value != actual => Err(AppError::new(
            "conflict",
            "File version changed; read it again before editing",
        )),
        _ => Ok(()),
    }
}

fn check_writable(path: &Path) -> Result<()> {
    let meta = fs::metadata(path)?;
    if meta.permissions().readonly() {
        return Err(AppError::new(
            "permission_denied",
            "Read-only files are not replaced",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() > 1 {
            return Err(AppError::new(
                "hard_link_not_allowed",
                "Atomic replacement of multiply-linked files is refused",
            ));
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
        };
        let file = File::open(path)?;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        if info.nNumberOfLinks > 1 {
            return Err(AppError::new(
                "hard_link_not_allowed",
                "Atomic replacement of multiply-linked files is refused",
            ));
        }
    }
    Ok(())
}

fn commit(
    ws: &Workspace,
    path: &Path,
    bytes: &[u8],
    expected: Option<&str>,
    ctl: &Control,
) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::new("invalid_path", "Missing parent directory"))?;
    ctl.check()?;
    if expected.is_none() {
        ws.resolve(
            path.to_str()
                .ok_or_else(|| AppError::new("invalid_path", "Path is not Unicode"))?,
            true,
        )?;
        fs::create_dir_all(parent)?;
    }
    let mut tmp = tempfile::Builder::new()
        .prefix(".gbk-mcp-")
        .tempfile_in(parent)?;
    for chunk in bytes.chunks(65536) {
        ctl.check()?;
        tmp.write_all(chunk)?;
    }
    if expected.is_some() {
        check_writable(path)?;
        tmp.as_file()
            .set_permissions(fs::metadata(path)?.permissions())?;
    }
    tmp.as_file().sync_all()?;
    // Re-resolve immediately before replacing to catch moved parents and links.
    if ws.resolve(
        path.to_str()
            .ok_or_else(|| AppError::new("invalid_path", "Path is not Unicode"))?,
        expected.is_none(),
    )? != path
    {
        return Err(AppError::new("conflict", "File path changed before commit"));
    }
    if let Some(expected) = expected {
        let (mut current, _) = crate::workspace::open_file(path)?;
        let mut hash = blake3::Hasher::new();
        let mut block = [0; 65536];
        loop {
            ctl.check()?;
            let n = current.read(&mut block)?;
            if n == 0 {
                break;
            }
            hash.update(&block[..n]);
        }
        let actual = hash.finalize().to_hex().to_string();
        verify_revision(&actual, Some(expected))?;
        check_writable(path)?;
    }
    ctl.check()?;
    if expected.is_some() {
        replace_existing(tmp, path)?;
    } else {
        tmp.persist_noclobber(path)
            .map_err(|e| AppError::new("create_conflict", e.error.to_string()))?;
    }
    #[cfg(unix)]
    {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_existing(tmp: NamedTempFile, path: &Path) -> Result<()> {
    tmp.persist(path).map_err(|e| AppError::from(e.error))?;
    Ok(())
}

#[cfg(windows)]
fn replace_existing(tmp: NamedTempFile, path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;
    // ReplaceFileW opens the replacement without sharing access. Close our
    // writing handle first, retaining automatic path cleanup on failure.
    let tmp = tmp.into_temp_path();
    let target: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let source: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
    let ok = unsafe {
        ReplaceFileW(
            target.as_ptr(),
            source.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

fn allow_edit(info: &EncodingInfo, adds_non_ascii: bool) -> Result<()> {
    if info.status == EncodingStatus::Ambiguous
        || (info.status == EncodingStatus::Ascii && adds_non_ascii)
    {
        return Err(AppError::new(
            "encoding_required",
            "Specify encoding=utf8 or gbk explicitly before modifying ambiguous text or adding non-ASCII text to an ASCII file",
        ));
    }
    Ok(())
}

fn preview(old: &str, new: &str) -> String {
    let diff = similar::TextDiff::configure()
        .timeout(Duration::from_millis(250))
        .diff_lines(old, new);
    let mut out = String::new();
    for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
        use std::fmt::Write as _;
        // Hunk display may be large, so cap each formatted piece below.
        let text = hunk.to_string();
        let remaining = 8192usize.saturating_sub(out.len());
        if text.len() > remaining {
            let mut end = remaining;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            out.push_str(&text[..end]);
            out.push_str("\n[diff truncated]\n");
            break;
        }
        let _ = write!(&mut out, "{text}");
    }
    out
}

pub fn write_file(ws: &Workspace, req: WriteRequest, ctl: &Control) -> Result<Value> {
    let _lock = ws
        .writes
        .lock()
        .map_err(|_| AppError::new("internal", "Write lock poisoned"))?;
    if req.content.len() as u64 > MAX_FILE_BYTES {
        return Err(AppError::new(
            "file_too_large",
            "Input content exceeds 32 MiB",
        ));
    }
    let path = ws.resolve(&req.path, true)?;
    let exists = path.try_exists()?;
    let (old, info, before) = if exists {
        check_writable(&path)?;
        let (_, old, info, actual) = load(&path, req.encoding, ctl)?;
        verify_revision(&actual, req.expected_revision.as_deref())?;
        allow_edit(&info, !req.content.is_ascii())?;
        (old, info, Some(actual))
    } else {
        if req.expected_revision.is_some() {
            return Err(AppError::new("conflict", "Expected file no longer exists"));
        }
        let encoding = if req.encoding == EncodingChoice::Gbk {
            Encoding::Gbk
        } else {
            Encoding::Utf8
        };
        (
            String::new(),
            EncodingInfo {
                encoding,
                status: EncodingStatus::Explicit,
                bom: false,
                candidates: vec![encoding],
            },
            None,
        )
    };
    let mut bytes = if info.bom {
        vec![0xef, 0xbb, 0xbf]
    } else {
        Vec::new()
    };
    bytes.extend(encoding::encode(&req.content, info.encoding)?);
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(AppError::new(
            "file_too_large",
            "Encoded output exceeds 32 MiB",
        ));
    }
    let diff = preview(&old, &req.content);
    let revision = blake3::hash(&bytes).to_hex().to_string();
    if !req.dry_run {
        commit(ws, &path, &bytes, before.as_deref(), ctl)?;
    }
    Ok(
        json!({"path": ws.relative(&path), "encoding": info, "created": !exists, "dry_run": req.dry_run,
        "applied": !req.dry_run, "previous_revision": before, "revision": revision, "diff": diff}),
    )
}

pub fn edit_file(ws: &Workspace, req: EditRequest, ctl: &Control) -> Result<Value> {
    let _lock = ws
        .writes
        .lock()
        .map_err(|_| AppError::new("internal", "Write lock poisoned"))?;
    let path = ws.resolve(&req.path, false)?;
    check_writable(&path)?;
    let (bytes, old, info, revision) = load(&path, req.encoding, ctl)?;
    verify_revision(&revision, Some(&req.expected_revision))?;
    if req.edits.is_empty() || req.edits.len() > 1000 {
        return Err(AppError::new("invalid_argument", "Provide 1..1000 edits"));
    }
    allow_edit(&info, req.edits.iter().any(|e| !e.new_string.is_ascii()))?;
    let mut spans = Vec::new();
    let mut size = bytes.len();
    for edit in &req.edits {
        ctl.check()?;
        if edit.old_string.is_empty() {
            return Err(AppError::new(
                "invalid_argument",
                "old_string cannot be empty",
            ));
        }
        let matches: Vec<_> = old
            .match_indices(&edit.old_string)
            .take(if edit.replace_all { 10001 } else { 2 })
            .collect();
        if matches.is_empty() {
            return Err(AppError::new(
                "no_match",
                "old_string was not found exactly; preserve CRLF/LF and whitespace",
            ));
        }
        if (!edit.replace_all && matches.len() != 1) || matches.len() > 10000 {
            return Err(AppError::new(
                "ambiguous_match",
                "Match is not unique or exceeds 10000 occurrences; provide more context",
            ));
        }
        let replacement = encoding::encode(&edit.new_string, info.encoding)?;
        for (start, text) in matches {
            size = size
                .checked_add(replacement.len())
                .ok_or_else(|| AppError::new("file_too_large", "Output size overflow"))?;
            if size > MAX_FILE_BYTES as usize * 2 {
                return Err(AppError::new(
                    "file_too_large",
                    "Edit expansion exceeds memory budget",
                ));
            }
            spans.push((
                start,
                start + text.len(),
                replacement.clone(),
                edit.new_string.as_str(),
            ));
        }
    }
    spans.sort_by_key(|s| s.0);
    if spans.windows(2).any(|w| w[0].1 > w[1].0) {
        return Err(AppError::new(
            "overlapping_edits",
            "Edits overlap in the original file",
        ));
    }
    let boundaries: Vec<_> = spans.iter().flat_map(|s| [s.0, s.1]).collect();
    let raw = encoding::source_boundaries(&bytes, &info, &boundaries)?;
    let mut output = Vec::new();
    let mut new = String::new();
    let (mut byte_cursor, mut text_cursor) = (0, 0);
    for (i, (start, end, replacement, new_text)) in spans.iter().enumerate() {
        ctl.check()?;
        output.extend_from_slice(&bytes[byte_cursor..raw[2 * i]]);
        output.extend_from_slice(replacement);
        new.push_str(&old[text_cursor..*start]);
        new.push_str(new_text);
        byte_cursor = raw[2 * i + 1];
        text_cursor = *end;
    }
    output.extend_from_slice(&bytes[byte_cursor..]);
    new.push_str(&old[text_cursor..]);
    if output.len() as u64 > MAX_FILE_BYTES || new.len() as u64 > MAX_FILE_BYTES {
        return Err(AppError::new(
            "file_too_large",
            "Edited output exceeds 32 MiB",
        ));
    }
    let diff = preview(&old, &new);
    let next_revision = blake3::hash(&output).to_hex().to_string();
    if !req.dry_run {
        commit(ws, &path, &output, Some(&revision), ctl)?;
    }
    Ok(
        json!({"path": ws.relative(&path), "encoding": info, "replacements": spans.len(), "dry_run": req.dry_run,
        "applied": !req.dry_run, "previous_revision": revision, "revision": next_revision, "diff": diff}),
    )
}
