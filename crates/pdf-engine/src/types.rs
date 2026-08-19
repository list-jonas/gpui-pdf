use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;

use document_core::PageGeometry;

#[derive(Clone)]
pub struct Password(Vec<u8>);

impl Password {
    pub fn new(value: impl Into<Vec<u8>>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl Debug for Password {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("Password([REDACTED])")
    }
}

#[derive(Clone, Debug)]
#[must_use]
pub struct OpenRequest {
    pub bytes: Arc<[u8]>,
    pub password: Option<Password>,
}

impl OpenRequest {
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> Self {
        Self {
            bytes: bytes.into(),
            password: None,
        }
    }

    pub fn with_password(mut self, password: Password) -> Self {
        self.password = Some(password);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentMetadata {
    pub page_count: usize,
    pub pdf_version: (u8, u8),
    pub encrypted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageMetadata {
    pub index: usize,
    pub geometry: PageGeometry,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderRequest {
    pub page_index: usize,
    pub scale: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedPage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl RenderedPage {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        Self {
            width,
            height,
            rgba,
        }
    }

    pub fn is_valid(&self) -> bool {
        let expected = u64::from(self.width)
            .checked_mul(u64::from(self.height))
            .and_then(|pixels| pixels.checked_mul(4));
        expected == u64::try_from(self.rgba.len()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::Password;

    #[test]
    fn password_debug_output_is_redacted() {
        let password = Password::new(b"secret".to_vec());

        assert_eq!(format!("{password:?}"), "Password([REDACTED])");
    }
}
