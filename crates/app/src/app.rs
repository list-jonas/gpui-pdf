use std::path::PathBuf;

use gpui::{
    AppContext, Application, KeyBinding, Menu, MenuItem, SystemMenuType,
    WindowBackgroundAppearance, WindowOptions, px, size,
};
use gpui_component::Root;
use ui::{
    ActualSize, AddNoteHere, AddTextHere, AddTextTool, Cancel, ClearEdits, CloseWindow,
    CopyFilePath, CopyPageText, CopySelection, DeleteAnnotation, DeleteSelection, Deselect,
    DocumentProperties, EditAnnotation, EditTool, EditorRequest, EditorView, FindSelection,
    FirstPage, FitPage, FitWidth, GoToPage, HandTool, HighlightSelection, HighlightTool, LastPage,
    MinimizeWindow, NextPage, NextSearchResult, NoteTool, OpenDocument, OpenInDefaultViewer,
    PasteText, PreviousPage, PreviousSearchResult, RedactSelection, RedactTool, Redo,
    RevealInFinder, SaveDocument, SaveDocumentAs, ScrollDown, ScrollPageDown, ScrollPageUp,
    ScrollToBottom, ScrollToTop, ScrollUp, Search, SelectAllText, SelectTool, ShapeTool,
    SignatureTool, StrikeoutSelection, StrikeoutTool, ToggleFullScreen, TogglePropertiesPanel,
    ToggleReadingMode, ToggleSidebar, UnderlineSelection, UnderlineTool, Undo, ZoomIn, ZoomOut,
    ZoomWindow,
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

    let application = Application::new().with_assets(ui::Assets);
    let finder_sender = request_sender.clone();
    application.on_open_urls(move |urls| {
        for path in urls.iter().filter_map(|value| file_url_to_path(value)) {
            let _ = finder_sender.try_send(EditorRequest::Open(path));
        }
    });
    application.run(move |cx| {
        gpui_component::init(cx);
        ui::apply_glass(cx);
        cx.activate(true);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys(key_bindings());
        cx.set_menus(app_menus());
        let requests = request_sender.clone();
        let updates = update_receiver.clone();
        let window_options = WindowOptions {
            titlebar: Some(gpui::TitlebarOptions {
                // The app draws its own unified title bar; AppKit only owns
                // the traffic lights, inset to sit on that bar's center line.
                title: None,
                appears_transparent: true,
                traffic_light_position: Some(gpui::point(px(14.0), px(13.0))),
            }),
            window_min_size: Some(size(px(900.0), px(620.0))),
            // Translucent chrome over a desktop blur (macOS vibrancy). Every
            // surface except the PDF page paints a tint on top of it.
            window_background: WindowBackgroundAppearance::Blurred,
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
        file_menu(),
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Undo", Undo),
                MenuItem::action("Redo", Redo),
                MenuItem::separator(),
                MenuItem::action("Copy", CopySelection),
                MenuItem::action("Copy Page Text", CopyPageText),
                MenuItem::action("Paste as Text", PasteText),
                MenuItem::separator(),
                MenuItem::action("Select All Text", SelectAllText),
                MenuItem::action("Deselect", Deselect),
                MenuItem::action("Delete Selection", DeleteSelection),
                MenuItem::separator(),
                MenuItem::action("Discard Pending Edits", ClearEdits),
                MenuItem::separator(),
                MenuItem::action("Find…", Search),
                MenuItem::action("Find Next", NextSearchResult),
                MenuItem::action("Find Previous", PreviousSearchResult),
                MenuItem::action("Search for Selection", FindSelection),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Zoom In", ZoomIn),
                MenuItem::action("Zoom Out", ZoomOut),
                MenuItem::separator(),
                MenuItem::action("Actual Size", ActualSize),
                MenuItem::action("Fit Page", FitPage),
                MenuItem::action("Fit Width", FitWidth),
                MenuItem::separator(),
                MenuItem::action("Reading Mode", ToggleReadingMode),
                MenuItem::action("Full Screen Mode", ToggleFullScreen),
                MenuItem::separator(),
                MenuItem::action("Toggle Page Thumbnails", ToggleSidebar),
                MenuItem::action("Toggle Properties Panel", TogglePropertiesPanel),
            ],
        },
        Menu {
            name: "Page".into(),
            items: vec![
                MenuItem::action("Previous Page", PreviousPage),
                MenuItem::action("Next Page", NextPage),
                MenuItem::separator(),
                MenuItem::action("First Page", FirstPage),
                MenuItem::action("Last Page", LastPage),
                MenuItem::action("Go to Page…", GoToPage),
            ],
        },
        window_menu(),
        Menu {
            name: "Tools".into(),
            items: vec![
                MenuItem::action("Select Text", SelectTool),
                MenuItem::action("Hand / Pan", HandTool),
                MenuItem::action("Edit Annotations", EditTool),
                MenuItem::separator(),
                MenuItem::action("Highlight Text", HighlightTool),
                MenuItem::action("Underline Text", UnderlineTool),
                MenuItem::action("Strike Out Text", StrikeoutTool),
                MenuItem::separator(),
                MenuItem::action("Add Text", AddTextTool),
                MenuItem::action("Add Comment", NoteTool),
                MenuItem::action("Add Signature", SignatureTool),
                MenuItem::action("Draw Shape", ShapeTool),
                MenuItem::action("Redact", RedactTool),
            ],
        },
        Menu {
            name: "Annotate".into(),
            items: vec![
                MenuItem::action("Highlight Selection", HighlightSelection),
                MenuItem::action("Underline Selection", UnderlineSelection),
                MenuItem::action("Strike Out Selection", StrikeoutSelection),
                MenuItem::action("Redact Selection", RedactSelection),
                MenuItem::separator(),
                MenuItem::action("Add Text Here", AddTextHere),
                MenuItem::action("Add Comment Here", AddNoteHere),
                MenuItem::separator(),
                MenuItem::action("Edit Annotation", EditAnnotation),
                MenuItem::action("Delete Annotation", DeleteAnnotation),
            ],
        },
    ]
}

fn file_menu() -> Menu {
    Menu {
        name: "File".into(),
        items: vec![
            MenuItem::action("Open…", OpenDocument),
            MenuItem::separator(),
            MenuItem::action("Save", SaveDocument),
            MenuItem::action("Save As…", SaveDocumentAs),
            MenuItem::separator(),
            MenuItem::action("Document Properties…", DocumentProperties),
            MenuItem::action("Open in Default Viewer", OpenInDefaultViewer),
            MenuItem::action("Reveal in Finder", RevealInFinder),
            MenuItem::action("Copy File Path", CopyFilePath),
            MenuItem::separator(),
            MenuItem::action("Close Window", CloseWindow),
        ],
    }
}

fn window_menu() -> Menu {
    Menu {
        name: "Window".into(),
        items: vec![
            MenuItem::action("Minimize", MinimizeWindow),
            MenuItem::action("Zoom", ZoomWindow),
        ],
    }
}

/// Tool shortcuts must not fire while a text field owns the keyboard, so
/// single-letter bindings are scoped to the editor without an active input.
fn key_bindings() -> Vec<KeyBinding> {
    const EDITOR: Option<&str> = Some("PdfEditor");
    const CANVAS: Option<&str> = Some("PdfEditor && !Input");
    vec![
        KeyBinding::new("cmd-o", OpenDocument, EDITOR),
        KeyBinding::new("cmd-s", SaveDocument, EDITOR),
        KeyBinding::new("cmd-shift-s", SaveDocumentAs, EDITOR),
        KeyBinding::new("cmd-w", CloseWindow, EDITOR),
        KeyBinding::new("cmd-d", DocumentProperties, EDITOR),
        KeyBinding::new("cmd-z", Undo, EDITOR),
        KeyBinding::new("cmd-shift-z", Redo, EDITOR),
        KeyBinding::new("cmd-c", CopySelection, CANVAS),
        KeyBinding::new("cmd-shift-c", CopyPageText, CANVAS),
        KeyBinding::new("cmd-v", PasteText, CANVAS),
        KeyBinding::new("cmd-a", SelectAllText, CANVAS),
        KeyBinding::new("cmd-shift-a", Deselect, CANVAS),
        KeyBinding::new("escape", Cancel, EDITOR),
        // Text inputs bind Escape themselves at a deeper context, so an
        // input-scoped binding is needed for Escape to leave the field.
        KeyBinding::new("escape", Cancel, Some("PdfEditor && Input")),
        KeyBinding::new("backspace", DeleteSelection, CANVAS),
        KeyBinding::new("delete", DeleteSelection, CANVAS),
        KeyBinding::new("cmd-f", Search, EDITOR),
        KeyBinding::new("cmd-g", NextSearchResult, EDITOR),
        KeyBinding::new("cmd-shift-g", PreviousSearchResult, EDITOR),
        KeyBinding::new("cmd-e", FindSelection, CANVAS),
        // Markup shortcuts mirror Acrobat: Cmd+Ctrl plus the tool letter.
        KeyBinding::new("cmd-ctrl-h", HighlightSelection, CANVAS),
        KeyBinding::new("cmd-ctrl-u", UnderlineSelection, CANVAS),
        KeyBinding::new("cmd-ctrl-k", StrikeoutSelection, CANVAS),
        KeyBinding::new("cmd-ctrl-r", RedactSelection, CANVAS),
        KeyBinding::new("enter", EditAnnotation, CANVAS),
        KeyBinding::new("cmd-shift-r", RevealInFinder, EDITOR),
        // Arrows scroll like any document reader; Cmd jumps whole pages.
        KeyBinding::new("up", ScrollUp, CANVAS),
        KeyBinding::new("down", ScrollDown, CANVAS),
        KeyBinding::new("pageup", ScrollPageUp, CANVAS),
        KeyBinding::new("pagedown", ScrollPageDown, CANVAS),
        KeyBinding::new("space", ScrollPageDown, CANVAS),
        KeyBinding::new("shift-space", ScrollPageUp, CANVAS),
        KeyBinding::new("home", ScrollToTop, CANVAS),
        KeyBinding::new("end", ScrollToBottom, CANVAS),
        KeyBinding::new("left", PreviousPage, CANVAS),
        KeyBinding::new("right", NextPage, CANVAS),
        KeyBinding::new("cmd-left", PreviousPage, CANVAS),
        KeyBinding::new("cmd-right", NextPage, CANVAS),
        KeyBinding::new("cmd-up", FirstPage, CANVAS),
        KeyBinding::new("cmd-down", LastPage, CANVAS),
        KeyBinding::new("cmd-j", GoToPage, EDITOR),
        KeyBinding::new("cmd-=", ZoomIn, EDITOR),
        KeyBinding::new("cmd-+", ZoomIn, EDITOR),
        KeyBinding::new("cmd--", ZoomOut, EDITOR),
        KeyBinding::new("cmd-0", ActualSize, EDITOR),
        KeyBinding::new("cmd-1", FitPage, EDITOR),
        KeyBinding::new("cmd-2", FitWidth, EDITOR),
        KeyBinding::new("cmd-ctrl-s", ToggleSidebar, EDITOR),
        KeyBinding::new("cmd-alt-0", TogglePropertiesPanel, EDITOR),
        KeyBinding::new("cmd-shift-h", ToggleReadingMode, EDITOR),
        KeyBinding::new("cmd-ctrl-f", ToggleFullScreen, EDITOR),
        KeyBinding::new("cmd-m", MinimizeWindow, EDITOR),
        KeyBinding::new("v", SelectTool, CANVAS),
        KeyBinding::new("e", EditTool, CANVAS),
        KeyBinding::new("h", HandTool, CANVAS),
        KeyBinding::new("u", HighlightTool, CANVAS),
        KeyBinding::new("l", UnderlineTool, CANVAS),
        KeyBinding::new("k", StrikeoutTool, CANVAS),
        KeyBinding::new("t", AddTextTool, CANVAS),
        KeyBinding::new("n", NoteTool, CANVAS),
        KeyBinding::new("s", SignatureTool, CANVAS),
        KeyBinding::new("g", ShapeTool, CANVAS),
        KeyBinding::new("r", RedactTool, CANVAS),
        KeyBinding::new("cmd-q", Quit, None),
    ]
}

fn file_url_to_path(value: &str) -> Option<PathBuf> {
    url::Url::parse(value).ok()?.to_file_path().ok()
}

#[cfg(test)]
mod tests {
    use gpui::{KeyBinding, KeyContext, Keymap, Keystroke};
    use ui::Cancel;

    use super::key_bindings;

    fn resolves(keystroke: &str, contexts: &[&str]) -> Vec<String> {
        let keymap = Keymap::new(key_bindings());
        let stack: Vec<KeyContext> = contexts
            .iter()
            .map(|context| KeyContext::parse(context).unwrap())
            .collect();
        let (bindings, _) =
            keymap.bindings_for_input(&[Keystroke::parse(keystroke).unwrap()], &stack);
        bindings
            .iter()
            .map(|binding| binding.action().name().to_owned())
            .collect()
    }

    #[test]
    fn editor_shortcuts_resolve_on_the_canvas() {
        for (keystroke, action) in [
            ("cmd-z", "Undo"),
            ("cmd-f", "Search"),
            ("cmd-o", "OpenDocument"),
            ("cmd-w", "CloseWindow"),
            ("cmd-d", "DocumentProperties"),
            ("cmd-2", "FitWidth"),
            ("cmd-shift-h", "ToggleReadingMode"),
            ("cmd-ctrl-f", "ToggleFullScreen"),
            ("v", "SelectTool"),
            ("backspace", "DeleteSelection"),
            ("delete", "DeleteSelection"),
            ("cmd-a", "SelectAllText"),
            ("down", "ScrollDown"),
            ("space", "ScrollPageDown"),
            ("home", "ScrollToTop"),
            ("cmd-up", "FirstPage"),
            ("left", "PreviousPage"),
            ("escape", "Cancel"),
            ("cmd-shift-c", "CopyPageText"),
            ("cmd-v", "PasteText"),
            ("cmd-shift-a", "Deselect"),
            ("cmd-e", "FindSelection"),
            ("cmd-ctrl-h", "HighlightSelection"),
            ("cmd-ctrl-u", "UnderlineSelection"),
            ("cmd-ctrl-k", "StrikeoutSelection"),
            ("cmd-ctrl-r", "RedactSelection"),
        ] {
            let resolved = resolves(keystroke, &["Root", "PdfEditor"]);
            assert!(
                resolved.iter().any(|name| name.ends_with(action)),
                "{keystroke} did not resolve to {action}: {resolved:?}"
            );
        }
    }

    /// Context menu items render the shortcut bound to their action, so every
    /// menu command that claims one must actually resolve to it.
    #[test]
    fn context_menu_commands_keep_their_shortcuts_out_of_text_inputs() {
        for keystroke in ["cmd-shift-c", "cmd-v", "cmd-shift-a", "cmd-e", "enter"] {
            let resolved = resolves(keystroke, &["Root", "PdfEditor", "Input"]);
            assert!(
                resolved.is_empty(),
                "{keystroke} leaked into input: {resolved:?}"
            );
        }
    }

    #[test]
    fn tool_letters_are_inert_inside_text_inputs() {
        let resolved = resolves("v", &["Root", "PdfEditor", "Input"]);
        assert!(
            resolved.is_empty(),
            "tool shortcut leaked into input: {resolved:?}"
        );
    }

    /// Typing in a field must never delete an annotation or scroll the page.
    #[test]
    fn editing_keys_are_inert_inside_text_inputs() {
        for keystroke in [
            "backspace",
            "delete",
            "space",
            "down",
            "home",
            "cmd-a",
            "left",
        ] {
            let resolved = resolves(keystroke, &["Root", "PdfEditor", "Input"]);
            assert!(
                resolved.is_empty(),
                "{keystroke} leaked into input: {resolved:?}"
            );
        }
    }

    /// Escape must stay reachable so it can return focus to the page.
    #[test]
    fn escape_still_resolves_inside_text_inputs() {
        let resolved = resolves("escape", &["Root", "PdfEditor", "Input"]);
        assert!(
            resolved
                .first()
                .is_some_and(|name| name.ends_with("Cancel")),
            "escape did not reach the editor: {resolved:?}"
        );
    }

    /// The input widget binds Escape at the same depth. Ours is registered
    /// afterwards and must take precedence, otherwise Escape does nothing
    /// visible while a field has focus.
    #[test]
    fn editor_escape_wins_over_the_input_widgets_own_binding() {
        let mut bindings = vec![KeyBinding::new("escape", Cancel, Some("Input"))];
        bindings.extend(key_bindings());
        let keymap = Keymap::new(bindings);
        let stack: Vec<KeyContext> = ["Root", "PdfEditor", "Input"]
            .iter()
            .map(|context| KeyContext::parse(context).unwrap())
            .collect();

        let (resolved, _) =
            keymap.bindings_for_input(&[Keystroke::parse("escape").unwrap()], &stack);

        assert!(
            resolved
                .first()
                .is_some_and(|binding| binding.action().name().ends_with("Cancel")),
            "editor Escape lost to the input binding"
        );
    }
}
