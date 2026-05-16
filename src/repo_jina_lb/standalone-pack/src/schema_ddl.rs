//! Native **code** DDL aligned with Agentic Memory **CODE_SCHEMA**
//! (`agentic_memory/ladybug/schema.py` in the main Agentic Memory repo), with the `Chunk.embedding` width
//! parameterized for the **Jina v4 dense-max** profile (`FLOAT[2048]`).

/// Maximum dense width for `jina-embeddings-v4` in our Ladybug ingest profile.
///
/// Mirror: `VECTOR_DIMENSIONS_JINA_EMBEDDINGS_V4_MAX` in `agentic_memory/ladybug/schema.py`.
pub(crate) const JINA_EMBED_DIM_V4_MAX: usize = 2048;

/// Ordered DDL mirroring `LadybugTableGroup(name="code").ddl` from `schema.py`.
pub(crate) fn native_code_ddl(embed_dims: usize) -> Vec<String> {
    let chunk = format!(
        "CREATE NODE TABLE Chunk(\n\
  id STRING PRIMARY KEY,\n\
  repo_id STRING,\n\
  path STRING,\n\
  name STRING,\n\
  text STRING,\n\
  embedding FLOAT[{embed_dims}],\n\
  properties_json STRING\n\
);"
    );
    vec![
        r#"CREATE NODE TABLE CodeDocument(
  id STRING PRIMARY KEY,
  repo_id STRING,
  path STRING,
  name STRING,
  source_hash STRING,
  pending_source_hash STRING,
  index_status STRING,
  repo_root STRING,
  metadata_json STRING,
  updated_at STRING
);"#
        .to_string(),
        r#"CREATE NODE TABLE File(
  id STRING PRIMARY KEY,
  repo_id STRING,
  path STRING,
  name STRING,
  text STRING,
  properties_json STRING
);"#
        .to_string(),
        r#"CREATE NODE TABLE Function(
  id STRING PRIMARY KEY,
  repo_id STRING,
  path STRING,
  name STRING,
  qualified_name STRING,
  signature STRING,
  docstring STRING,
  text STRING,
  properties_json STRING
);"#
        .to_string(),
        r#"CREATE NODE TABLE Class(
  id STRING PRIMARY KEY,
  repo_id STRING,
  path STRING,
  name STRING,
  qualified_name STRING,
  docstring STRING,
  text STRING,
  properties_json STRING
);"#
        .to_string(),
        chunk,
        "CREATE REL TABLE DEFINES(FROM File TO Function, FROM File TO Class, properties_json STRING, MANY_MANY);".to_string(),
        "CREATE REL TABLE IMPORTS(FROM File TO File, properties_json STRING, MANY_MANY);".to_string(),
        "CREATE REL TABLE HAS_METHOD(FROM Class TO Function, properties_json STRING, MANY_MANY);".to_string(),
        "CREATE REL TABLE DESCRIBES(FROM Chunk TO Function, FROM Chunk TO Class, properties_json STRING, MANY_MANY);".to_string(),
        "CREATE REL TABLE CALLS(FROM Function TO Function, properties_json STRING, MANY_MANY);".to_string(),
    ]
}

pub(crate) const INSTALL_VECTOR_SQL: &str = "INSTALL VECTOR; LOAD VECTOR;";

/// Same statement as Python `LadybugNativeCodeIndexer.VECTOR_INDEX_STATEMENT`.
pub(crate) const CREATE_CODE_VECTOR_INDEX_SQL: &str = r#"CALL CREATE_VECTOR_INDEX(
  "Chunk",
  "code_chunk_embedding_index",
  "embedding",
  metric := "cosine"
);"#;
