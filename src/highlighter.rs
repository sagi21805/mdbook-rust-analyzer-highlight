use std::{
    cmp::Ordering,
    collections::HashMap,
    path::{Path, PathBuf},
};

use mdbook_include_rs::parser::{Lines, process_directives};
use mdbook_preprocessor::config::Playground;
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
        spans: Vec<Lines>,
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

        if spans.is_empty() {
            out.push_str(highlighted);
        } else {
            for s in spans {
                out.push_str(
                    &highlighted
                        .lines()
                        .skip(s.start - 1)
                        .take(s.end - s.start + 1)
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
                out.push_str("\n");
            }
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

            eprintln!("CODE: \n{}", code);

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
    ) -> WhichlangFeatures {
        let mut features = WhichlangFeatures::from(
            f.map(|m| m.as_str()).unwrap_or_default(),
        );

        if features.icon.is_none() {
            features.icon = Some(Icon::Rust);
        }

        features
    }

    pub fn process_markdown(
        &mut self,
        source_path: &Path,
        content: &str,
    ) -> String {
        let re = Regex::new(HLRS_CODEBLOCK_REGEX).unwrap();

        re.replace_all(content, |caps: &regex::Captures| {
            let mut features: WhichlangFeatures = WhichlangFeatures::default();
            if self.config.whichlang_support {
                features = self.extract_whichlang_features(caps.get(1));
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

            let escaped = snippet.trim_end().replace('\n', "&#10;");
            let playground = if features.playground.is_some() { "playground" } else { "" };
            let features = features.to_string();

            format!(
                "\n\n<pre class=\"{playground}\"><code class=\"language-rust {features}\">{escaped}</code></pre>\n\n",
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
    const NUMBER_OF_CONSECUTIVE_COMMENTS: usize = 7;

    let mut out = String::with_capacity(code.len() * 2);
    let inlay_start = |i: &InlayHint| match i.position {
        InlayHintPosition::After => i.range.end(),
        InlayHintPosition::Before => i.range.start(),
    };
    highlights
        .sort_by(|a, b| a.range.start().cmp(&b.range.start()));
    inlay_hints
        .sort_by(|a, b| inlay_start(a).cmp(&inlay_start(b)));

    let comment_indices: Vec<usize> = highlights
        .iter()
        .enumerate()
        .filter(|(_, h)| {
            !h.highlight.is_empty()
                && matches!(h.highlight.tag, HlTag::Comment)
        })
        .map(|(i, _)| i)
        .collect();

    let line_of = |h: &HlRange| -> usize {
        code[..usize::from(h.range.start())]
            .bytes()
            .filter(|&b| b == b'\n')
            .count()
    };

    let mut boring_lines: std::collections::HashSet<usize> =
        std::collections::HashSet::new();
    let is_transparent =
        |h: &HlRange| matches!(h.highlight.tag, HlTag::None);
    let share_line = |a: &HlRange, b: &HlRange| -> bool {
        let (start, end) = if a.range.end() <= b.range.start() {
            (
                usize::from(a.range.end()),
                usize::from(b.range.start()),
            )
        } else if b.range.end() <= a.range.start() {
            (
                usize::from(b.range.end()),
                usize::from(a.range.start()),
            )
        } else {
            return true;
        };
        !code[start..end].contains('\n')
    };
    let mut last_boring_lines: std::collections::HashSet<usize> =
        std::collections::HashSet::new();
    let mut run_start = 0;
    while run_start < comment_indices.len() {
        let mut run_end = run_start;
        while run_end + 1 < comment_indices.len() {
            let gap_start = comment_indices[run_end] + 1;
            let gap_end = comment_indices[run_end + 1];
            let left_comment =
                &highlights[comment_indices[run_end]];
            let right_comment =
                &highlights[comment_indices[run_end + 1]];
            if highlights[gap_start..gap_end].iter().all(|h| {
                is_transparent(h)
                    || share_line(h, left_comment)
                    || share_line(h, right_comment)
            }) {
                run_end += 1;
            } else {
                break;
            }
        }
        if run_end - run_start + 1
            >= NUMBER_OF_CONSECUTIVE_COMMENTS
        {
            for idx in &comment_indices[run_start..=run_end] {
                let comment = &highlights[*idx];
                boring_lines.insert(line_of(comment));
                for h in highlights.iter() {
                    if share_line(h, comment) {
                        boring_lines.insert(line_of(h));
                    }
                }
            }
            last_boring_lines.insert(line_of(
                &highlights[comment_indices[run_end]],
            ));
        }
        run_start = run_end + 1;
    }

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
                    // let mods: String = hl
                    //     .highlight
                    //     .mods
                    //     .iter()
                    //     .map(|m| format!(" ra-mod-{m}"))
                    //     .collect();

                    let mods = "";

                    let trimmed = text.trim_end();
                    let removed = &text[trimmed.len()..];
                    // Append "ra-boring" when this comment is
                    // part of a bulk of 3+.

                    out.push_str(&format!(
                        "<span class=\"{class}{mods}\">{trimmed}</span>{removed}"
                    ));
                }
                cursor = end;
            }
            TextAddon::InlayHint(i) => {
                let mut label =
                    html_escape(&i.label.to_string());
                if let InlayKind::Chaining
                | InlayKind::ClosingBrace = i.kind
                {
                    out.push(' ');
                }
                if let InlayKind::Parameter = i.kind {
                    label.push(' ');
                    check = true;
                }
                let trimmed = label.trim_end();
                let removed = &label[trimmed.len()..];
                out.push_str(&format!(
                    "<span class=\"inlay-hint\">{trimmed}</span>{removed}"
                ));
            }
        }
    }
    if cursor < code.len() {
        out.push_str(&html_escape(&code[cursor..]));
    }
    out
    // let mut with_boring = String::with_capacity(out.len());
    // for (i, line) in out.lines().enumerate() {
    //     if boring_lines.contains(&(i + 1)) {
    //         with_boring.push_str(&format!(
    //             "<span class=\"boring\">{line}\n</span>"
    //         ));
    //     } else {
    //         with_boring.push_str(line);
    //         with_boring.push('\n');
    //     }
    // }

    // with_boring
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
