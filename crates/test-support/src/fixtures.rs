use crate::pdf_builder::{build_pdf, stream};

pub const MULTI_PAGE_COUNT: usize = 3;

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

pub fn form_pdf() -> Vec<u8> {
    let content = b"BT /F1 12 Tf 50 80 Td (secret value) Tj 0 -40 Td (public value) Tj ET";
    build_pdf(&[
        b"<< /Type /Catalog /Pages 2 0 R /AcroForm 5 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 240 180] /Resources << /Font << /F1 7 0 R >> >> /Contents 4 0 R /Annots [6 0 R 8 0 R 11 0 R] >>".to_vec(),
        stream("", content),
        b"<< /Fields [6 0 R 8 0 R 11 0 R] /DA (/F1 12 Tf 0 g) /DR << /Font << /F1 7 0 R >> >> >>".to_vec(),
        b"<< /Type /Annot /Subtype /Widget /FT /Tx /T (customer.name) /V (Original) /Rect [50 120 200 150] /P 3 0 R /DA (/F1 12 Tf 0 g) >>".to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
        b"<< /Type /Annot /Subtype /Widget /FT /Btn /T (accept) /V /Off /AS /Off /Rect [20 140 32 152] /P 3 0 R /AP << /N << /Off 9 0 R /Yes 10 0 R >> >> >>".to_vec(),
        stream("/Type /XObject /Subtype /Form /BBox [0 0 12 12]", b""),
        stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 12 12]",
            b"0 0 12 12 re S 2 2 m 10 10 l S",
        ),
        b"<< /Type /Annot /Subtype /Widget /FT /Ch /T (country) /V (ES) /Opt [[(ES) (Spain)] [(AT) (Austria)]] /Rect [50 100 200 118] /P 3 0 R /DA (/F1 12 Tf 0 g) >>".to_vec(),
    ])
}

pub fn multi_page_pdf() -> Vec<u8> {
    let font_id = 3 + MULTI_PAGE_COUNT * 2;
    let page_ids: Vec<_> = (0..MULTI_PAGE_COUNT).map(|index| 3 + index * 2).collect();
    let kids = page_ids
        .iter()
        .map(|page_id| format!("{page_id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!("<< /Type /Pages /Kids [{kids}] /Count {MULTI_PAGE_COUNT} >>").into_bytes(),
    ];

    for (index, page_id) in page_ids.iter().enumerate() {
        let content_id = page_id + 1;
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] \
                 /Resources << /Font << /F1 {font_id} 0 R >> >> /Contents {content_id} 0 R >>"
            )
            .into_bytes(),
        );
        let content = format!("BT /F1 18 Tf 20 50 Td (Fixture page {}) Tj ET", index + 1);
        objects.push(stream("", content.as_bytes()));
    }

    objects.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec());
    build_pdf(&objects)
}

/// Pages in the large stress fixture. Big enough that eager whole-document
/// work is obvious, small enough to build in well under a second.
pub const STRESS_PAGE_COUNT: usize = 250;

/// A large, text-heavy document for exercising load, scroll and select-all
/// performance. Generated rather than checked in, so the repository carries no
/// third-party PDF and the content is ours to redistribute.
pub fn stress_pdf(page_count: usize) -> Vec<u8> {
    let font_id = 3 + page_count * 2;
    let page_ids: Vec<_> = (0..page_count).map(|index| 3 + index * 2).collect();
    let kids = page_ids
        .iter()
        .map(|page_id| format!("{page_id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!("<< /Type /Pages /Kids [{kids}] /Count {page_count} >>").into_bytes(),
    ];

    for (index, page_id) in page_ids.iter().enumerate() {
        let content_id = page_id + 1;
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
                 /Resources << /Font << /F1 {font_id} 0 R >> >> /Contents {content_id} 0 R >>"
            )
            .into_bytes(),
        );
        objects.push(stream("", &stress_page_content(index)));
    }

    objects.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec());
    build_pdf(&objects)
}

/// A full page of prose, so text extraction and selection see realistic
/// volumes of individual runs.
fn stress_page_content(page_index: usize) -> Vec<u8> {
    const LINES_PER_PAGE: usize = 46;
    use std::fmt::Write as _;
    let mut content = format!(
        "BT /F1 20 Tf 72 720 Td (Stress page {}) Tj ET\n",
        page_index + 1
    );
    for line in 0..LINES_PER_PAGE {
        let y = 690 - line * 14;
        if y < 72 {
            break;
        }
        let _ = writeln!(
            content,
            "BT /F1 11 Tf 72 {y} Td (Page {} line {}: the quick brown fox jumps over the lazy dog \
             while typesetting engines measure every glyph advance.) Tj ET",
            page_index + 1,
            line + 1
        );
    }
    content.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixtures_have_distinct_content() {
        assert_ne!(text_pdf(), rotated_pdf());
        assert_ne!(text_pdf(), image_pdf());
        assert!(malformed_pdf().starts_with(b"%PDF"));
        assert_ne!(text_pdf(), form_pdf());
    }

    #[test]
    fn multi_page_fixture_has_expected_page_count() {
        let pdf = multi_page_pdf();

        assert!(
            pdf.windows(b"/Count 3".len())
                .any(|window| window == b"/Count 3")
        );
        for page_number in 1..=MULTI_PAGE_COUNT {
            assert!(
                pdf.windows(format!("Fixture page {page_number}").len())
                    .any(|window| window == format!("Fixture page {page_number}").as_bytes())
            );
        }
    }

    #[test]
    fn stress_fixture_is_large_and_well_formed() {
        let pdf = stress_pdf(STRESS_PAGE_COUNT);

        assert!(pdf.starts_with(b"%PDF"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        let marker = format!("/Count {STRESS_PAGE_COUNT}");
        assert!(
            pdf.windows(marker.len())
                .any(|window| window == marker.as_bytes())
        );
    }
}
