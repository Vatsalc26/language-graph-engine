use language_graph_engine::config::Config;
use language_graph_engine::app::AppState;
use language_graph_engine::server::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Language Graph Engine (Phase 1) ===");
    
    // Load default config
    let config = Config::default();
    
    println!("Database location: {:?}", config.db_path);

    // Initialize application state (creates DB, runs migrations, seeds, loads resolver)
    let state = AppState::new(config)?;

    // Run Axum server
    Server::run(state).await?;

    Ok(())
}
