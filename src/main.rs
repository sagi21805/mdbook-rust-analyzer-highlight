#![feature(path_absolute_method)]
use mdbook::book::{Book, BookItem};
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor, PreprocessorContext};
use ra_ap_ide::{Analysis, AnalysisHost, Highlight, HighlightConfig, HlRange, HlTag, SymbolKind};
use ra_ap_ide_db::{ChangeWithProcMacros, MiniCore};
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace_at};
use ra_ap_project_model::CargoConfig;
use ra_ap_vfs::{AbsPathBuf, Change, FileId, Vfs, VfsPath};
use regex::Regex;
use std::arch::asm;
use std::fmt::Debug;
use std::io;
use std::path::Path;
mod _snippet;

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

// ── Preprocessor ─────────────────────────────────────────────────────────────

struct RaHighlight;

impl Preprocessor for RaHighlight {
    fn name(&self) -> &str {
        "mdbook-rust-analyzer-highlight"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        // Read project-root from book.toml:
        //   [preprocessor.ra-highlight]
        //   project-root = "/absolute/path/to/your/crate"
        let project_root = ctx
            .config
            .get_preprocessor(self.name())
            .and_then(|t| t.get("project-root"))
            .and_then(|v| v.as_str());

        // If project-root is configured, load the full workspace once.
        // Otherwise fall back to single-file mode (no macro expansion).
        let mut highlighter: Box<dyn Highlighter> = match project_root {
            Some(root) => {
                eprintln!("[ra-highlight] Loading workspace at {root} …");
                Box::new(WorkspaceHighlighter::load(root))
            }
            None => {
                eprintln!("[ra-highlight] No project-root set, using single-file mode");
                Box::new(SingleFileHighlighter)
            }
        };

        book.for_each_mut(|item| {
            if let BookItem::Chapter(ch) = item {
                ch.content = process_markdown(&ch.content, highlighter.as_mut());
            }
        });

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

// ── Highlighter trait — lets us swap single-file vs workspace ─────────────────

trait Highlighter {
    fn highlight_snippet(&mut self, code: &str) -> String;
}

// ── Single-file fallback (original behaviour) ─────────────────────────────────

struct SingleFileHighlighter;

impl Highlighter for SingleFileHighlighter {
    fn highlight_snippet(&mut self, code: &str) -> String {
        let helper_str = std::fs::read_to_string("helper.rs").unwrap_or_else(|e| {
            eprintln!("[ra-highlight] No helper file: {e}");
            String::new()
        });

        let mut extended_code = code.to_string();
        extended_code.push_str(&helper_str);

        let (analysis, file_id) = Analysis::from_single_file(extended_code);
        let highlights = analysis
            .highlight(HIGHLIGHT_CONFIG, file_id)
            .unwrap_or_default();

        ranges_to_html(code, &highlights)
    }
}

// ── Full workspace highlighter ────────────────────────────────────────────────

pub struct WorkspaceHighlighter {
    host: AnalysisHost,
    vfs: Vfs,
    snippet_file_id: FileId, // FileId of the sentinel file
}

impl WorkspaceHighlighter {
    /// Load the Cargo workspace at `project_root`
    pub fn load(project_root: &str) -> Self {
        let root = Path::new(project_root);
        let sentinel = Path::new(project_root).join("src/_snippet.rs");
        std::fs::write(&sentinel, "// placeholder").unwrap();
        let cargo_toml = root.join("Cargo.toml");

        let load_cfg = LoadCargoConfig {
            // Runs `cargo check` so build-script out-dirs are known.
            // Set to false to skip build scripts and speed up loading.
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

        // for f in vfs.iter() {
        //     eprintln!("{:?}", f);
        // }

        eprintln!("SENTINAL PATH: {}", sentinel_vfs);

        let (snippet_file_id, _) = vfs.file_id(&sentinel_vfs).expect("sentinel must be in VFS");

        eprintln!("FILE ID: {:?}", snippet_file_id);

        Self {
            host: AnalysisHost::with_database(db),
            vfs,
            snippet_file_id,
        }
    }
}

impl Highlighter for WorkspaceHighlighter {
    fn highlight_snippet(&mut self, code: &str) -> String {
        let mut change = ChangeWithProcMacros::default();
        change.change_file(self.snippet_file_id, Some(code.to_string()));
        self.host.apply_change(change);

        let analysis = self.host.analysis();
        let highlights = analysis
            .highlight(HIGHLIGHT_CONFIG, self.snippet_file_id)
            .unwrap_or_default();

        ranges_to_html(code, &highlights)
    }
}

fn process_markdown(content: &str, hl: &mut dyn Highlighter) -> String {
    let re = Regex::new(r"(?ms)^```rust[^\n]*\n(.*?)^```[ \t]*$").unwrap();
    re.replace_all(content, |caps: &regex::Captures| {
        format!(
            "<pre><code class=\"language-hlrs\">{}</code></pre>",
            hl.highlight_snippet(&caps[1])
        )
    })
    .into_owned()
}

// ── Splice <span> tags at RA's byte ranges ────────────────────────────────────

fn ranges_to_html(code: &str, highlights: &[HlRange]) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    let mut cursor = 0usize;

    for hl in highlights {
        let start = usize::from(hl.range.start());
        let end = usize::from(hl.range.end());

        if start >= code.len() || end > code.len() {
            continue;
        }

        // Emit any unhighlighted gap before this range.
        if cursor < start {
            out.push_str(&html_escape(&code[cursor..start]));
        }

        let class = hl_to_class(hl.highlight);
        let text = html_escape(&code[start..end]);

        if class.is_empty() {
            out.push_str(&text);
        } else {
            // Append modifier classes, e.g. "ra-mod-mutable", "ra-mod-consuming".
            let mods: String = hl
                .highlight
                .mods
                .iter()
                .map(|m| format!(" ra-mod-{m}"))
                .collect();
            out.push_str(&format!("<span class=\"{class}{mods}\">{text}</span>"));
        }

        cursor = end;
    }

    // Emit any trailing text after the last highlight range.
    if cursor < code.len() {
        out.push_str(&html_escape(&code[cursor..]));
    }

    out
}

// ── RA semantic tag → CSS class ───────────────────────────────────────────────

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
