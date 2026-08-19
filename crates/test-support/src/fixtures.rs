use crate::pdf_builder::{build_pdf, stream};

pub fn text_pdf() -> Vec<u8> {
    let content = b"BT /F1 18 Tf 20 50 Td (Phase zero) Tj ET";
    build_pdf(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_vec(),
        stream("", content),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
    ])
}

pub fn rotated_pdf() -> Vec<u8> {
    let content = b"0 0 1 rg 0 0 200 100 re f";
    build_pdf(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Rotate 90 /Resources << >> /Contents 4 0 R >>".to_vec(),
        stream("", content),
    ])
}

pub fn image_pdf() -> Vec<u8> {
    let content = b"q 100 0 0 100 50 0 cm /Im1 Do Q";
    build_pdf(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Resources << /XObject << /Im1 5 0 R >> >> /Contents 4 0 R >>".to_vec(),
        stream("", content),
        stream(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /ASCIIHexDecode",
            b"ff0000>",
        ),
    ])
}

pub fn malformed_pdf() -> Vec<u8> {
    b"%PDF-1.7\n1 0 obj\n<< definitely not complete".to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixtures_have_distinct_content() {
        assert_ne!(text_pdf(), rotated_pdf());
        assert_ne!(text_pdf(), image_pdf());
        assert!(malformed_pdf().starts_with(b"%PDF"));
    }
}
