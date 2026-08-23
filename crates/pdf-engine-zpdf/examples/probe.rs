//! Ad-hoc `AcroForm` probe: cargo run --release -p pdf-engine-zpdf --example probe -- file.pdf
#![allow(clippy::too_many_lines)]
use pdf_engine::{
    EditCommand, FormAction, FormButtonKind, FormFieldKind, OpenRequest, PdfEngine, RenderRequest,
};
use pdf_engine_zpdf::ZpdfEngine;

fn main() {
    let path = std::env::args().nth(1).expect("usage: probe <file.pdf>");
    let bytes = std::fs::read(&path).expect("read pdf");
    let mut document = ZpdfEngine.open(OpenRequest::new(bytes)).expect("open");

    println!("pages={}", document.metadata().page_count);
    let fields = document.form_fields().expect("form fields");
    println!("fields={}", fields.len());
    for field in &fields {
        let kind = match field.kind {
            FormFieldKind::Text => "Tx",
            FormFieldKind::Button => "Btn",
            FormFieldKind::Choice => "Ch",
            FormFieldKind::Signature => "Sig",
            FormFieldKind::Unknown => "?",
        };
        let button = match field.button_kind {
            Some(FormButtonKind::CheckBox) => "/chk",
            Some(FormButtonKind::Radio) => "/radio",
            Some(FormButtonKind::Push) => "/push",
            None => "",
        };
        println!(
            "  {:?} kind={}{} ro={} value={:?} opts={} widgets={}",
            field.name,
            kind,
            button,
            field.read_only,
            field.value,
            field.options.len(),
            field.widgets.len(),
        );
        for widget in &field.widgets {
            let action = match &widget.action {
                Some(FormAction::SetToday { .. }) => " SetToday",
                Some(FormAction::ResetForm) => " ResetForm",
                Some(FormAction::SetButtonValue { .. }) => " SetButtonValue",
                None => "",
            };
            println!(
                "    widget page={} rect=({:.0},{:.0},{:.0},{:.0}) visible={} on={:?}{}",
                widget.page_index,
                widget.rect.x_min,
                widget.rect.y_min,
                widget.rect.x_max,
                widget.rect.y_max,
                widget.visible,
                widget.on_value,
                action,
            );
        }
    }

    for index in 0..document.metadata().page_count {
        let page = document
            .render_page(RenderRequest {
                page_index: index,
                scale: 1.0,
            })
            .unwrap_or_else(|error| panic!("render page {index}: {error}"));
        assert!(page.is_valid(), "page {index} raster invalid");
    }
    println!("render OK");

    let edits: Vec<EditCommand> = fields
        .iter()
        .filter(|field| !field.read_only)
        .filter_map(|field| {
            let value = match field.kind {
                FormFieldKind::Text => "PROBE".to_owned(),
                FormFieldKind::Button if field.button_kind != Some(FormButtonKind::Push) => {
                    if field.button_kind == Some(FormButtonKind::Push) {
                        return None;
                    }
                    let mut on_states = Vec::new();
                    for widget in &field.widgets {
                        if let Some(state) = &widget.on_value {
                            on_states.push(state.clone());
                        }
                    }
                    let value = field
                        .widgets
                        .first()
                        .and_then(|widget| widget.on_value.clone());
                    if let Some(state) = &value {
                        if !on_states.contains(state) {
                            return None;
                        }
                    } else {
                        return None;
                    }
                    value.expect("checked above")
                }
                FormFieldKind::Choice => field
                    .options
                    .first()
                    .map_or_else(|| "PROBE".to_owned(), |(export, _)| export.clone()),
                _ => return None,
            };
            Some(EditCommand::FillForm {
                name: field.name.clone(),
                value,
            })
        })
        .collect();

    let stem = std::path::Path::new(&path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("output");
    let output_path = format!("/tmp/{stem}_probe_filled.pdf");
    match document.export(&edits) {
        Ok(output) => {
            std::fs::write(&output_path, &output).expect("write output");
            let mut reopened = ZpdfEngine.open(OpenRequest::new(output)).expect("reopen");
            let after = reopened.form_fields().expect("reopened fields");
            let mut mismatches = Vec::new();
            for edit in &edits {
                let EditCommand::FillForm { name, value } = edit else {
                    continue;
                };
                match after.iter().find(|field| field.name == *name) {
                    Some(field) if &field.value == value => {}
                    Some(field) => {
                        mismatches.push(format!("{name}: set {value:?}, got {:?}", field.value));
                    }
                    None => mismatches.push(format!("{name}: missing after round-trip")),
                }
            }
            if mismatches.is_empty() {
                println!("fill round-trip OK ({} edits)", edits.len());
            } else {
                for mismatch in &mismatches {
                    println!("MISMATCH {mismatch}");
                }
            }
            for index in 0..reopened.metadata().page_count {
                assert!(
                    reopened
                        .render_page(RenderRequest {
                            page_index: index,
                            scale: 1.0
                        })
                        .expect("rerender")
                        .is_valid()
                );
            }
            println!("saved {output_path}");
        }
        Err(error) => println!("EXPORT ERROR {error}"),
    }
}
