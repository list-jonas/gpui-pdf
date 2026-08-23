use gpui::{App, Entity, Subscription};
use gpui_component::input::InputState;
use pdf_engine::FormField;

pub struct FieldInput {
    pub field: FormField,
    pub input: Entity<InputState>,
    pub _subscription: Subscription,
}

impl FieldInput {
    pub fn value(&self, cx: &App) -> String {
        self.input.read(cx).value().to_string()
    }
}
