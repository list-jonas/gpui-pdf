#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormFieldKind {
    Text,
    Button,
    Choice,
    Signature,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormButtonKind {
    CheckBox,
    Radio,
    Push,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormAction {
    SetToday {
        field_name: String,
        format: String,
    },
    OpenUrl {
        url: String,
    },
    ResetForm,
    SetButtonValue {
        field_name: String,
        when_checked: Option<String>,
        when_unchecked: Option<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormValidation {
    Date {
        format: String,
        display_format: String,
        example: String,
        reject_future: bool,
        minimum: String,
        maximum: String,
    },
    AustrianInsuranceDate,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct FormField {
    pub name: String,
    pub kind: FormFieldKind,
    pub value: String,
    pub options: Vec<(String, String)>,
    /// Text fields: maximum character count (`/MaxLen`); comb fields use it as
    /// the cell count.
    pub max_len: Option<usize>,
    /// Text field accepts multiple lines (`/Ff` bit 13).
    pub multiline: bool,
    /// Value is a password and must never be displayed or logged plainly.
    pub password: bool,
    /// Comb formatting with equally spaced cells (`/Ff` bit 25).
    pub comb: bool,
    /// Choice field allows multiple selected values (`/Ff` bit 22).
    pub multi_select: bool,
    /// Field must have a value before submission (`/Ff` bit 2).
    pub required: bool,
    pub read_only: bool,
    pub button_kind: Option<FormButtonKind>,
    pub validation: Option<FormValidation>,
    pub widgets: Vec<FormWidget>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FormWidget {
    pub page_index: usize,
    pub rect: PdfRect,
    /// False for annotations marked Invisible, Hidden, or `NoView`.
    pub visible: bool,
    /// Acrobat-style tooltip from the widget's `/TU` text string.
    pub hint: Option<String>,
    /// Exact non-Off appearance state for this widget (for example `1` or
    /// `Yes`). Acrobat persists this name in both `/V` and `/AS`.
    pub on_value: Option<String>,
    pub action: Option<FormAction>,
}
use document_core::PdfRect;
