Use the GBK MCP server's read_file, write_file, edit_file, glob and grep for all
workspace text file operations. This project may contain both GBK and UTF-8.
Do not use built-in text editing or shell redirection to rewrite existing files.

Read a file before editing and pass its revision as expected_revision. Prefer
edit_file for partial changes. Preserve exact CRLF/LF in old_string. Existing
files keep their encoding; new files default to UTF-8.
When rereading files just created as UTF-8, pass encoding=utf8 explicitly.

If encoding.status is ambiguous, reading/searching defaults to GBK. Confirm the
intended interpretation and pass encoding explicitly before writing. For an ASCII
file, explicitly choose encoding before introducing non-ASCII characters.

If grep/glob returns complete=false, do not infer that no matches exist. Narrow
the path/glob or increase timeout_ms, and inspect error_counts/stop_reason.
Shell commands remain available for building and testing, not rewriting text.
