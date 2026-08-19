use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};
use gpui_component::IconNamed;

const ASSETS: &[(&str, &[u8])] = &[
    (
        "hugeicons/select.svg",
        include_bytes!("../assets/hugeicons/select.svg"),
    ),
    (
        "hugeicons/hand.svg",
        include_bytes!("../assets/hugeicons/hand.svg"),
    ),
    (
        "hugeicons/annotate.svg",
        include_bytes!("../assets/hugeicons/annotate.svg"),
    ),
    (
        "hugeicons/edit.svg",
        include_bytes!("../assets/hugeicons/edit.svg"),
    ),
    (
        "hugeicons/text.svg",
        include_bytes!("../assets/hugeicons/text.svg"),
    ),
    (
        "hugeicons/image.svg",
        include_bytes!("../assets/hugeicons/image.svg"),
    ),
    (
        "hugeicons/link.svg",
        include_bytes!("../assets/hugeicons/link.svg"),
    ),
    (
        "hugeicons/highlight.svg",
        include_bytes!("../assets/hugeicons/highlight.svg"),
    ),
    (
        "hugeicons/underline.svg",
        include_bytes!("../assets/hugeicons/underline.svg"),
    ),
    (
        "hugeicons/strike.svg",
        include_bytes!("../assets/hugeicons/strike.svg"),
    ),
    (
        "hugeicons/redact.svg",
        include_bytes!("../assets/hugeicons/redact.svg"),
    ),
    (
        "hugeicons/shapes.svg",
        include_bytes!("../assets/hugeicons/shapes.svg"),
    ),
    (
        "hugeicons/sign.svg",
        include_bytes!("../assets/hugeicons/sign.svg"),
    ),
    (
        "hugeicons/more.svg",
        include_bytes!("../assets/hugeicons/more.svg"),
    ),
    (
        "hugeicons/search.svg",
        include_bytes!("../assets/hugeicons/search.svg"),
    ),
    (
        "hugeicons/open.svg",
        include_bytes!("../assets/hugeicons/open.svg"),
    ),
    (
        "hugeicons/share.svg",
        include_bytes!("../assets/hugeicons/share.svg"),
    ),
    (
        "hugeicons/previous.svg",
        include_bytes!("../assets/hugeicons/previous.svg"),
    ),
    (
        "hugeicons/next.svg",
        include_bytes!("../assets/hugeicons/next.svg"),
    ),
    (
        "hugeicons/zoom-in.svg",
        include_bytes!("../assets/hugeicons/zoom-in.svg"),
    ),
    (
        "hugeicons/zoom-out.svg",
        include_bytes!("../assets/hugeicons/zoom-out.svg"),
    ),
    (
        "hugeicons/fit.svg",
        include_bytes!("../assets/hugeicons/fit.svg"),
    ),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ASSETS
            .iter()
            .find_map(|(asset_path, bytes)| (*asset_path == path).then_some(Cow::Borrowed(*bytes))))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ASSETS
            .iter()
            .filter(|(asset_path, _)| asset_path.starts_with(path))
            .map(|(asset_path, _)| SharedString::from(*asset_path))
            .collect())
    }
}

#[derive(Clone, Copy)]
pub(crate) enum HugeIcon {
    Select,
    Hand,
    Annotate,
    Edit,
    Text,
    Image,
    Link,
    Highlight,
    Underline,
    Strike,
    Redact,
    Shapes,
    Sign,
    More,
    Search,
    Open,
    Share,
    Previous,
    Next,
    ZoomIn,
    ZoomOut,
    Fit,
}

impl IconNamed for HugeIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Select => "hugeicons/select.svg",
            Self::Hand => "hugeicons/hand.svg",
            Self::Annotate => "hugeicons/annotate.svg",
            Self::Edit => "hugeicons/edit.svg",
            Self::Text => "hugeicons/text.svg",
            Self::Image => "hugeicons/image.svg",
            Self::Link => "hugeicons/link.svg",
            Self::Highlight => "hugeicons/highlight.svg",
            Self::Underline => "hugeicons/underline.svg",
            Self::Strike => "hugeicons/strike.svg",
            Self::Redact => "hugeicons/redact.svg",
            Self::Shapes => "hugeicons/shapes.svg",
            Self::Sign => "hugeicons/sign.svg",
            Self::More => "hugeicons/more.svg",
            Self::Search => "hugeicons/search.svg",
            Self::Open => "hugeicons/open.svg",
            Self::Share => "hugeicons/share.svg",
            Self::Previous => "hugeicons/previous.svg",
            Self::Next => "hugeicons/next.svg",
            Self::ZoomIn => "hugeicons/zoom-in.svg",
            Self::ZoomOut => "hugeicons/zoom-out.svg",
            Self::Fit => "hugeicons/fit.svg",
        }
        .into()
    }
}
