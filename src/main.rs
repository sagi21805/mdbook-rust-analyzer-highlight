#![feature(path_absolute_method)]
use mdbook::book::{Book, BookItem};
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor, PreprocessorContext};
use ra_ap_ide::{
    AdjustmentHints, AnalysisHost, GenericParameterHints, Highlight, HighlightConfig, HlRange,
    HlTag, InlayFieldsToResolve, InlayHint, InlayHintPosition, InlayHintsConfig, InlayKind,
    SymbolKind, TextRange,
};
use ra_ap_ide_db::{ChangeWithProcMacros, MiniCore};
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace_at};
use ra_ap_project_model::CargoConfig;
use ra_ap_vfs::{AbsPathBuf, FileId, VfsPath};
use regex::Regex;
use std::cmp::Ordering;
use std::path::Path;
use std::{io, usize};
mod _snippet;

const HLRS_CODEBLOCK_REGEX: &str = r"```rust(?:,?([^\n]+))?\n([\s\S]*?)\n?```";
const RUST_ICON_URL: &str = "@https://www.rust-lang.org/static/images/rust-logo-blk.svg";

static HIGHLIGHT_CONFIG: HighlightConfig = HighlightConfig {
    strings: true,
    punctuation: true,
    specialize_punctuation: true,
    operator: true,
    specialize_operator: true,
    inject_doc_comment: true,
    macro_bang: true,
    syntactic_name_ref_highlighting: true,
    comments: true,
    // When using a real workspace the sysroot is loaded and minicore
    // is not needed.  It's harmless to leave at default in both modes.
    minicore: MiniCore::default(),
};

static INLAY_HINT_CONFIG: InlayHintsConfig = InlayHintsConfig {
    adjustment_hints: AdjustmentHints::Always,
    adjustment_hints_disable_reborrows: false,
    adjustment_hints_hide_outside_unsafe: true,
    adjustment_hints_mode: ra_ap_ide::AdjustmentHintsMode::Prefix,
    binding_mode_hints: false,
    chaining_hints: true,
    closing_brace_hints_min_lines: Some(25),
    closure_capture_hints: false,
    closure_return_type_hints: ra_ap_ide::ClosureReturnTypeHints::Always, // was WithBlock, default is "never"
    closure_style: ra_ap_hir_ty::display::ClosureStyle::RANotation,
    discriminant_hints: ra_ap_ide::DiscriminantHints::Always,
    fields_to_resolve: InlayFieldsToResolve {
        resolve_hint_tooltip: true,
        resolve_label_command: true,
        resolve_label_location: true,
        resolve_label_tooltip: true,
        resolve_text_edits: true,
    },
    generic_parameter_hints: GenericParameterHints {
        type_hints: true,
        lifetime_hints: true,
        const_hints: true,
    },
    hide_closure_initialization_hints: false,
    hide_closure_parameter_hints: false,
    hide_inferred_type_hints: false,
    hide_named_constructor_hints: false,
    implicit_drop_hints: false,
    implied_dyn_trait_hints: true,
    lifetime_elision_hints: ra_ap_ide::LifetimeElisionHints::Never,
    max_length: Some(25),
    minicore: MiniCore::default(),
    param_names_for_lifetime_elision_hints: true,
    parameter_hints: true,
    parameter_hints_for_missing_arguments: true,
    range_exclusive_hints: true,
    render_colons: true,
    sized_bound: true,
    type_hints: true,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let preprocessor = RaHighlight;
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("supports") => {
            let renderer = args.next().unwrap_or_default();
            std::process::exit(if preprocessor.supports_renderer(&renderer) {
                0
            } else {
                1
            });
        }
        _ => {
            let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;
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

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
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
                ch.content = highlighter.as_mut().process_markdown(&ch.content, support);
            }
        });

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

impl RaHighlight {
    fn whichlang_support(&self, ctx: &PreprocessorContext) -> bool {
        if let Some(cfg) = ctx.config.get(&format!("preprocessor.{}", self.name())) {
            match cfg.get("whichlang") {
                Some(feature) => feature
                    .as_bool()
                    .expect("\nERROR: `whichlang` configuration should be a boolean"),
                None => return false,
            };
        }
        false
    }
}

pub struct WorkspaceHighlighter {
    host: AnalysisHost,
    snippet_file_id: FileId,
}

impl WorkspaceHighlighter {
    /// Load the Cargo workspace at `project_root`
    pub fn load(project_root: &str) -> Self {
        let root = Path::new(project_root);
        let sentinel = Path::new(project_root).join("src/_snippet.rs");
        std::fs::write(&sentinel, "// placeholder").unwrap();
        let cargo_toml = root.join("Cargo.toml");

        let load_cfg = LoadCargoConfig {
            load_out_dirs_from_check: true,
            with_proc_macro_server: ProcMacroServerChoice::Sysroot,
            prefill_caches: false,
            num_worker_threads: 4,
            proc_macro_processes: 4,
        };

        let (db, vfs, _proc_macros) = load_workspace_at(
            cargo_toml.as_ref(),
            &CargoConfig {
                sysroot: Some(ra_ap_project_model::RustLibSource::Discover),
                ..Default::default()
            },
            &load_cfg,
            &|msg| eprintln!("[ra] {msg}"),
        )
        .expect("failed to load Cargo workspace");

        let sentinel_vfs = VfsPath::from(AbsPathBuf::assert(
            sentinel
                .absolute()
                .unwrap()
                .try_into()
                .expect("path is not valid UTF-8"),
        ));

        let (snippet_file_id, _) = vfs.file_id(&sentinel_vfs).expect("sentinel must be in VFS");

        Self {
            host: AnalysisHost::with_database(db),
            snippet_file_id,
        }
    }
}

impl WorkspaceHighlighter {
    fn highlight_snippet(&mut self, code: &str) -> String {
        let mut change = ChangeWithProcMacros::default();
        change.change_file(self.snippet_file_id, Some(code.to_string()));
        self.host.apply_change(change);

        let analysis = self.host.analysis();
        let mut highlights = analysis
            .highlight(HIGHLIGHT_CONFIG, self.snippet_file_id)
            .unwrap_or_default();

        let mut inlay_hints = analysis
            .inlay_hints(&INLAY_HINT_CONFIG, self.snippet_file_id, None)
            .unwrap_or_default();

        ranges_to_html(code, &mut highlights, &mut inlay_hints)
    }

    fn extract_whichlang_features<'a>(&self, f: Option<regex::Match<'a>>) -> String {
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

    fn process_markdown(&mut self, content: &str, whichlang_support: bool) -> String {
        let re = Regex::new(HLRS_CODEBLOCK_REGEX).unwrap();

        re.replace_all(content, |caps: &regex::Captures| {
            let mut features = String::from("");
            if whichlang_support {
                features.push_str(&self.extract_whichlang_features(caps.get(0)));
            }
            format!(
                "<pre><code class=\"language-hlrs\" {features} >{}</code></pre>",
                self.highlight_snippet(&caps[1])
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
    eprintln!("HERE");
    let mut out = String::with_capacity(code.len() * 2);
    let mut cursor = 0usize;

    let inlay_start = |i: &InlayHint| match i.position {
        InlayHintPosition::After => i.range.end(),
        InlayHintPosition::Before => i.range.start(),
    };

    highlights.sort_by(|a, b| a.range.start().cmp(&b.range.start()));
    inlay_hints.sort_by(|a, b| inlay_start(a).cmp(&inlay_start(b)));

    let mut addons: Vec<TextAddon> = Vec::with_capacity(highlights.len() + inlay_hints.len());

    let mut highlights_iter = highlights.iter().filter(|h| !h.highlight.is_empty());
    let mut hints_iter = inlay_hints.iter();
    let mut highlight = highlights_iter.next();
    let mut hint = hints_iter.next();
    loop {
        match (highlight, hint) {
            (Some(h), Some(i)) => match h.range.start().cmp(&inlay_start(i)) {
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
                    // }
                    // InlayHintPosition::After => {
                    //     addons.push(TextAddon::Highlight(h));
                    //     highlight = highlights_iter.next()
                    // }
                }
            },
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

    for a in addons {
        eprintln!("{:#?}", a);

        let start = usize::from(a.range().start());
        let end = usize::from(a.range().end());
        if cursor < start {
            out.push_str(&html_escape(&code[cursor..start]));
        }
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
                    out.push_str(&format!("<span class=\"{class}{mods}\">{text}</span>"));
                }
                cursor = end; // advance past the code
            }
            TextAddon::InlayHint(i) => {
                let mut label = i.label.to_string();
                if let InlayKind::Chaining | InlayKind::ClosingBrace = i.kind {
                    out.push(' ');
                }
                if let InlayKind::Parameter = i.kind {
                    label.push(' ');
                }
                out.push_str(&format!("<span class=\"inlay-hint\">{label}</span>"));
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
        HlTag::BoolLiteral | HlTag::NumericLiteral => "hlrs-litnum",
        HlTag::StringLiteral | HlTag::ByteLiteral | HlTag::CharLiteral => "hlrs-litstr",
        HlTag::Comment => "hlrs-comment",
        HlTag::EscapeSequence => "hlrs-attribute",
        HlTag::FormatSpecifier => "hlrs-macro",
        HlTag::BuiltinType => "hlrs-type",
        HlTag::UnresolvedReference => "hlrs-variable",

        HlTag::Symbol(sym) => match sym {
            SymbolKind::Function | SymbolKind::Method => "hlrs-function",

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

            SymbolKind::SelfParam | SymbolKind::SelfType => "hlrs-selftoken",

            SymbolKind::Attribute | SymbolKind::BuiltinAttr | SymbolKind::Derive => {
                "hlrs-attribute"
            }
            SymbolKind::CrateRoot => "hlrs-type",

            _ => hl.tag.to_string().leak(),
        },

        _ => hl.tag.to_string().leak(),
    }
}

// ── HTML escaping ─────────────────────────────────────────────────────────────

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
