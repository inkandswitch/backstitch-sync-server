use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use serde::Serialize;
use url::Url;

#[derive(ValueEnum, Debug, Clone, Serialize)]
pub enum Authentication {
    None,
}

#[derive(Parser, Debug, Clone)]
#[command(rename_all = "kebab-case")]
pub struct CommandConfig {
    #[arg(long)]
    #[clap(
        help = "The public sync port to use. This will be given to the Backstitch client during the HTTP handshake."
    )]
    pub public_sync_port: Url,
    #[arg(long)]
    #[clap(help = "The internal localhost port to use for the sync server.")]
    pub sync_port: u16,
    #[arg(long)]
    #[clap(help = "The internal localhost port to use for the HTTP server.")]
    pub http_port: u16,
    #[arg(long)]
    #[clap(help = "The authentication scheme to use.")]
    pub auth: Authentication,
    #[arg(long)]
    #[clap(help = "The webviewer URL to recommend, if any.")]
    pub webviewer: Option<Url>,
    #[arg(long)]
    #[clap(help = "The data directory to use.")]
    pub data_dir: PathBuf,
}
