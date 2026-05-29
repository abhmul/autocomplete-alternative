use autocomplete_server::run;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "autocomplete-broker", about = "Local HTTP autocomplete broker")]
struct Cli {
    /// Path to a TOML broker config file.
    #[arg(long)]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    run(cli.config).await?;
    Ok(())
}
