use document_core::PdfRect;
use gpui::{App, Context, Entity};
use gpui_component::input::InputState;
use pdf_engine::{EditCommand, TextStamp};

use crate::EditorView;

impl EditorView {
    pub(super) fn add_text(&mut self, cx: &mut Context<Self>) {
        let result = (|| {
            let text = value(&self.add_text, cx);
            if text.trim().is_empty() {
                return Err("Enter text first".to_owned());
            }
            Ok(EditCommand::AddText(TextStamp {
                page_index: self.page_index,
                text,
                x: number(&self.text_x, cx, "X")?,
                y: number(&self.text_y, cx, "Y")?,
                size: positive_number(&self.text_size, cx, "font size")?,
            }))
        })();
        self.push_edit(result, "Text queued; use Save As to write it", cx);
    }

    pub(super) fn add_redaction(&mut self, cx: &mut Context<Self>) {
        let result = (|| {
            let rect = PdfRect::new(
                number(&self.redact_x0, cx, "left")?,
                number(&self.redact_y0, cx, "bottom")?,
                number(&self.redact_x1, cx, "right")?,
                number(&self.redact_y1, cx, "top")?,
            )
            .map_err(|error| error.to_string())?;
            Ok(EditCommand::Redact {
                page_index: self.page_index,
                rect,
            })
        })();
        self.push_edit(
            result,
            "Redaction queued; Save As rewrites active page content",
            cx,
        );
    }

    fn push_edit(
        &mut self,
        edit: Result<EditCommand, String>,
        message: &'static str,
        cx: &mut Context<Self>,
    ) {
        match edit {
            Ok(edit) => {
                self.edits.push(edit);
                self.status = message.into();
            }
            Err(error) => self.status = error.into(),
        }
        cx.notify();
    }
}

fn value(input: &Entity<InputState>, cx: &App) -> String {
    input.read(cx).value().to_string()
}

fn number(input: &Entity<InputState>, cx: &App, label: &str) -> Result<f64, String> {
    value(input, cx)
        .parse::<f64>()
        .map_err(|_| format!("{label} must be a number"))
        .and_then(|number| {
            number
                .is_finite()
                .then_some(number)
                .ok_or_else(|| format!("{label} must be finite"))
        })
}

fn positive_number(input: &Entity<InputState>, cx: &App, label: &str) -> Result<f64, String> {
    let number = number(input, cx, label)?;
    (number > 0.0)
        .then_some(number)
        .ok_or_else(|| format!("{label} must be positive"))
}
