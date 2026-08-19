use std::path::PathBuf;

use gpui::{
    AppContext, Application, KeyBinding, Menu, MenuItem, SystemMenuType, WindowOptions, px, size,
};
use gpui_component::{Root, Theme, ThemeMode, TitleBar};
use ui::{
    ActualSize, AddTextTool, CommitText, EditorRequest, EditorView, FitPage, HandTool,
    HighlightTool, NextPage, OpenDocument, PreviousPage, RedactTool, SaveDocument, SelectTool,
    ZoomIn, ZoomOut,
};

use crate::session;

gpui::actions!(gpui_pdf, [Quit]);

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
        Theme::change(ThemeMode::Dark, None, cx);
        cx.activate(true);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([
            KeyBinding::new("cmd-o", OpenDocument, Some("PdfEditor")),
            KeyBinding::new("cmd-shift-s", SaveDocument, Some("PdfEditor")),
            KeyBinding::new("cmd-s", SaveDocument, Some("PdfEditor")),
            KeyBinding::new("left", PreviousPage, Some("PdfEditor")),
            KeyBinding::new("right", NextPage, Some("PdfEditor")),
            KeyBinding::new("v", SelectTool, Some("PdfEditor")),
            KeyBinding::new("h", HandTool, Some("PdfEditor")),
            KeyBinding::new("u", HighlightTool, Some("PdfEditor")),
            KeyBinding::new("t", AddTextTool, Some("PdfEditor")),
            KeyBinding::new("r", RedactTool, Some("PdfEditor")),
            KeyBinding::new("cmd-=", ZoomIn, Some("PdfEditor")),
            KeyBinding::new("cmd--", ZoomOut, Some("PdfEditor")),
            KeyBinding::new("cmd-0", FitPage, Some("PdfEditor")),
            KeyBinding::new("cmd-1", ActualSize, Some("PdfEditor")),
            KeyBinding::new("cmd-enter", CommitText, Some("PdfEditor")),
            KeyBinding::new("cmd-q", Quit, None),
        ]);
        cx.set_menus(app_menus());
        let requests = request_sender.clone();
        let updates = update_receiver.clone();
        let window_options = WindowOptions {
            titlebar: Some(TitleBar::title_bar_options()),
            window_min_size: Some(size(px(900.0), px(620.0))),
            ..WindowOptions::default()
        };
        cx.open_window(window_options, |window, cx| {
            let view = cx.new(|cx| EditorView::new(requests, updates, window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("failed to open application window");
    });
}

fn app_menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "GPUI PDF".into(),
            items: vec![
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Quit GPUI PDF", Quit),
            ],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("Open…", OpenDocument),
                MenuItem::action("Save As…", SaveDocument),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Zoom In", ZoomIn),
                MenuItem::action("Zoom Out", ZoomOut),
                MenuItem::separator(),
                MenuItem::action("Fit Page", FitPage),
                MenuItem::action("Actual Size", ActualSize),
            ],
        },
        Menu {
            name: "Page".into(),
            items: vec![
                MenuItem::action("Previous Page", PreviousPage),
                MenuItem::action("Next Page", NextPage),
            ],
        },
        Menu {
            name: "Tools".into(),
            items: vec![
                MenuItem::action("Select Text", SelectTool),
                MenuItem::action("Hand / Pan", HandTool),
                MenuItem::action("Highlight Text", HighlightTool),
                MenuItem::action("Add Text", AddTextTool),
                MenuItem::action("Redact", RedactTool),
                MenuItem::separator(),
                MenuItem::action("Commit Text", CommitText),
            ],
        },
    ]
}

fn file_url_to_path(value: &str) -> Option<PathBuf> {
    url::Url::parse(value).ok()?.to_file_path().ok()
}
