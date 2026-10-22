use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const OUTPUT_BYTES: usize = 64 * 1024;
pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
pub const LINE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EncodingChoice {
    #[default]
    Auto,
    #[serde(alias = "utf-8")]
    Utf8,
    #[serde(alias = "cp936")]
    Gbk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    Utf8,
    Gbk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EncodingStatus {
    Explicit,
    Detected,
    Ambiguous,
    Ascii,
}

#[derive(Clone, Debug, Serialize)]
pub struct EncodingInfo {
    pub encoding: Encoding,
    pub status: EncodingStatus,
    pub bom: bool,
    pub candidates: Vec<Encoding>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadRequest {
    pub path: String,
    /// One-based first line. Defaults to 1.
    pub offset: Option<usize>,
    /// Maximum number of lines, default 200.
    pub limit: Option<usize>,
    #[serde(default)]
    pub encoding: EncodingChoice,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteRequest {
    pub path: String,
    pub content: String,
    /// Required when replacing an existing file; obtain from read_file.
    pub expected_revision: Option<String>,
    #[serde(default)]
    pub encoding: EncodingChoice,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Replacement {
    /// Must be nonempty and match the original text exactly, including newlines.
    pub old_string: String,
    pub new_string: String,
    /// Replace every occurrence. Otherwise require one unique occurrence.
    #[serde(default)]
    pub replace_all: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditRequest {
    pub path: String,
    pub edits: Vec<Replacement>,
    pub expected_revision: String,
    #[serde(default)]
    pub encoding: EncodingChoice,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    #[default]
    File,
    Directory,
    Any,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GlobRequest {
    /// Glob relative to the search directory, e.g. **/*.c; **/* lists entries.
    pub pattern: String,
    /// Defaults to the configured root.
    pub path: Option<String>,
    #[serde(default)]
    pub entry_type: EntryType,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub include_hidden: bool,
    /// Defaults to true. Includes .gitignore and .ignore.
    pub respect_ignore: Option<bool>,
    pub max_depth: Option<usize>,
    pub max_results: Option<usize>,
    /// Query budget, default 10000ms; at most 300000ms.
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    #[default]
    Content,
    Files,
    Count,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GrepRequest {
    pub pattern: String,
    pub path: Option<String>,
    /// Optional file path glob relative to path.
    pub glob: Option<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub fixed_strings: bool,
    #[serde(default)]
    pub ignore_case: bool,
    #[serde(default)]
    pub output_mode: OutputMode,
    /// Number of context lines before and after a match, maximum 20.
    pub context: Option<usize>,
    #[serde(default)]
    pub include_hidden: bool,
    pub respect_ignore: Option<bool>,
    pub max_depth: Option<usize>,
    pub max_results: Option<usize>,
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub encoding: EncodingChoice,
}
