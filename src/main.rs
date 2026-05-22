#![feature(path_absolute_method)]
use std::io;

use mdbook_preprocessor::book::{Book, BookItem};
use mdbook_preprocessor::{Preprocessor, PreprocessorContext};
use toml::value::Table;

use crate::config::{Config, ConfigError};
use crate::highlighter::RustAnalyzerHighlighter;

mod addon;
mod config;
mod highlight_conf;
mod highlighter;
mod inlay_hint_conf;
mod whichlang;

struct HighlighterPreprocessor;

impl Preprocessor for HighlighterPreprocessor {
    fn name(&self) -> &str {
        "mdbook-rust-analyzer-highlight"
    }

    fn run(
        &self,
        ctx: &PreprocessorContext,
        mut book: Book,
    ) -> Result<Book, mdbook_preprocessor::errors::Error> {
        let conf_raw = ctx
            .config
            .get::<Table>(&format!(
                "preprocessor.{}",
                self.name()
            ))?
            .ok_or(ConfigError::ConfigNotFound)?;

        let config = Config::try_from(&conf_raw)?;

        let mut highlighter =
            RustAnalyzerHighlighter::new(&config);

        book.for_each_mut(|item| {
            if let BookItem::Chapter(ch) = item {
                ch.content = highlighter.process_markdown(
                    ch.source_path
                        .clone()
                        .unwrap_or("SUMMARY.md".into())
                        .as_path(),
                    &ch.content,
                );
            }
        });

        Ok(book)
    }

    fn supports_renderer(
        &self,
        renderer: &str,
    ) -> Result<bool, mdbook_preprocessor::errors::Error> {
        Ok(renderer == "html")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let preprocessor = HighlighterPreprocessor;
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("supports") => {
            let renderer = args.next().unwrap_or_default();
            std::process::exit(
                if preprocessor.supports_renderer(&renderer)? {
                    0
                } else {
                    1
                },
            );
        }
        _ => {
            let (ctx, book) =
                mdbook_preprocessor::parse_input(io::stdin())?;
            let result = preprocessor.run(&ctx, book)?;
            serde_json::to_writer(io::stdout(), &result)?;
        }
    }
    Ok(())
}
