# Chapter 1
```rust
use mdbook::book::{Book, BookItem};
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor, PreprocessorContext};
use ra_ap_ide::{Analysis, Highlight, HighlightConfig, HlRange, HlTag, SymbolKind};
use regex::Regex;
use std::io;

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
        "ra-highlight"
    }

    fn run(&self, _ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        book.for_each_mut(|item| {
            if let BookItem::Chapter(ch) = item {
                ch.content = process_markdown(&ch.content);
            }
        });
        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}


fn process_markdown(content: &str) -> String {
    let re = Regex::new(r"(?ms)^```rust[^\n]*\n(.*?)^```[ \t]*$").unwrap();
    re.replace_all(content, |caps: &regex::Captures| {
        format!(
            "<pre><code class=\"language-hlrs\">{}</code></pre>",
            highlight_to_html(&caps[1])
        )
    })
    .into_owned()
}


fn highlight_to_html(code: &str) -> String {
    let (analysis, file_id) = Analysis::from_single_file(code.to_string());

    let config = HighlightConfig {
        strings: true,
        punctuation: true,
        specialize_punctuation: true,
        operator: true,
        specialize_operator: true,
        inject_doc_comment: true,
        macro_bang: true,
        syntactic_name_ref_highlighting: true,
        comments: true,
        minicore: Default::default(),
    };

    let highlights = analysis.highlight(config, file_id).unwrap_or_default();

    ranges_to_html(code, &highlights)
}


fn ranges_to_html(code: &str, highlights: &[HlRange]) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    let mut cursor = 0usize;

    for hl in highlights {
        let start = usize::from(hl.range.start());
        let end = usize::from(hl.range.end());

        if cursor < start {
            out.push_str(&html_escape(&code[cursor..start]));
        }

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

        cursor = end;
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
        HlTag::EscapeSequence => "hlrs-attribute", // Matches neutral gray-white
        HlTag::FormatSpecifier => "hlrs-macro",    // Matches orange/tan
        HlTag::BuiltinType => "hlrs-type",
        HlTag::UnresolvedReference => "hlrs-variable", // Highlighting red marks it as "needs attention"

        HlTag::Symbol(sym) => match sym {
            SymbolKind::Function | SymbolKind::Method => "hlrs-function",
            SymbolKind::Struct
            | SymbolKind::Trait
            | SymbolKind::TypeAlias
            | SymbolKind::TypeParam
            | SymbolKind::Module => "hlrs-type",

            SymbolKind::Enum | SymbolKind::Variant => "hlrs-enum",

            SymbolKind::Macro => "hlrs-macro",

            SymbolKind::Const
            | SymbolKind::ConstParam
            | SymbolKind::Static
            | SymbolKind::Field
            | SymbolKind::Local
            | SymbolKind::ValueParam => "hlrs-variable",

            SymbolKind::LifetimeParam => "hlrs-lifetime",

            // Corrected to match .hlrs-selftoken
            SymbolKind::SelfParam | SymbolKind::SelfType => "hlrs-selftoken",

            // Corrected to match .hlrs-attribute
            SymbolKind::Attribute | SymbolKind::BuiltinAttr | SymbolKind::Derive => {
                "hlrs-attribute"
            }

            _ => "",
        },
        Test::A => ""

        _ => "",
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

pub enum Test {
    A,
    B
}

```
ANOTHER SNIPPET 


```rust
fn highlight_to_html(code: &str) -> String {
    let (analysis, file_id) = Analysis::from_single_file(code.to_string());

    let config = HighlightConfig {
        strings: true,
        punctuation: true,
        specialize_punctuation: true,
        operator: true,
        specialize_operator: true,
        inject_doc_comment: true,
        macro_bang: true,
        syntactic_name_ref_highlighting: true,
        comments: true,
        minicore: Default::default(),
    };

    let highlights = analysis.highlight(config, file_id).unwrap_or_default();

    ranges_to_html(code, &highlights)
}

```

```rust
static GLOBAL_DESCRIPTOR_TABLE_LONG_MODE: GlobalDescriptorTableLong =
    GlobalDescriptorTableLong::default();

use std::arch::asm;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".start")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn second_stage() -> ! {
    // Set data segment register
    asm!("mov eax, 0x10", "mov ds, eax",);
    // Enable paging and load page tables with an identity
    // mapping
    #[cfg(target_arch = "x86")]
    cpu_utils::structures::paging::enable();
    // Load the global descriptor table for long mode
    GLOBAL_DESCRIPTOR_TABLE_LONG_MODE.load();
    // Update global descriptor table to enable long mode
    // and jump to kernel code
    asm!(
        "ljmp ${section}, ${next_stage}",
        section = const Sections::KernelCode as u8,
        next_stage = const KERNEL_OFFSET,
        options(att_syntax)
    );

    #[allow(clippy::all)]
    loop {}
}

```
