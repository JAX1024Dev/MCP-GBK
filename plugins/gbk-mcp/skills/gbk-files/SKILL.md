---
description: Read, search, create and edit workspace text files through GBK MCP. Use for GBK, CP936, UTF-8 and mixed-encoding projects, or when built-in text tools are blocked by GBK protection.
---

Use this plugin's GBK MCP tools for workspace text operations:

| Operation | Tool |
| --- | --- |
| Read text | read_file |
| Create/overwrite text | write_file |
| Exact partial changes | edit_file |
| Find files/directories | glob |
| Search text | grep |

Read existing files first; pass the returned revision as expected_revision.
Existing files keep their encoding. Ambiguous files need explicit encoding before
editing. Preserve CRLF/LF exactly in old_string. Prefer dry_run for a diff preview;
submit using the original revision, not the preview's new revision.

complete=false means the search is incomplete, even with zero matches. Narrow the
path/glob or increase timeout_ms. Do not infer absence from a partial search.

Built-in Read remains available for images, PDFs and notebooks. Shell remains
available for builds, Git and programs, not for bypassing text protection. A hook
denial does not perform the requested read or edit. Call the actual MCP tool next.
If these tools are missing, use /mcp to diagnose the connection. A subagent without
the tools must report to its parent instead of using native text tools or Shell.
