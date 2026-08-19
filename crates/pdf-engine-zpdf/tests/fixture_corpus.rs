use document_core::PdfRect;
use pdf_engine::{
    EditCommand, EngineError, EngineErrorKind, OpenRequest, Password, PdfDocument, PdfEngine,
    RenderRequest, TextStamp,
};
use pdf_engine_zpdf::ZpdfEngine;
use test_support::{form_pdf, image_pdf, malformed_pdf, rotated_pdf, text_pdf};
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

#[test]
fn form_fields_fill_and_round_trip() {
    let mut document = ZpdfEngine.open(OpenRequest::new(form_pdf())).unwrap();
    let fields = document.form_fields().unwrap();
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].name, "customer.name");
    assert_eq!(fields[0].value, "Original");
    assert_eq!(fields[1].value, "Off");
    assert_eq!(fields[2].value, "ES");

    let output = document
        .export(&[
            EditCommand::FillForm {
                name: "customer.name".to_owned(),
                value: "Ada Lovelace".to_owned(),
            },
            EditCommand::FillForm {
                name: "accept".to_owned(),
                value: "true".to_owned(),
            },
            EditCommand::FillForm {
                name: "country".to_owned(),
                value: "AT".to_owned(),
            },
        ])
        .unwrap();
    let reopened = ZpdfEngine.open(OpenRequest::new(output)).unwrap();
    let fields = reopened.form_fields().unwrap();

    assert_eq!(fields[0].value, "Ada Lovelace");
    assert_eq!(fields[1].value, "Yes");
    assert_eq!(fields[2].value, "AT");
}

#[test]
fn added_text_is_extractable_after_save() {
    let mut document = ZpdfEngine.open(OpenRequest::new(text_pdf())).unwrap();
    let output = document
        .export(&[EditCommand::AddText(TextStamp {
            page_index: 0,
            text: "Added by Luna PDF".to_owned(),
            x: 20.0,
            y: 20.0,
            size: 12.0,
        })])
        .unwrap();
    let mut reopened = ZpdfEngine.open(OpenRequest::new(output)).unwrap();

    assert!(
        reopened
            .extract_text(0)
            .unwrap()
            .contains("Added by Luna PDF")
    );
}

#[test]
fn redaction_rewrites_file_and_removes_target_text() {
    let mut document = ZpdfEngine.open(OpenRequest::new(form_pdf())).unwrap();
    let output = document
        .export(&[EditCommand::Redact {
            page_index: 0,
            rect: PdfRect::new(40.0, 70.0, 200.0, 100.0).unwrap(),
        }])
        .unwrap();
    let mut reopened = ZpdfEngine.open(OpenRequest::new(output)).unwrap();
    let text = reopened.extract_text(0).unwrap();

    assert!(!text.contains("secret value"));
    assert!(text.contains("public value"));
}

fn expect_error(result: Result<Box<dyn PdfDocument>, EngineError>) -> EngineError {
    match result {
        Ok(_) => panic!("expected document open to fail"),
        Err(error) => error,
    }
}
