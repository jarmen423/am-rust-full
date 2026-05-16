//! Binary entrypoint for [`jina_ladybug_repo_index`] — embed a repo via Jina and write Ladybug native code nodes.

fn main() {
    if let Err(err) = jina_ladybug_repo_index::main_cli() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
