use clap::Parser;
use anyhow::Result;
use cody_core::{config::MapConfig, pipeline};

#[derive(Parser)]
#[command(name = "cody", about = "Codebase semantic indexer — generates a codemap.md")]
struct Cli {
    /// Directory to index
    dir: String,

    /// Output file path
    #[arg(long, default_value = "codemap.md")]
    out: String,

    /// Call graph traversal depth
    #[arg(long, default_value_t = 6)]
    depth: usize,

    /// Enable LSP hover queries to verify boundary event types
    #[arg(long)]
    lsp: bool,

    /// Minimum confidence threshold for boundary events
    #[arg(long, default_value_t = 0.5)]
    min_confidence: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("cody=info".parse()?)
                .add_directive("cody_core=info".parse()?)
        )
        .init();

    let cli = Cli::parse();
    let config = MapConfig {
        root_dir:       std::path::PathBuf::from(&cli.dir),
        out_path:       cli.out,
        max_depth:      cli.depth,
        use_lsp:        cli.lsp,
        min_confidence: cli.min_confidence,
    };
    pipeline::run_map(&config)
}
