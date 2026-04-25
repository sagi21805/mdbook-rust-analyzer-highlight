use std::{
    cmp::Ordering,
    collections::HashMap,
    path::{Path, PathBuf},
};

use mdbook_include_rs::parser::process_directives;
use proc_macro2::Span;
use ra_ap_ide::{
    AnalysisHost, Highlight, HlRange, HlTag, InlayHint,
    InlayHintPosition, InlayKind, SymbolKind,
};
use ra_ap_ide_db::base_db::SourceDatabase;
use ra_ap_load_cargo::{
    LoadCargoConfig, ProcMacroServerChoice, load_workspace_at,
};
use ra_ap_project_model::CargoConfig;
use ra_ap_vfs::{AbsPathBuf, FileId, Vfs, VfsPath};
use regex::Regex;

use crate::{
    addon::TextAddon,
    config::Config,
    whichlang::{Icon, WhichlangFeatures},
};

const HLRS_CODEBLOCK_REGEX: &str =
    r"```rust(?:,?([^\n]+))?\n([\s\S]*?)\n?```";

pub struct RustAnalyzerHighlighter<'a> {
    config: &'a Config<'a>,
    host: AnalysisHost,
    vfs: Vfs,
    /// Cache of highlighted snippets
    hl_cache: HashMap<FileId, String>,
}

impl<'a> RustAnalyzerHighlighter<'a> {
    pub fn new(config: &'a Config<'a>) -> Self {
        let root = &config.project_root;
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
            &|msg| eprintln!("{msg}"),
        )
        .expect("failed to load Cargo workspace");

        Self {
            config,
            host: AnalysisHost::with_database(db),
            vfs,
            hl_cache: HashMap::new(),
        }
    }

    fn get_file_span(
        &mut self,
        file_path: PathBuf,
        spans: Vec<Span>,
    ) -> Option<String> {
        let vfs_path = VfsPath::from(AbsPathBuf::assert(
            file_path
                .absolute()
                .unwrap()
                .try_into()
                .expect("Path is not a valid UTF-8"),
        ));

        let (file_id, _excluded) =
            self.vfs.file_id(&vfs_path)?;

        let highlighted = self.highlight_file(file_id);

        let mut out = String::new();

        for s in spans {
            out.push_str(
                &highlighted
                    .lines()
                    .skip(s.start().line - 1)
                    .take(s.end().line - s.start().line + 1)
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            out.push_str("\n");
        }

        Some(out)
    }

    fn highlight_file(&mut self, file_id: FileId) -> &String {
        if !self.hl_cache.contains_key(&file_id) {
            let analysis = self.host.analysis();
            let mut highlights = analysis
                .highlight(self.config.highlight_config, file_id)
                .unwrap_or_default();
            let mut inlay_hints = analysis
                .inlay_hints(
                    &self.config.inlay_hint_config,
                    file_id,
                    None,
                )
                .unwrap_or_default();
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
        }

        self.hl_cache.get(&file_id).unwrap()
    }

    fn extract_whichlang_features<'b>(
        &self,
        f: Option<regex::Match<'b>>,
    ) -> String {
        let mut features = WhichlangFeatures::from(
            f.map(|m| m.as_str()).unwrap_or_default(),
        );

        if features.icon.is_none() {
            features.icon = Some(Icon::Rust);
        }

        features.to_string()
    }

    pub fn process_markdown(
        &mut self,
        source_path: &Path,
        content: &str,
    ) -> String {
        let re = Regex::new(HLRS_CODEBLOCK_REGEX).unwrap();

        re.replace_all(content, |caps: &regex::Captures| {
            let mut features = String::from("");
            if self.config.whichlang_support {
                features
                    .push_str(&self.extract_whichlang_features(
                        caps.get(1),
                    ));
            }

            let cap_content = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let snippet = process_directives(
                &self.config.project_root,
                source_path,
                cap_content,
            )
            .unwrap()
            .into_iter()
            .map(|(path, span)| {
                self.get_file_span(path, span)
                    .unwrap_or(String::from(""))
            })
            .collect::<Vec<_>>()
            .join("\n");

            format!(
                "<pre><code class=\"language-hlrs {features}\">{snippet}</code></pre>",
            )
        })
        .to_string()
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
