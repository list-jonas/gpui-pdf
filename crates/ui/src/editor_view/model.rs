use document_core::{PdfPoint, PdfRect};
use gpui::{Pixels, Point};
use gpui_component::input::InputState;

use gpui::Entity;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Tool {
    #[default]
    Select,
    Hand,
    Highlight,
    AddText,
    Redact,
}

impl Tool {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Hand => "Hand",
            Self::Highlight => "Highlight",
            Self::AddText => "Add text",
            Self::Redact => "Redact",
        }
    }
}

pub enum DragState {
    Region {
        page_index: usize,
        start: PdfPoint,
        current: PdfPoint,
    },
    Pan {
        start: Point<Pixels>,
        offset: Point<Pixels>,
    },
    InlineText {
        page_index: usize,
        start: PdfPoint,
        point: PdfPoint,
    },
}

impl DragState {
    pub fn rect(&self) -> Option<PdfRect> {
        let Self::Region { start, current, .. } = self else {
            return None;
        };
        PdfRect::new(
            start.x.min(current.x),
            start.y.min(current.y),
            start.x.max(current.x),
            start.y.max(current.y),
        )
        .ok()
    }
}

pub struct InlineText {
    pub page_index: usize,
    pub point: PdfPoint,
    pub input: Entity<InputState>,
}

#[derive(Clone, Copy)]
pub struct SearchMatch {
    pub page_index: usize,
    pub rect: PdfRect,
}

#[derive(Clone, Copy)]
pub struct OverlayRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}
