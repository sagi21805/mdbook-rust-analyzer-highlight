#[derive(Default)]
pub struct WhichlangFeatures {
    pub fp: Option<String>,
    pub icon: Option<Icon>,
    pub banner: Option<()>,
}

pub enum Icon {
    Rust,
    Other { url: String },
}

impl Icon {
    fn url(&self) -> &str {
        match self {
            Icon::Rust => {
                "https://www.rust-lang.org/static/images/rust-logo-blk.svg"
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
            self.fp
                .as_deref()
                .map(|fp| format!("fp={}", fp))
                .unwrap_or(String::from("")),
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
        if value.is_empty() {
            return default;
        }

        let parts = value.split(',');

        for part in parts {
            let mut kv = part.splitn(2, '=');

            let key = kv.next().unwrap_or_else(|| {
                panic!("[ ERROR ]: missing key in '{}'", value);
            });
            let value = kv.next().unwrap_or_else(|| {
                panic!(
                    "[ ERROR ]: missing value in '{}'",
                    value
                );
            });

            match key.trim() {
                "fp" => default.fp = Some(value.to_string()),
                "icon" => {
                    default.icon = Some(Icon::Other {
                        url: value
                            .strip_prefix("@")
                            .unwrap_or(value)
                            .to_string(),
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
