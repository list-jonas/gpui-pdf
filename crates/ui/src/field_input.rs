use gpui::{App, Entity};
use gpui_component::input::InputState;
use pdf_engine::FormField;

pub struct FieldInput {
    pub field: FormField,
    pub input: Entity<InputState>,
}

impl FieldInput {
    pub fn value(&self, cx: &App) -> String {
        self.input.read(cx).value().to_string()
    }
}
