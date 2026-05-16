//! Hosted Jina Search Foundation embeddings client (`POST /v1/embeddings`).
//!
//! We default to `jina-embeddings-v4` dense vectors at **maximum Matryoshka width** (2048 by Jina docs)
//! using `task: "code.passage"` for repository passages.
//!
//! Design notes (see Jina embeddings API docs):
//! - **`late_chunking`**: Intended for models like v3 when you want chunk embeddings derived from full-document
//!   context. We already AST-chunk locally, so **`late_chunking=false`** avoids double semantics.
//! - **`truncate=true`**: Safe when individual snippet UTF-8 can exceed deployment limits — drops tail vs hard error.
//! - **`return_multivector=false`**: Ladybug's built-in vector index consumes **dense** fixed-width `FLOAT[N]` arrays;
//!   multi-vector / late-interaction payloads need a separate storage/query story.

use reqwest::blocking::Client;
use serde::Deserialize;

const DEFAULT_ENDPOINT: &str = "https://api.jina.ai/v1/embeddings";

#[derive(Debug, Deserialize)]
struct EmbedResponseItem {
    #[serde(default)]
    embedding: Option<Vec<f32>>,
    #[serde(default)]
    index: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct EmbedResponseRaw {
    data: Option<Vec<EmbedResponseItem>>,
}

#[derive(Debug)]
pub(crate) struct JinaEmbedOpts<'a> {
    pub(crate) api_key: &'a str,
    pub(crate) model: &'a str,
    pub(crate) task: &'a str,
    pub(crate) dimensions: u32,
    pub(crate) truncate: bool,
    pub(crate) late_chunking: bool,
    pub(crate) normalized: bool,
    pub(crate) return_multivector: bool,
}

impl Default for JinaEmbedOpts<'_> {
    fn default() -> Self {
        Self {
            api_key: "",
            model: "jina-embeddings-v4",
            task: "code.passage",
            dimensions: 2048,
            truncate: true,
            late_chunking: false,
            normalized: true,
            return_multivector: false,
        }
    }
}

/// Embed a batch of passages; returns vectors in API order (`data[*].embedding`).
///
/// Caller must honor Jina throughput limits (hosted v4 is throttled; batch conservatively).
pub(crate) fn embed_passages_blocking(
    client: &Client,
    passages: &[String],
    opts: &JinaEmbedOpts<'_>,
) -> Result<Vec<Vec<f32>>, String> {
    if passages.is_empty() {
        return Ok(vec![]);
    }
    if opts.api_key.is_empty() {
        return Err(
            "JINA_API_KEY is empty; set env JINA_API_KEY or pass via CLI --jina-api-key".into(),
        );
    }

    let body = serde_json::json!({
        "model": opts.model,
        "task": opts.task,
        "input": passages,
        "dimensions": opts.dimensions,
        "truncate": opts.truncate,
        "late_chunking": opts.late_chunking,
        "normalized": opts.normalized,
        "return_multivector": opts.return_multivector,
    });

    let resp = client
        .post(DEFAULT_ENDPOINT)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", opts.api_key))
        .json(&body)
        .send()
        .map_err(|e| format!("Jina embeddings HTTP transport error: {e}"))?;

    let status = resp.status();
    let text = resp
        .text()
        .map_err(|e| format!("reading Jina response body failed: {e}"))?;

    if !status.is_success() {
        let head = text.chars().take(800).collect::<String>();
        return Err(format!(
            "Jina embeddings HTTP {status}: {}",
            head.replace('\n', " ")
        ));
    }

    let parsed: EmbedResponseRaw =
        serde_json::from_str(&text).map_err(|e| format!("invalid JSON from Jina: {e}"))?;

    let mut items = parsed.data.unwrap_or_default();
    // Sort defensively by `.index`
    items.sort_by_key(|item| item.index.unwrap_or(-1));

    let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(items.len());
    for item in items {
        match item.embedding {
            Some(v) => vectors.push(v),
            None => return Err("Jina embeddings response contained an item without embedding[]".into()),
        }
    }

    if vectors.len() != passages.len() {
        return Err(format!(
            "Jina embeddings count mismatch (expected {}, got {}).",
            passages.len(),
            vectors.len()
        ));
    }

    Ok(vectors)
}
