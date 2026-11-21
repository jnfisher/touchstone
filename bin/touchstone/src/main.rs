use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use touchstone::config::Config;

#[derive(Parser)]
#[command(version, about = "MMUD Explorer, as a web server")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the server.
    Serve {
        /// Path to touchstone.toml (defaults apply when omitted).
        #[arg(long)]
        config: Option<PathBuf>,
        /// Address to bind, overriding the config (default 0.0.0.0:8370).
        #[arg(long)]
        bind: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Serve { config, bind } => {
            let cfg = match config {
                Some(p) => Config::load(&p)?,
                None => Config::default(),
            };
            let started = touchstone::source::start(&cfg)?;
            if cfg
                .source
                .as_ref()
                .is_some_and(|s| s.reload_interval_secs.is_some())
            {
                eprintln!(
                    "touchstone: reload_interval_secs is ignored: game data reloads only when \
                     asked (the refresh button, or POST /api/reload)"
                );
            }
            let addr = bind.unwrap_or(cfg.bind);
            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .with_context(|| format!("binding {addr}"))?;
            eprintln!("touchstone listening on http://{addr}");
            axum::serve(listener, touchstone::app(started.state))
                .with_graceful_shutdown(async {
                    let _ = tokio::signal::ctrl_c().await;
                })
                .await?;
        }
    }
    Ok(())
}
