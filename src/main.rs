#![feature(path_absolute_method)]
use mdbook::book::{Book, BookItem};
use mdbook::errors::Error;
use mdbook::preprocess::{
    CmdPreprocessor, Preprocessor, PreprocessorContext,
};
use mdbook_include_rs::parser::process_directives;
use proc_macro2::Span;
use ra_ap_ide::{
    AdjustmentHints, AnalysisHost, GenericParameterHints,
    Highlight, HighlightConfig, HlRange, HlTag,
    InlayFieldsToResolve, InlayHint, InlayHintPosition,
    InlayHintsConfig, InlayKind, SymbolKind, TextRange,
};
use ra_ap_ide_db::MiniCore;
use ra_ap_ide_db::base_db::SourceDatabase;
use ra_ap_load_cargo::{
    LoadCargoConfig, ProcMacroServerChoice, load_workspace_at,
};
use ra_ap_project_model::CargoConfig;
use ra_ap_vfs::{AbsPathBuf, FileId, Vfs, VfsPath};
use regex::Regex;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{io, usize};

mod highlight_conf;
mod inlay_hint_conf;

const HLRS_CODEBLOCK_REGEX: &str =
    r"```rust(?:,?([^\n]+))?\n([\s\S]*?)\n?```";
const RUST_ICON_URL: &str =
    "@https://www.rust-lang.org/static/images/rust-logo-blk.svg";
const DIRECTIVE_REGEX: &str = r"(?ms)^#!\[((?:source_file|function|struct|enum|trait|impl|impl_method|trait_impl|function_body)![\s\S]*?)\]$";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let preprocessor = RaHighlight;
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("supports") => {
            let renderer = args.next().unwrap_or_default();
            std::process::exit(
                if preprocessor.supports_renderer(&renderer) {
                    0
                } else {
                    1
                },
            );
        }
        _ => {
            let (ctx, book) =
                CmdPreprocessor::parse_input(io::stdin())?;
            let result = preprocessor.run(&ctx, book)?;
            serde_json::to_writer(io::stdout(), &result)?;
        }
    }
    Ok(())
}

struct RaHighlight;

impl Preprocessor for RaHighlight {
    fn name(&self) -> &str {
        "mdbook-rust-analyzer-highlight"
    }

    fn run(
        &self,
        ctx: &PreprocessorContext,
        mut book: Book,
    ) -> Result<Book, Error> {
        let project_root = ctx
            .config
            .get_preprocessor(self.name())
            .and_then(|t| t.get("project-root"))
            .and_then(|v| v.as_str())
            .unwrap();

        let mut highlighter: Box<WorkspaceHighlighter> =
            Box::new(WorkspaceHighlighter::load(project_root));

        let support = self.whichlang_support(ctx);

        book.for_each_mut(|item| {
            if let BookItem::Chapter(ch) = item {
                ch.content =
                    highlighter.as_mut().process_markdown(
                        ch.source_path
                            .clone()
                            .unwrap_or("SUMMARY.md".into())
                            .as_path(),
                        &ch.content,
                        support,
                    );
            }
        });

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

impl RaHighlight {
    fn whichlang_support(
        &self,
        ctx: &PreprocessorContext,
    ) -> bool {
        if let Some(cfg) = ctx
            .config
            .get(&format!("preprocessor.{}", self.name()))
        {
            return match cfg.get("whichlang") {
                Some(feature) => feature.as_bool().expect(
                    "\nERROR: `whichlang` configuration should be a \
                     boolean",
                ),
                None => {
                    false
                }
            };
        }
        false
    }
}

pub struct WorkspaceHighlighter {
    root: PathBuf,
    host: AnalysisHost,
    vfs: Vfs,
    /// Cache of highlighted snippets
    hl_cache: HashMap<FileId, String>,
}

impl WorkspaceHighlighter {
    /// Load the Cargo workspace at `project_root`
    pub fn load(project_root: &str) -> Self {
        let root = Path::new(project_root);
        let cargo_toml = root.join("Cargo.toml");

        let load_cfg = LoadCargoConfig {
            load_out_dirs_from_check: true,
            with_proc_macro_server:
                ProcMacroServerChoice::Sysroot,
            prefill_caches: false,
            num_worker_threads: 4,
            proc_macro_processes: 4,
        };

        let (db, vfs, _proc_macros) = load_workspace_at(
            cargo_toml.as_ref(),
            &CargoConfig {
                sysroot: Some(
                    ra_ap_project_model::RustLibSource::Discover,
                ),
                ..Default::default()
            },
            &load_cfg,
            &|msg| eprintln!("[ra] {msg}"),
        )
        .expect("failed to load Cargo workspace");

        Self {
            root: root.to_path_buf(),
            host: AnalysisHost::with_database(db),
            vfs,
            hl_cache: HashMap::new(),
        }
    }
}

impl WorkspaceHighlighter {
    fn highlight_snippet(
        &mut self,
        file_path: PathBuf,
        span: Vec<Span>,
    ) -> Option<String> {
        let analysis = self.host.analysis();
        let vfs_path = VfsPath::from(AbsPathBuf::assert(
            file_path
                .absolute()
                .unwrap()
                .try_into()
                .expect("Path is not a valid UTF-8"),
        ));

        let (file_id, _excluded) =
            self.vfs.file_id(&vfs_path)?;

        let mut highlights = analysis
            .highlight(HIGHLIGHT_CONFIG, file_id)
            .unwrap_or_default();

        let mut inlay_hints = analysis
            .inlay_hints(&INLAY_HINT_CONFIG, file_id, None)
            .unwrap_or_default();

        let highlighted = match self.hl_cache.get(&file_id) {
            Some(highlighted) => highlighted,
            None => {
                let code = self
                    .host
                    .raw_database()
                    .file_text(file_id)
                    .text(self.host.raw_database());

                let highlighted = ranges_to_html(
                    code,
                    &mut highlights,
                    &mut inlay_hints,
                );
                self.hl_cache.insert(file_id, highlighted);
                // Safe, just instered into this file_id
                unsafe {
                    self.hl_cache
                        .get(&file_id)
                        .unwrap_unchecked()
                }
            }
        };

        let mut out = String::new();

        for s in span {
            out.push_str(
                &highlighted
                    .lines()
                    .skip(s.start().line - 1)
                    .take(s.end().line - s.start().line + 1)
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            out.push_str("\n\n");
        }

        Some(out)
    }

    fn extract_whichlang_features<'a>(
        &self,
        f: Option<regex::Match<'a>>,
    ) -> String {
        let mut feature_string = match f {
            Some(feature) => feature.as_str().replace(',', " "),
            None => String::from(""),
        };
        if !feature_string.contains("icon=@https://") {
            feature_string.push_str(" icon=");
            feature_string.push_str(RUST_ICON_URL);
        }
        feature_string
    }

    fn process_markdown(
        &mut self,
        source_path: &Path,
        content: &str,
        whichlang_support: bool,
    ) -> String {
        let re = Regex::new(HLRS_CODEBLOCK_REGEX).unwrap();
        let directive_re = Regex::new(DIRECTIVE_REGEX).unwrap();

        re.replace_all(content, |caps: &regex::Captures| {
            let mut features = String::from("");
            if whichlang_support {
                features.push_str(
                    &self.extract_whichlang_features(caps.get(1)),
                );
            }
            let snippet = directive_re.replace_all(
                caps.get(2).map_or("", |m| m.as_str()),
                |dcaps: &regex::Captures| {
                    process_directives(
                        &self.root,
                        source_path,
                        dcaps.get(0).map_or("", |m| m.as_str()),
                    )
                    .unwrap()
                    .into_iter()
                    .map(|(path, span)| {
                        self.highlight_snippet(path, span)
                            .unwrap_or(String::from(""))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });

            format!(
                "<pre><code class=\"language-hlrs {features}\">{snippet}</code></pre>",
            )
        })
        .to_string()
    }
}

#[derive(Debug)]
pub enum TextAddon<'a> {
    Highlight(&'a HlRange),
    InlayHint(&'a InlayHint),
}

impl<'a> TextAddon<'a> {
    fn range(&self) -> TextRange {
        match self {
            TextAddon::Highlight(h) => h.range,
            TextAddon::InlayHint(h) => h.range,
        }
    }
}

fn ranges_to_html(
    code: &str,
    highlights: &mut [HlRange],
    inlay_hints: &mut Vec<InlayHint>,
) -> String {
    let mut out = String::with_capacity(code.len() * 2);

    let inlay_start = |i: &InlayHint| match i.position {
        InlayHintPosition::After => i.range.end(),
        InlayHintPosition::Before => i.range.start(),
    };

    highlights
        .sort_by(|a, b| a.range.start().cmp(&b.range.start()));
    inlay_hints
        .sort_by(|a, b| inlay_start(a).cmp(&inlay_start(b)));

    let mut addons: Vec<TextAddon> =
        Vec::with_capacity(highlights.len() + inlay_hints.len());

    let mut highlights_iter =
        highlights.iter().filter(|h| !h.highlight.is_empty());
    let mut hints_iter = inlay_hints.iter();
    let mut highlight = highlights_iter.next();
    let mut hint = hints_iter.next();
    loop {
        match (highlight, hint) {
            (Some(h), Some(i)) => {
                match h.range.start().cmp(&inlay_start(i)) {
                    Ordering::Less => {
                        addons.push(TextAddon::Highlight(h));
                        highlight = highlights_iter.next();
                    }
                    Ordering::Greater => {
                        addons.push(TextAddon::InlayHint(i));
                        hint = hints_iter.next();
                    }
                    Ordering::Equal => {
                        addons.push(TextAddon::InlayHint(i));
                        hint = hints_iter.next();
                    }
                }
            }
            (Some(h), None) => {
                addons.push(TextAddon::Highlight(h));
                highlight = highlights_iter.next();
            }
            (None, Some(i)) => {
                addons.push(TextAddon::InlayHint(i));
                hint = hints_iter.next();
            }
            (None, None) => break,
        }
    }

    let mut cursor = 0usize;
    let mut check = false;
    for a in addons {
        let start = usize::from(a.range().start());
        let end = usize::from(a.range().end());
        if cursor < start {
            let mut text = &code[cursor..start];
            if check {
                text = text.trim();
            }

            out.push_str(&html_escape(text));
        }
        check = false;
        match a {
            TextAddon::Highlight(hl) => {
                let class = hl_to_class(hl.highlight);
                let text = html_escape(&code[start..end]);
                if class.is_empty() {
                    out.push_str(&text);
                } else {
                    let mods: String = hl
                        .highlight
                        .mods
                        .iter()
                        .map(|m| format!(" ra-mod-{m}"))
                        .collect();
                    out.push_str(&format!(
                        "<span class=\"{class}{mods}\">{text}</span>"
                    ));
                }
                cursor = end; // advance past the code
            }
            TextAddon::InlayHint(i) => {
                let mut label = i.label.to_string();
                if let InlayKind::Chaining
                | InlayKind::ClosingBrace = i.kind
                {
                    out.push(' ');
                }
                if let InlayKind::Parameter = i.kind {
                    label.push(' ');
                    check = true;
                }
                out.push_str(&format!(
                    "<span class=\"inlay-hint\">{label}</span>"
                ));
            }
        }
    }

    if cursor < code.len() {
        out.push_str(&html_escape(&code[cursor..]));
    }

    out
}

fn hl_to_class(hl: Highlight) -> &'static str {
    match hl.tag {
        HlTag::Keyword => "hlrs-keyword",
        HlTag::BoolLiteral | HlTag::NumericLiteral => {
            "hlrs-litnum"
        }
        HlTag::StringLiteral
        | HlTag::ByteLiteral
        | HlTag::CharLiteral => "hlrs-litstr",
        HlTag::Comment => "hlrs-comment",
        HlTag::EscapeSequence => "hlrs-attribute",
        HlTag::FormatSpecifier => "hlrs-macro",
        HlTag::BuiltinType => "hlrs-type",
        HlTag::UnresolvedReference => "hlrs-variable",

        HlTag::Symbol(sym) => match sym {
            SymbolKind::Function | SymbolKind::Method => {
                "hlrs-function"
            }

            SymbolKind::Struct
            | SymbolKind::Trait
            | SymbolKind::TypeAlias
            | SymbolKind::TypeParam
            | SymbolKind::Module
            | SymbolKind::Enum => "hlrs-type",

            SymbolKind::Variant => "hlrs-enum",

            SymbolKind::Macro => "hlrs-macro",

            SymbolKind::Const
            | SymbolKind::ConstParam
            | SymbolKind::Static
            | SymbolKind::Field
            | SymbolKind::Local
            | SymbolKind::ValueParam => "hlrs-variable",

            SymbolKind::LifetimeParam => "hlrs-lifetime",

            SymbolKind::SelfParam | SymbolKind::SelfType => {
                "hlrs-selftoken"
            }

            SymbolKind::Attribute
            | SymbolKind::BuiltinAttr
            | SymbolKind::Derive => "hlrs-attribute",
            SymbolKind::CrateRoot => "hlrs-type",

            _ => hl.tag.to_string().leak(),
        },

        _ => hl.tag.to_string().leak(),
    }
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}
