Use the GBK MCP server's read_file, write_file, edit_file, glob and grep for all
workspace text file operations. This project may contain both GBK and UTF-8.
Do not use apply_patch or shell redirection to rewrite existing project files.

Read before editing and pass the returned revision as expected_revision. Prefer
edit_file for partial changes; all edits in a call refer to the original file.
Preserve exact whitespace and CRLF/LF. Existing files keep their encoding; newly
created files default to UTF-8.
When rereading files just created as UTF-8, pass encoding=utf8 explicitly.

Ambiguous files default to GBK for reading/searching. Confirm their interpretation
and specify encoding explicitly when writing. Explicitly choose encoding before
introducing non-ASCII text into an ASCII file.

When grep/glob returns complete=false, zero matches do not establish absence.
Narrow the path/glob or increase timeout_ms, and inspect errors/stop_reason.
Use shell commands for builds/tests, not for reading or rewriting project text.
