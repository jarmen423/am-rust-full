//! Stable primary-key conventions matching `LadybugNativeCodeIndexer` in
//! `agentic_memory.ladybug.code_indexer`.

/// SHA256 hex digest of UTF-8 `text`.
pub(crate) fn sha256_hex_utf8(text: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// MD5 hex of raw file bytes (`code_indexer._file_hash`).
pub(crate) fn md5_hex(bytes: &[u8]) -> String {
    use md5::{Digest, Md5};
    let d = Md5::digest(bytes);
    d.iter().map(|b| format!("{:02x}", b)).collect()
}

pub(crate) fn file_id(repo_id: &str, rel_path: &str) -> String {
    format!("file:{}", sha256_hex_utf8(&format!("{repo_id}:{rel_path}")))
}

pub(crate) fn document_id(repo_id: &str, rel_path: &str) -> String {
    format!(
        "code-document:{}",
        sha256_hex_utf8(&format!("{repo_id}:{rel_path}"))
    )
}

pub(crate) fn function_id(repo_id: &str, signature: &str) -> String {
    format!("function:{}", sha256_hex_utf8(&format!("{repo_id}:{signature}")))
}

pub(crate) fn class_id(repo_id: &str, signature: &str) -> String {
    format!("class:{}", sha256_hex_utf8(&format!("{repo_id}:{signature}")))
}

/// Chunk id matches `_chunk_id` Python helper.
pub(crate) fn chunk_id(
    repo_id: &str,
    target_kind: &str,
    target_signature: &str,
    text: &str,
) -> String {
    format!(
        "chunk:{}",
        sha256_hex_utf8(&format!(
            "{repo_id}:{target_kind}:{target_signature}:{text}"
        ))
    )
}
