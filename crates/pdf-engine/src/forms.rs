#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormFieldKind {
    Text,
    Button,
    Choice,
    Signature,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormField {
    pub name: String,
    pub kind: FormFieldKind,
    pub value: String,
    pub options: Vec<(String, String)>,
    pub read_only: bool,
}
