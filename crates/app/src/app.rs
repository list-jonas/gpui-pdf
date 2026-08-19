use std::path::PathBuf;

use gpui::{AppContext, Application, KeyBinding, WindowOptions};
use gpui_component::Root;
use ui::{EditorRequest, EditorView, NextPage, OpenDocument, PreviousPage, SaveDocument};

use crate::session;

pub fn run() {
    let initial_path = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .find(|path| path.is_file());
    let (request_sender, request_receiver) = async_channel::bounded(16);
    let (update_sender, update_receiver) = async_channel::bounded(16);
    session::start(request_receiver, update_sender);
    if let Some(path) = initial_path {
        let _ = request_sender.try_send(EditorRequest::Open(path));
    }

    let application = Application::new();
    let finder_sender = request_sender.clone();
    application.on_open_urls(move |urls| {
        for path in urls.iter().filter_map(|value| file_url_to_path(value)) {
            let _ = finder_sender.try_send(EditorRequest::Open(path));
        }
    });
    application.run(move |cx| {
        gpui_component::init(cx);
        cx.bind_keys([
            KeyBinding::new("cmd-o", OpenDocument, Some("PdfEditor")),
            KeyBinding::new("cmd-shift-s", SaveDocument, Some("PdfEditor")),
            KeyBinding::new("cmd-s", SaveDocument, Some("PdfEditor")),
            KeyBinding::new("left", PreviousPage, Some("PdfEditor")),
            KeyBinding::new("right", NextPage, Some("PdfEditor")),
        ]);
        let requests = request_sender.clone();
        let updates = update_receiver.clone();
        cx.open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(|cx| EditorView::new(requests, updates, window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("failed to open application window");
    });
}

fn file_url_to_path(value: &str) -> Option<PathBuf> {
    url::Url::parse(value).ok()?.to_file_path().ok()
}
