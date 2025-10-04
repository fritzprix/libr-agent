// types.rs - Request argument types
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct AddContentArgs {
    #[serde(rename = "fileUrl", alias = "file_url")]
    pub file_url: Option<String>,
    pub content: Option<String>,
    pub metadata: Option<AddContentMetadata>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddContentMetadata {
    pub filename: Option<String>,
    #[serde(rename = "mimeType", alias = "mime_type")]
    pub mime_type: Option<String>,
    pub size: Option<u64>,
    #[serde(rename = "uploadedAt", alias = "uploaded_at")]
    pub uploaded_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PaginationArgs {
    #[serde(default)]
    pub offset: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListContentArgs {
    #[serde(default)]
    pub pagination: Option<PaginationArgs>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReadContentArgs {
    pub content_id: String,
    pub from_line: Option<usize>,
    pub to_line: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchOptions {
    #[serde(rename = "topN", alias = "top_n")]
    #[serde(default)]
    pub top_n: Option<usize>,
    #[serde(default)]
    pub threshold: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct KeywordSearchArgs {
    pub query: String,
    #[serde(default)]
    pub options: Option<SearchOptions>,
}
