use crate::{
    control::Control,
    error::{AppError, Result},
    files, search,
    types::*,
    workspace::Workspace,
};
use rmcp::{RoleServer, ServerHandler, model::*, service::RequestContext};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::Semaphore;

const INSTRUCTIONS: &str = "Use this server's read_file, write_file, edit_file, glob and grep for workspace file access. Existing files keep their GBK/UTF-8 encoding; new files default to UTF-8. Read before changing existing files and pass the returned revision as expected_revision. Ambiguous files default to GBK for reads/searches; specify encoding explicitly to edit. If complete=false, zero matches do NOT establish absence: narrow the path/glob or increase timeout_ms. Preserve exact CRLF/LF in old_string. Use edit_file for partial changes and write_file for complete contents. These tools are limited to the configured workspace root; they do not run shell commands.";

#[derive(Clone)]
pub struct GbkServer {
    workspace: Workspace,
    search_slots: Arc<Semaphore>,
    files_slots: Arc<Semaphore>,
    worker_slots: Arc<Semaphore>,
    workers_per_query: u32,
}

impl GbkServer {
    pub fn new(workspace: Workspace, threads: usize) -> Self {
        let threads = threads.clamp(1, 8);
        Self {
            workspace,
            search_slots: Arc::new(Semaphore::new(2)),
            files_slots: Arc::new(Semaphore::new(2)),
            worker_slots: Arc::new(Semaphore::new(threads)),
            workers_per_query: (threads / 2).max(1) as u32,
        }
    }

    pub fn tools() -> Vec<Tool> {
        vec![
            definition::<ReadRequest>(
                "read_file",
                "Read UTF-8/GBK text by lines (offset=1, limit=200). Returns content, encoding and a revision for subsequent edits. Ambiguous text defaults to GBK and is marked.",
                true,
            ),
            definition::<WriteRequest>(
                "write_file",
                "Create a text file (UTF-8 by default) or replace its complete content while preserving the existing encoding. Existing files require expected_revision from read_file. dry_run previews without writing.",
                false,
            ),
            definition::<EditRequest>(
                "edit_file",
                "Replace exact old_string with new_string, preserving original untouched bytes and encoding. All edits refer to the original file; unique matches required unless replace_all. Requires expected_revision; supports dry_run.",
                false,
            ),
            definition::<GlobRequest>(
                "glob",
                "Find files/directories by path glob, e.g. **/*.c or **/*; entry_type=directory/any also lists directories. Honors ignore rules by default. Results are bounded; inspect complete and stop_reason.",
                true,
            ),
            definition::<GrepRequest>(
                "grep",
                "Parallel GBK/UTF-8 content search. Supports regex or fixed_strings, context, glob and content/files/count modes (count counts matching lines). Results/time are bounded: complete=false is NOT proof of no matches.",
                true,
            ),
        ]
    }

    /// Public dispatch shared by MCP and integration tests/benchmarks.
    pub async fn execute(
        &self,
        name: &str,
        arguments: Value,
        token: tokio_util::sync::CancellationToken,
    ) -> Result<Value> {
        let is_search = matches!(name, "grep" | "glob");
        let timeout = if is_search {
            search::timeout(arguments.get("timeout_ms").and_then(Value::as_u64))?
        } else {
            60000
        };
        let ctl = Control::new(token.child_token(), timeout);
        // Cancellation on future drop reaches already-running blocking workers.
        let _cancel_guard = ctl.token.clone().drop_guard();
        let slots = if is_search {
            self.search_slots.clone()
        } else {
            self.files_slots.clone()
        };
        let acquire = slots.acquire_owned();
        let permit = tokio::select! {
            _ = ctl.token.cancelled() => return Err(AppError::new("cancelled", "Request cancelled while queued")),
            _ = tokio::time::sleep_until(ctl.deadline.into()) => return Err(AppError::new("timeout", "Request timed out while queued")),
            p = acquire => p.map_err(|_| AppError::new("internal", "Server is shutting down"))?,
        };
        let worker_permit = if is_search {
            Some(tokio::select! {
                _ = ctl.token.cancelled() => return Err(AppError::new("cancelled", "Request cancelled while queued")),
                _ = tokio::time::sleep_until(ctl.deadline.into()) => return Err(AppError::new("timeout", "Request timed out while queued")),
                p = self.worker_slots.clone().acquire_many_owned(self.workers_per_query) => p.map_err(|_| AppError::new("internal", "Server is shutting down"))?,
            })
        } else {
            None
        };
        let workspace = self.workspace.clone();
        let name = name.to_owned();
        let threads = self.workers_per_query as usize;
        tokio::task::spawn_blocking(move || {
            let (_permit, _workers) = (permit, worker_permit);
            ctl.check()?;
            match name.as_str() {
                "read_file" => files::read_file(&workspace, parse(arguments)?, &ctl),
                "write_file" => files::write_file(&workspace, parse(arguments)?, &ctl),
                "edit_file" => files::edit_file(&workspace, parse(arguments)?, &ctl),
                "glob" => search::glob(&workspace, parse(arguments)?, &ctl, threads),
                "grep" => search::grep(&workspace, parse(arguments)?, &ctl, threads),
                _ => Err(AppError::new("unknown_tool", "Unknown tool")),
            }
        })
        .await
        .map_err(|e| AppError::new("internal", e.to_string()))?
    }
}

fn parse<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| AppError::new("invalid_argument", e.to_string()))
}

fn definition<T: JsonSchema>(
    name: &'static str,
    description: &'static str,
    readonly: bool,
) -> Tool {
    let schema = serde_json::to_value(schemars::schema_for!(T)).expect("valid schema");
    Tool::new(name, description, schema.as_object().unwrap().clone()).with_annotations(
        ToolAnnotations::new()
            .read_only(readonly)
            .destructive(!readonly)
            .open_world(false),
    )
}

impl ServerHandler for GbkServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("gbk-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        Self::tools().into_iter().find(|tool| tool.name == name)
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, rmcp::ErrorData> {
        Ok(ListToolsResult::with_all_items(Self::tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResponse, rmcp::ErrorData> {
        let result = self
            .execute(
                &request.name,
                Value::Object(request.arguments.unwrap_or_default()),
                context.ct,
            )
            .await;
        let response = match result {
            Ok(value) => {
                let text = value.to_string();
                if text.len() > OUTPUT_BYTES {
                    CallToolResult::error(vec![ContentBlock::text(
                        "{\"code\":\"output_limit\",\"message\":\"Response exceeds 64 KiB; narrow the request\"}",
                    )])
                } else {
                    CallToolResult::success(vec![ContentBlock::text(text)])
                }
            }
            Err(error) => {
                let mut message = error.message;
                if message.len() > 2048 {
                    let mut end = 2048;
                    while !message.is_char_boundary(end) {
                        end -= 1;
                    }
                    message.truncate(end);
                }
                CallToolResult::error(vec![ContentBlock::text(
                    json!({"code": error.code, "message": message}).to_string(),
                )])
            }
        };
        Ok(response.into())
    }
}
