use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct OidcConfig {
    pub endpoint: Url,
    pub redirect_port: u16,
    pub client_id: String,
}

#[derive(Debug, Clone)]
pub enum Authentication {
    None,
    Oidc(OidcConfig),
}

#[derive(ValueEnum, Debug, Clone, Serialize)]
enum AuthenticationMode {
    None,
    Oidc,
}

#[derive(Parser, Debug, Clone)]
#[command(rename_all = "kebab-case")]
pub struct CommandConfig {
    #[arg(
        long,
        help = "The internal localhost port to use for the HTTP server.",
        default_value = "3000"
    )]
    pub port: u16,
    #[arg(long, help = "The webviewer URL to recommend, if any.")]
    pub webviewer: Option<Url>,
    #[arg(long, help = "The data directory to use.")]
    pub data_dir: PathBuf,
    #[arg(
        long,
        help = "If we should apply our authentication scheme (if any) to the Webviewer.",
        default_value = "true"
    )]
    pub webviewer_endpoint_auth: bool,
    #[arg(
        long,
        help = "The authentication scheme to use.",
        default_value = "none"
    )]
    auth: AuthenticationMode,
    #[arg(long, help = "The OIDC issuer URL.", required_if_eq("auth", "oidc"))]
    oidc_issuer: Option<Url>,
    #[arg(
        long,
        help = "The client ID port specified while registering the Backstitch client.",
        default_value = "backstitch"
    )]
    oidc_client_id: String,
    #[arg(
        long,
        help = "The localhost redirect port specified while registering the Backstitch client.",
        default_value = "58656"
    )]
    oidc_redirect_port: u16,
}

impl CommandConfig {
    pub fn authentication(&self) -> Authentication {
        match self.auth {
            AuthenticationMode::None => Authentication::None,
            AuthenticationMode::Oidc => Authentication::Oidc(OidcConfig {
                endpoint: self.oidc_issuer.clone().unwrap(),
                client_id: self.oidc_client_id.clone(),
                redirect_port: self.oidc_redirect_port,
            }),
        }
    }
}
