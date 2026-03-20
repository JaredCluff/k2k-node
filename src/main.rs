use clap::{Parser, Subcommand};

mod cli;
mod config;
mod server;
mod handlers;
mod middleware;
mod keys;
mod capabilities;
mod tasks;
mod discovery;
mod indexer;
mod embeddings;
mod vectordb;
mod db;

#[derive(Parser)]
#[command(name = "k2k-node", version, about = "K2K federation node — lightweight reference implementation")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Path to config file
    #[arg(short, long, default_value = "config.yaml")]
    config: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the K2K server
    Start,
    /// Index files at a path
    Index {
        /// Path to index
        path: String,
    },
    /// Show node status
    Status,
    /// Register with a peer node
    Register {
        /// Peer node URL
        peer_url: String,
    },
    /// Approve a pending client
    Approve {
        /// Client ID to approve
        client_id: String,
    },
    /// Query a peer node
    Query {
        /// Peer node URL
        peer_url: String,
        /// Search query
        query: String,
        /// Number of results
        #[arg(short = 'k', long, default_value = "10")]
        top_k: usize,
    },
    /// List discovered peers
    Peers,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "k2k_node=info".into()),
        )
        .init();

    let cli = Cli::parse();
    let config = config::K2KNodeConfig::load(&cli.config)?;

    match cli.command {
        Commands::Start => cli::start::run(config).await,
        Commands::Index { path } => cli::index::run(config, &path).await,
        Commands::Status => cli::status::run(config).await,
        Commands::Register { peer_url } => cli::register::run(config, &peer_url).await,
        Commands::Approve { client_id } => cli::approve::run(config, &client_id).await,
        Commands::Query { peer_url, query, top_k } => {
            cli::query::run(config, &peer_url, &query, top_k).await
        }
        Commands::Peers => cli::peers::run(config).await,
    }
}
