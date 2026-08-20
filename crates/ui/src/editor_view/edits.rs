use gpui::Context;

use crate::EditorView;

impl EditorView {
    pub(super) fn set_highlight_color(&mut self, color: (f64, f64, f64), cx: &mut Context<Self>) {
        self.highlight_color = color;
        self.status = "Highlight color changed".into();
        cx.notify();
    }

    pub(super) fn set_annotation_color(&mut self, color: (f64, f64, f64), cx: &mut Context<Self>) {
        self.annotation_color = color;
        self.status = "Annotation color changed".into();
        cx.notify();
    }

    pub(super) fn set_shape_kind(&mut self, kind: pdf_engine::ShapeKind, cx: &mut Context<Self>) {
        self.shape_kind = kind;
        self.status = format!("{} shape selected", super::model::shape_label(kind)).into();
        cx.notify();
    }
}
