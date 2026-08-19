use pdf_engine::{
    EngineError, EngineErrorKind, OpenRequest, Password, PdfDocument, PdfEngine, RenderRequest,
};
use pdf_engine_zpdf::ZpdfEngine;
use test_support::{image_pdf, malformed_pdf, rotated_pdf, text_pdf};
use zpdf::PdfDocument as NativeDocument;
use zpdf_writer::{EncryptionConfig, RewriteOptions, rewrite_pdf};

#[test]
fn text_fixture_reports_metadata_extracts_and_renders() {
    let mut document = ZpdfEngine.open(OpenRequest::new(text_pdf())).unwrap();

    assert_eq!(document.metadata().page_count, 1);
    assert!(document.extract_text(0).unwrap().contains("Phase zero"));
    assert!(
        document
            .render_page(RenderRequest {
                page_index: 0,
                scale: 1.0,
            })
            .unwrap()
            .is_valid()
    );
}

#[test]
fn image_fixture_renders() {
    let mut document = ZpdfEngine.open(OpenRequest::new(image_pdf())).unwrap();

    let page = document
        .render_page(RenderRequest {
            page_index: 0,
            scale: 1.0,
        })
        .unwrap();
    assert_eq!((page.width, page.height), (200, 100));
}

#[test]
fn rotated_fixture_reports_rotated_geometry_and_raster() {
    let mut document = ZpdfEngine.open(OpenRequest::new(rotated_pdf())).unwrap();

    assert_eq!(
        document
            .page_metadata(0)
            .unwrap()
            .geometry
            .rotation
            .degrees(),
        90
    );
    let page = document
        .render_page(RenderRequest {
            page_index: 0,
            scale: 1.0,
        })
        .unwrap();
    assert_eq!((page.width, page.height), (100, 200));
}

#[test]
fn encrypted_fixture_requires_correct_password() {
    let source = NativeDocument::open(text_pdf()).unwrap();
    let encrypted = rewrite_pdf(
        source.file(),
        &RewriteOptions {
            encrypt: Some(EncryptionConfig::aes256("reader", "owner")),
            ..RewriteOptions::default()
        },
    )
    .unwrap();

    let error = expect_error(ZpdfEngine.open(OpenRequest::new(encrypted.clone())));
    assert_eq!(error.kind(), EngineErrorKind::PasswordRequired);
    let error = expect_error(
        ZpdfEngine.open(OpenRequest::new(encrypted.clone()).with_password(Password::new("wrong"))),
    );
    assert_eq!(error.kind(), EngineErrorKind::IncorrectPassword);
    let document = ZpdfEngine
        .open(OpenRequest::new(encrypted).with_password(Password::new("reader")))
        .unwrap();
    assert!(document.metadata().encrypted);
}

#[test]
fn malformed_fixture_fails_without_panicking() {
    let error = expect_error(ZpdfEngine.open(OpenRequest::new(malformed_pdf())));

    assert_eq!(error.kind(), EngineErrorKind::InvalidDocument);
}

fn expect_error(result: Result<Box<dyn PdfDocument>, EngineError>) -> EngineError {
    match result {
        Ok(_) => panic!("expected document open to fail"),
        Err(error) => error,
    }
}
