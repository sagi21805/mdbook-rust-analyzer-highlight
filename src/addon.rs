use ra_ap_ide::{HlRange, InlayHint, TextRange};

#[derive(Debug)]
pub enum TextAddon<'a> {
    Highlight(&'a HlRange),
    InlayHint(&'a InlayHint),
}

impl<'a> TextAddon<'a> {
    pub fn range(&self) -> TextRange {
        match self {
            TextAddon::Highlight(h) => h.range,
            TextAddon::InlayHint(h) => h.range,
        }
    }
}
