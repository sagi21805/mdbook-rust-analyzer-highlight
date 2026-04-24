#[derive(Default)]
pub struct WhichlangFeatures {
    fp: Option<String>,
    icon: Option<Icon>,
    banner: Option<()>,
}

pub enum Icon {
    Rust,
    Asm,
    Other { url: String },
}

impl Icon {
    fn url(&self) -> &str {
        match self {
            Icon::Rust => {
                "https://www.rust-lang.org/static/images/rust-logo-blk.svg"
            }
            Icon::Asm => {
                "https://icons.veryicon.com/png/o/business/vscode-program-item-icon/assembly-7.png"
            }
            Icon::Other { url } => url,
        }
    }
}

impl ToString for WhichlangFeatures {
    fn to_string(&self) -> String {
        if let Some(_) = self.banner {
            return String::from("banner=no");
        }
        format!(
            "{}{}",
            self.fp.as_deref().map(|fp| fp).unwrap_or(""),
            self.icon
                .as_ref()
                .map(|i| format!(",icon=@{}", i.url()))
                .unwrap_or(String::from(""))
        )
    }
}

impl From<&str> for WhichlangFeatures {
    fn from(value: &str) -> Self {
        let mut default = Self::default();
        let parts = value.split(',');

        for part in parts {
            let mut kv = part.splitn(2, '=');
            let key = kv.next().expect("msg");
            let value = kv.next().expect("msg");

            match key {
                "fp" => default.fp = Some(value.to_string()),
                "icon" => {
                    default.icon = Some(Icon::Other {
                        url: value.to_string(),
                    })
                }
                "banner" => default.banner = Some(()),
                _ => {
                    eprintln!(
                        "[ INFO ]: unknown key: {} (ignoreing)",
                        key
                    )
                }
            }
        }

        default
    }
}
