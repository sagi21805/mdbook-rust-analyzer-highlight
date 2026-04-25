use ra_ap_ide::HighlightConfig;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HighlightConfigDe {
    pub strings: bool,
    pub punctuation: bool,
    pub specialize_punctuation: bool,
    pub operator: bool,
    pub specialize_operator: bool,
    pub inject_doc_comment: bool,
    pub macro_bang: bool,
    pub syntactic_name_ref_highlighting: bool,
    pub comments: bool,
}

impl<'a> From<HighlightConfigDe> for HighlightConfig<'a> {
    fn from(c: HighlightConfigDe) -> Self {
        HighlightConfig {
            strings: c.strings,
            punctuation: c.punctuation,
            specialize_punctuation: c.specialize_punctuation,
            operator: c.operator,
            specialize_operator: c.specialize_operator,
            inject_doc_comment: c.inject_doc_comment,
            macro_bang: c.macro_bang,
            syntactic_name_ref_highlighting: c
                .syntactic_name_ref_highlighting,
            comments: c.comments,
            minicore: Default::default(),
        }
    }
}

impl Default for HighlightConfigDe {
    fn default() -> Self {
        HighlightConfigDe {
            strings: true,
            punctuation: true,
            specialize_punctuation: true,
            operator: true,
            specialize_operator: true,
            inject_doc_comment: true,
            macro_bang: true,
            syntactic_name_ref_highlighting: true,
            comments: true,
        }
    }
}

impl HighlightConfigDe {
    pub fn from_file(path: &str) -> HighlightConfigDe {
        if path.is_empty() {
            return HighlightConfigDe::default();
        }

        let f = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => {
                    eprintln!(
                        "[ INFO ]: File: {path} not found, using default configuration"
                    );
                    String::from("")
                }
                _ => panic!("{}", e),
            },
        };

        serde_json::from_str(&f).unwrap_or_else(|e| {
            eprintln!("[ INFO ]: Configuration content is invalid\n {e} \nusing default configuration");
            HighlightConfigDe::default()
        })
    }
}
