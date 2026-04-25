use std::path::PathBuf;

use ra_ap_ide::{HighlightConfig, InlayHintsConfig};
use thiserror::Error;

use crate::{
    highlight_conf::HighlightConfigDe,
    inlay_hint_conf::InlayHintsConfigDe,
};

pub struct Config<'a> {
    pub project_root: PathBuf,
    pub highlight_config: HighlightConfig<'a>,
    pub inlay_hint_config: InlayHintsConfig<'a>,
    pub whichlang_support: bool,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("\"project-root\" is not valid")]
    InvalidProjectRoot,
    #[error(
        "\"mdbook-rust-analyzer-highlight\" configuration is not found"
    )]
    ConfigNotFound,
}

impl TryFrom<&toml::value::Table> for Config<'_> {
    type Error = ConfigError;

    fn try_from(
        value: &toml::value::Table,
    ) -> Result<Self, Self::Error> {
        let project_root = value
            .get("project-root")
            .and_then(|v| v.as_str())
            .ok_or(ConfigError::InvalidProjectRoot)?
            .into();
        let hl_conf_path = value
            .get("highlight-config")
            .and_then(|v| v.as_str())
            .unwrap_or_else(||{
                eprintln!("[ INFO ]: \"highlight-config\" was not found, using default config");
                ""
            });
        let inlay_conf_path = value
            .get("inlay-hint-config")
            .and_then(|v| v.as_str())
            .unwrap_or_else(||{
                eprintln!("[ INFO ]: \"inlay-hint-config\" was not found, using default config");
                ""
            });

        let whichlang_support = value
            .get("whichlang")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let highlight_config: HighlightConfig<'_> =
            HighlightConfigDe::from_file(hl_conf_path).into();
        let inlay_hint_config: InlayHintsConfig =
            InlayHintsConfigDe::from_file(inlay_conf_path)
                .into();

        Ok(Self {
            project_root,
            highlight_config,
            inlay_hint_config,
            whichlang_support,
        })
    }
}
