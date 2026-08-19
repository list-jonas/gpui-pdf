use gpui::Context;

use crate::EditorView;

impl EditorView {
    pub(super) fn set_highlight_color(&mut self, color: (f64, f64, f64), cx: &mut Context<Self>) {
        self.highlight_color = color;
        self.status = "Highlight color changed".into();
        cx.notify();
    }
}
