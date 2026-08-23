use document_core::PdfRect;
use pdf_engine::{
    EditCommand, EngineError, EngineErrorKind, FormAction, FormButtonKind, FormValidation,
    OpenRequest, Password, PdfDocument, PdfEngine, RenderRequest, ShapeKind, TextStamp,
};
use pdf_engine_zpdf::ZpdfEngine;
use test_support::austrian_neufoe2_pdf;
use test_support::{form_pdf, image_pdf, malformed_pdf, rotated_pdf, scripted_form_pdf, text_pdf};
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
    assert_eq!(fields[1].button_kind, Some(FormButtonKind::CheckBox));
    assert_eq!(fields[1].widgets[0].on_value.as_deref(), Some("Yes"));
    assert_eq!(fields[2].value, "ES");
    assert_eq!(fields[0].widgets[0].page_index, 0);
    assert_eq!(
        fields[0].widgets[0].rect,
        PdfRect::new(50.0, 120.0, 200.0, 150.0).unwrap()
    );

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
    let native = NativeDocument::open(output.clone()).unwrap();
    let accept = native
        .acro_form()
        .unwrap()
        .fields
        .iter()
        .find(|field| field.name == "accept")
        .unwrap();
    assert_eq!(
        native
            .file()
            .resolve(accept.widgets[0])
            .unwrap()
            .as_dict()
            .unwrap()
            .get_name("AS")
            .unwrap(),
        "Yes"
    );
    let reopened = ZpdfEngine.open(OpenRequest::new(output)).unwrap();
    let fields = reopened.form_fields().unwrap();

    assert_eq!(fields[0].value, "Ada Lovelace");
    assert_eq!(fields[1].value, "Yes");
    assert_eq!(fields[2].value, "AT");
}

#[test]
fn scripted_form_exposes_button_states_actions_and_validation() {
    let document = ZpdfEngine
        .open(OpenRequest::new(scripted_form_pdf()))
        .unwrap();
    let fields = document.form_fields().unwrap();

    assert_eq!(fields.len(), 6);
    let date = fields.iter().find(|field| field.name == "date").unwrap();
    assert_eq!(
        date.validation,
        Some(FormValidation::Date {
            format: "dd.mm.yyyy".to_owned(),
            display_format: "TT.MM.JJJJ".to_owned(),
            example: "11.03.2007".to_owned(),
            reject_future: true,
            minimum: "01.01.1850".to_owned(),
            maximum: "31.12.2200".to_owned(),
        })
    );

    let set_date = fields
        .iter()
        .find(|field| field.name == "set-date")
        .unwrap();
    assert_eq!(set_date.button_kind, Some(FormButtonKind::Push));
    assert_eq!(
        set_date.widgets[0].action,
        Some(FormAction::SetToday {
            field_name: "date".to_owned(),
            format: "dd.mm.yyyy".to_owned(),
        })
    );

    let kind = fields.iter().find(|field| field.name == "kind").unwrap();
    assert_eq!(kind.button_kind, Some(FormButtonKind::CheckBox));
    assert_eq!(kind.widgets.len(), 2);
    assert_eq!(kind.widgets[0].on_value.as_deref(), Some("1"));
    assert_eq!(kind.widgets[1].on_value.as_deref(), Some("2"));

    let master = fields.iter().find(|field| field.name == "master").unwrap();
    assert_eq!(
        master.widgets[0].action,
        Some(FormAction::SetButtonValue {
            field_name: "kind".to_owned(),
            when_checked: None,
            when_unchecked: Some("Off".to_owned()),
        })
    );
    let reset = fields.iter().find(|field| field.name == "reset").unwrap();
    assert_eq!(reset.widgets[0].action, Some(FormAction::ResetForm));
    let insurance = fields
        .iter()
        .find(|field| field.name == "insurance-date")
        .unwrap();
    assert_eq!(
        insurance.validation,
        Some(FormValidation::AustrianInsuranceDate)
    );
}

#[test]
fn multi_state_checkbox_round_trips_exact_export_value() {
    let mut document = ZpdfEngine
        .open(OpenRequest::new(scripted_form_pdf()))
        .unwrap();
    let output = document
        .export(&[EditCommand::FillForm {
            name: "kind".to_owned(),
            value: "2".to_owned(),
        }])
        .unwrap();
    let native = NativeDocument::open(output.clone()).unwrap();
    let kind = native
        .acro_form()
        .unwrap()
        .fields
        .iter()
        .find(|field| field.name == "kind")
        .unwrap();
    let appearance_states: Vec<_> = kind
        .widgets
        .iter()
        .map(|id| {
            native
                .file()
                .resolve(*id)
                .unwrap()
                .as_dict()
                .unwrap()
                .get_name("AS")
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(appearance_states, ["Off", "2"]);
    let reopened = ZpdfEngine.open(OpenRequest::new(output)).unwrap();
    let fields = reopened.form_fields().unwrap();

    assert_eq!(
        fields
            .iter()
            .find(|field| field.name == "kind")
            .unwrap()
            .value,
        "2"
    );
}

#[test]
fn austrian_neufoe2_form_loads_and_round_trips() {
    let mut document = ZpdfEngine
        .open(OpenRequest::new(austrian_neufoe2_pdf()))
        .unwrap();
    let fields = document.form_fields().unwrap();

    assert_eq!(fields.len(), 41);
    for page_index in 0..2 {
        assert!(
            document
                .render_page(RenderRequest {
                    page_index,
                    scale: 1.0,
                })
                .unwrap()
                .is_valid()
        );
    }
    let date = fields
        .iter()
        .find(|field| field.name == "Tagesdatum2")
        .unwrap();
    assert_eq!(
        date.validation,
        Some(FormValidation::Date {
            format: "dd.mm.yyyy".to_owned(),
            display_format: "TT.MM.JJJJ".to_owned(),
            example: "11.03.2007".to_owned(),
            reject_future: true,
            minimum: "01.01.1850".to_owned(),
            maximum: "31.12.2200".to_owned(),
        })
    );

    let output = document
        .export(&[EditCommand::FillForm {
            name: "Tagesdatum2".to_owned(),
            value: "23.08.2026".to_owned(),
        }])
        .unwrap();
    let fields = ZpdfEngine
        .open(OpenRequest::new(output))
        .unwrap()
        .form_fields()
        .unwrap();
    assert_eq!(
        fields
            .iter()
            .find(|field| field.name == "Tagesdatum2")
            .unwrap()
            .value,
        "23.08.2026"
    );
}

#[test]
fn text_fragments_have_selectable_page_geometry() {
    let mut document = ZpdfEngine.open(OpenRequest::new(text_pdf())).unwrap();
    let fragments = document.text_fragments(0).unwrap();

    assert_eq!(fragments.len(), "Phase zero".chars().count());
    assert_eq!(
        fragments
            .iter()
            .map(|fragment| fragment.text.as_str())
            .collect::<String>(),
        "Phase zero"
    );
    assert_eq!(fragments[0].text, "P");
    assert_eq!(fragments[5].text, " ");
    assert_eq!(fragments[9].text, "o");
    assert!(fragments.iter().all(|fragment| fragment.rect.width() > 1.0));
    assert!(
        fragments
            .iter()
            .all(|fragment| fragment.rect.height() > 10.0)
    );
    assert!(
        fragments
            .windows(2)
            .all(|pair| pair[0].rect.x_max <= pair[1].rect.x_min)
    );
}

#[test]
fn highlight_annotation_round_trips() {
    let mut document = ZpdfEngine.open(OpenRequest::new(text_pdf())).unwrap();
    let output = document
        .export(&[EditCommand::Highlight {
            page_index: 0,
            rects: vec![PdfRect::new(18.0, 45.0, 115.0, 66.0).unwrap()],
            color: (1.0, 0.86, 0.2),
        }])
        .unwrap();
    let native = NativeDocument::open(output.clone()).unwrap();
    let page = native.page(0).unwrap();

    assert!(
        native
            .page_annotations(&page)
            .iter()
            .any(|annotation| annotation.subtype == "Highlight")
    );
    let mut reopened = ZpdfEngine.open(OpenRequest::new(output)).unwrap();
    assert!(
        reopened
            .render_page(RenderRequest {
                page_index: 0,
                scale: 1.0,
            })
            .unwrap()
            .is_valid()
    );
}

#[test]
fn review_annotations_round_trip_with_expected_subtypes() {
    let mut document = ZpdfEngine.open(OpenRequest::new(text_pdf())).unwrap();
    let output = document
        .export(&[
            EditCommand::Underline {
                page_index: 0,
                rects: vec![PdfRect::new(18.0, 45.0, 115.0, 66.0).unwrap()],
                color: (0.23, 0.51, 0.96),
            },
            EditCommand::StrikeOut {
                page_index: 0,
                rects: vec![PdfRect::new(18.0, 45.0, 115.0, 66.0).unwrap()],
                color: (0.94, 0.27, 0.27),
            },
            EditCommand::Note {
                page_index: 0,
                x: 30.0,
                y: 90.0,
                contents: "Review this section".to_owned(),
                color: (0.13, 0.65, 0.32),
            },
            EditCommand::Shape {
                page_index: 0,
                kind: ShapeKind::Rectangle,
                rect: PdfRect::new(20.0, 20.0, 100.0, 40.0).unwrap(),
                color: (0.23, 0.51, 0.96),
                width: 2.0,
            },
            EditCommand::Shape {
                page_index: 0,
                kind: ShapeKind::Ellipse,
                rect: PdfRect::new(120.0, 20.0, 180.0, 55.0).unwrap(),
                color: (0.94, 0.27, 0.27),
                width: 2.0,
            },
        ])
        .unwrap();
    let native = NativeDocument::open(output.clone()).unwrap();
    let page = native.page(0).unwrap();
    let annotations = native.page_annotations(&page);
    let subtypes: Vec<_> = annotations
        .iter()
        .map(|annotation| annotation.subtype.as_str())
        .collect();

    for expected in ["Underline", "StrikeOut", "Text", "Square", "Circle"] {
        assert!(
            subtypes.contains(&expected),
            "missing {expected} annotation"
        );
    }
    let mut reopened = ZpdfEngine.open(OpenRequest::new(output)).unwrap();
    assert!(
        reopened
            .render_page(RenderRequest {
                page_index: 0,
                scale: 1.0,
            })
            .unwrap()
            .is_valid()
    );
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
