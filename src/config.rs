use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct OidcConfig {
    // This has to be a string, because the Url crate likes to add a bad trailing slash.
    pub issuer: String,
    pub redirect_port: u16,
    pub client_id: String,
    pub resource: Option<String>,
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
        help = "The path of the secret signing key to use. \
        The file must be a TODO(subd): specify
        "
    )]
    pub signing_key: Option<PathBuf>,
    #[arg(
        long,
        help = "The internal localhost port to use for the HTTP server.",
        default_value_t = 3000u16
    )]
    pub port: u16,
    #[arg(long, help = "The webviewer URL to recommend, if any.")]
    pub webviewer: Option<String>,
    #[arg(
        long,
        help = "The path of the webviewer frontend to serve at the `/` route, if any."
    )]
    pub webviewer_path: Option<PathBuf>,
    #[arg(long, help = "The data directory to use.")]
    pub data_dir: PathBuf,
    #[arg(
        long,
        help = "If we should apply our authentication scheme (if any) to the Webviewer.",
        default_value_t = false
    )]
    pub no_webviewer_auth: bool,
    #[arg(
        long,
        help = "Whether the resource server should allow invalid certifications. VERY DANGEROUS: This should only be used in dev builds!"
    )]
    pub accept_invalid_certs: bool,
    #[arg(
        long,
        help = "The authentication scheme to use.",
        default_value = "none"
    )]
    auth: AuthenticationMode,
    // This has to be a string, because the Url crate likes to add a bad trailing slash.
    #[arg(long, help = "The OIDC issuer URL.", required_if_eq("auth", "oidc"))]
    oidc_issuer: Option<String>,
    #[arg(
        long,
        help = "The client ID port specified while registering the Backstitch client.",
        default_value = "backstitch"
    )]
    oidc_client_id: String,
    #[arg(
        long,
        help = "The localhost redirect port specified while registering the Backstitch client.",
        default_value_t = 58656u16
    )]
    oidc_redirect_port: u16,
    // TODO: Remove this once Endless implements RFC 9728
    #[arg(
        long,
        help = "Some OIDC providers require the `resource` parameter to be set to a specific URL to grant access. \
            This parameter tells Backstitch to use that URL."
    )]
    oidc_resource: Option<String>,
}

impl CommandConfig {
    pub fn authentication(&self) -> Authentication {
        match self.auth {
            AuthenticationMode::None => Authentication::None,
            AuthenticationMode::Oidc => Authentication::Oidc(OidcConfig {
                issuer: self.oidc_issuer.clone().unwrap(),
                client_id: self.oidc_client_id.clone(),
                redirect_port: self.oidc_redirect_port,
                resource: self.oidc_resource.clone(),
            }),
        }
    }
}
