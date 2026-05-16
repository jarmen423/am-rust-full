//! CLI entrypoint: Jina v4 embeddings into native Agentic Memory–shaped Ladybug code graph rows.
//!
//! Build:
//! ```text
//! cargo build -p am-workspace --bin jina-ladybug-repo-index --features jina-ladybug-index
//! ```

fn main() {
    if let Err(err) = am_workspace::repo_jina_lb::main_cli() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
