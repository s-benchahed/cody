use clap::{Parser, Subcommand};
use anyhow::Result;
use cody_core::{config::IndexConfig, db, pipeline, query, search, embed};

#[derive(Parser)]
#[command(name = "cody", about = "Codebase semantic indexer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the index for a directory
    Index {
        dir:              String,
        #[arg(long, default_value = "index.db")]
        db:               String,
        #[arg(long, default_value_t = 6)]
        depth:            usize,
        #[arg(long)]
        skip_embed:       bool,
        /// Run LSP hover queries to verify boundary event types (requires
        /// language servers in PATH: typescript-language-server, rust-analyzer, pyright-langserver)
        #[arg(long)]
        lsp:              bool,
        #[arg(long, default_value_t = 0.5)]
        min_confidence:   f64,
        #[arg(long)]
        all_entrypoints:  bool,
    },

    /// Query the index
    Query {
        #[arg(long, default_value = "index.db")]
        db: String,
        #[command(subcommand)]
        command: QueryCommand,
    },

    /// Embed traces using OpenAI API
    Embed {
        #[arg(long, default_value = "index.db")]
        db:      String,
        #[arg(long, env = "OPENAI_API_KEY")]
        api_key: String,
        #[arg(long, default_value = "text-embedding-3-small")]
        model:   String,
    },

    /// Natural language search over the index
    Search {
        #[arg(long, default_value = "index.db")]
        db:      String,
        #[arg(long, env = "ANTHROPIC_API_KEY")]
        api_key: String,
        question: String,
    },
}

#[derive(Subcommand)]
enum QueryCommand {
    Stats,
    Lookup { name: String },
    Callers { symbol: String },
    Callees { symbol: String },
    Deps { file: String },
    Path { from: String, to: String },
    Boundaries { fn_name: String },
    Medium { medium: String },
    Cross { service_a: String, service_b: String },
    Traces { fn_name: String },
    /// Show service dependency graph derived from boundary flows
    Topology,
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

    match cli.command {
        Commands::Index { dir, db, depth, skip_embed, lsp, min_confidence, all_entrypoints } => {
            let config = IndexConfig {
                root_dir: std::path::PathBuf::from(&dir),
                db_path: db,
                max_depth: depth,
                skip_embed,
                use_lsp: lsp,
                min_confidence,
                all_entrypoints,
                ..Default::default()
            };
            pipeline::run_index(&config)?;
        }

        Commands::Query { db, command } => {
            let conn = db::open(&db)?;
            match command {
                QueryCommand::Stats => {
                    let s = cody_core::db::store::stats(&conn)?;
                    println!("{}", serde_json::to_string_pretty(&s)?);
                }
                QueryCommand::Lookup    { name }             => query::cmd_lookup(&conn, &name)?,
                QueryCommand::Callers   { symbol }           => query::cmd_callers(&conn, &symbol)?,
                QueryCommand::Callees   { symbol }           => query::cmd_callees(&conn, &symbol)?,
                QueryCommand::Deps      { file }             => query::cmd_deps(&conn, &file)?,
                QueryCommand::Path      { from, to }         => query::cmd_path(&conn, &from, &to)?,
                QueryCommand::Boundaries { fn_name }         => query::cmd_boundaries(&conn, &fn_name)?,
                QueryCommand::Medium    { medium }           => query::cmd_medium(&conn, &medium)?,
                QueryCommand::Cross     { service_a, service_b } => query::cmd_cross(&conn, &service_a, &service_b)?,
                QueryCommand::Traces    { fn_name }          => query::cmd_traces(&conn, &fn_name)?,
                QueryCommand::Topology                       => query::cmd_topology(&conn)?,
            }
        }

        Commands::Embed { db, api_key, model } => {
            let conn = db::open(&db)?;
            embed::embed_traces(&conn, &api_key, &model).await?;
        }

        Commands::Search { db, api_key, question } => {
            let conn = db::open(&db)?;
            let answer = search::run_query(&question, &conn, &api_key).await?;
            println!("{answer}");
        }
    }

    Ok(())
}
