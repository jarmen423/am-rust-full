use std::env;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub store_path: String,
    pub vault_path: String,
    pub dist_path: String,
}

impl ServerConfig {
    pub fn from_env() -> Self {
        let port = env::var("PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3031);
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let store_path = env::var("WORKSPACE_STORE_PATH")
            .unwrap_or_else(|_| format!("{}/.agentic-memory/workspace-store", home));
        let vault_path = env::var("WORKSPACE_VAULT_PATH")
            .unwrap_or_else(|_| format!("{}/.agentic-memory/workspace-vaults", home));
        let dist_path = env::var("DIST_PATH").unwrap_or_else(|_| "dist".into());
        Self {
            port,
            store_path,
            vault_path,
            dist_path,
        }
    }
}
