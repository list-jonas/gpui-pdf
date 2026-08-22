use gpui::{
    Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, img, prelude::FluentBuilder, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Input;
use gpui_component::menu::ContextMenuExt;
use gpui_component::scroll::ScrollableElement;
use gpui_component::{Disableable, Icon, Sizable, TitleBar};

use crate::EditorView;
use crate::actions::{
    ActualSize, FirstPage, FitPage, FitWidth, LastPage, NextPage, NextSearchResult, OpenDocument,
    PreviousPage, PreviousSearchResult, Redo, SaveDocument, SaveDocumentAs, Undo, ZoomIn, ZoomOut,
};
use crate::icons::HugeIcon;
use crate::theme::{
    ACCENT, ACCENT_SOFT, ACTIVE, BORDER, BORDER_STRONG, CHROME_TINT, DANGER, FLOAT_TINT, HOVER,
    PANEL_TINT, TEXT, TEXT_FAINT, TEXT_MUTED, WELL_TINT, solid, tint,
};

use super::Severity;
use super::model::Tool;

/// Compact unified title bar: one line, no document path, chrome trailing.
const TITLE_BAR_HEIGHT: f32 = 40.0;

#[derive(Clone, Copy)]
struct ToolButtonSpec {
    id: &'static str,
    label: &'static str,
    icon: HugeIcon,
    color: u32,
    tool: Tool,
}

impl EditorView {
    #[allow(clippy::too_many_lines)]
    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self
            .path
            .as_deref()
            .map_or_else(|| "GPUI PDF".to_owned(), super::document_io::file_name);

        TitleBar::new()
            .h(px(TITLE_BAR_HEIGHT))
            .bg(tint(CHROME_TINT))
            .border_color(tint(BORDER))
            .text_color(solid(TEXT))
            .child(
                div()
                    .flex()
                    .items_center()
                    .size_full()
                    .gap_2()
                    .pr_2()
                    .child(
                        div()
                            .id("document-title")
                            .min_w_0()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(title)
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(|view, event, _, _| view.clear_menu_target(event)),
                            )
                            .context_menu_with_id("title-menu", {
                                let view = cx.entity();
                                move |menu, _, cx| view.read(cx).document_menu(menu)
                            }),
                    )
                    .when(!self.history.is_empty(), |bar| {
                        bar.child(
                            div()
                                .flex_shrink_0()
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .bg(tint(ACCENT_SOFT))
                                .text_xs()
                                .text_color(solid(ACCENT))
                                .child(format!("{} unsaved", self.history.len())),
                        )
                    })
                    .child(div().flex_1())
                    .children(self.render_search_field(cx))
                    .child(
                        Button::new("toggle-sidebar")
                            .icon(HugeIcon::Sidebar)
                            .ghost()
                            .cursor_pointer()
                            .tooltip("Toggle page thumbnails")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.panels.sidebar = !view.panels.sidebar;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("toggle-properties")
                            .icon(HugeIcon::Panel)
                            .ghost()
                            .cursor_pointer()
                            .tooltip("Toggle properties panel")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.panels.properties = !view.panels.properties;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("search-toggle")
                            .icon(HugeIcon::Search)
                            .ghost()
                            .cursor_pointer()
                            .tooltip("Find (Cmd+F)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.toggle_search(window, cx);
                            })),
                    )
                    .child(
                        Button::new("undo")
                            .icon(HugeIcon::Undo)
                            .ghost()
                            .disabled(!self.history.can_undo())
                            .when(self.history.can_undo(), gpui::Styled::cursor_pointer)
                            .tooltip("Undo (Cmd+Z)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.undo(&Undo, window, cx);
                            })),
                    )
                    .child(
                        Button::new("redo")
                            .icon(HugeIcon::Redo)
                            .ghost()
                            .disabled(!self.history.can_redo())
                            .when(self.history.can_redo(), gpui::Styled::cursor_pointer)
                            .tooltip("Redo (Cmd+Shift+Z)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.redo(&Redo, window, cx);
                            })),
                    )
                    .child(
                        Button::new("open")
                            .icon(HugeIcon::Open)
                            .ghost()
                            .cursor_pointer()
                            .tooltip("Open PDF (Cmd+O)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_picker(&OpenDocument, window, cx);
                            })),
                    )
                    .child(
                        Button::new("save")
                            .icon(HugeIcon::Save)
                            .ghost()
                            .disabled(self.path.is_none())
                            .when(self.path.is_some(), gpui::Styled::cursor_pointer)
                            .tooltip("Save (Cmd+S)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.save_document(&SaveDocument, window, cx);
                            })),
                    )
                    .child(
                        Button::new("save-as")
                            .icon(HugeIcon::Share)
                            .ghost()
                            .disabled(self.path.is_none())
                            .when(self.path.is_some(), gpui::Styled::cursor_pointer)
                            .tooltip("Save As… (Cmd+Shift+S)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.save_picker(&SaveDocumentAs, window, cx);
                            })),
                    ),
            )
    }

    fn render_search_field(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        if !self.panels.search {
            return None;
        }
        let status = if self.search_query.is_empty() {
            String::new()
        } else if self.search_matches.is_empty() {
            "0/0".to_owned()
        } else {
            format!("{}/{}", self.search_index + 1, self.search_matches.len())
        };

        Some(
            div()
                .w(px(260.0))
                .h(px(30.0))
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap_1()
                .pl_2()
                .rounded_lg()
                .border_1()
                .border_color(tint(BORDER_STRONG))
                .bg(tint(WELL_TINT))
                .text_sm()
                .child(
                    Icon::new(HugeIcon::Search)
                        .size_4()
                        .text_color(tint(TEXT_MUTED)),
                )
                .child(
                    div().flex_1().min_w_0().child(
                        Input::new(&self.search_input)
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false),
                    ),
                )
                .when(!status.is_empty(), |search| {
                    search.child(
                        div()
                            .flex_shrink_0()
                            .min_w(px(34.0))
                            .text_center()
                            .text_xs()
                            .text_color(tint(TEXT_MUTED))
                            .child(status),
                    )
                })
                .child(
                    Button::new("search-previous")
                        .icon(HugeIcon::Previous)
                        .ghost()
                        .small()
                        .cursor_pointer()
                        .tooltip("Previous result (Cmd+Shift+G)")
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.previous_search_result(&PreviousSearchResult, window, cx);
                        })),
                )
                .child(
                    Button::new("search-next")
                        .icon(HugeIcon::Next)
                        .ghost()
                        .small()
                        .cursor_pointer()
                        .tooltip("Next result (Cmd+G)")
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.next_search_result(&NextSearchResult, window, cx);
                        })),
                )
                .child(
                    Button::new("search-close")
                        .icon(HugeIcon::Close)
                        .ghost()
                        .small()
                        .cursor_pointer()
                        .tooltip("Close search (Esc)")
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.close_search(window, cx);
                        })),
                )
                .into_any_element(),
        )
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("toolbar")
            .h(px(72.0))
            .flex()
            .items_center()
            .justify_center()
            .gap_1()
            .px_4()
            .bg(tint(CHROME_TINT))
            .border_b_1()
            .border_color(tint(BORDER))
            .child(self.tool_button(
                ToolButtonSpec {
                    id: "select",
                    label: "Select",
                    icon: HugeIcon::Select,
                    color: 0x004e_9cff,
                    tool: Tool::Select,
                },
                cx,
            ))
            .child(self.tool_button(
                ToolButtonSpec {
                    id: "hand",
                    label: "Hand",
                    icon: HugeIcon::Hand,
                    color: 0x00c5_cbd3,
                    tool: Tool::Hand,
                },
                cx,
            ))
            .child(self.tool_button(
                ToolButtonSpec {
                    id: "comment",
                    label: "Annotate",
                    icon: HugeIcon::Annotate,
                    color: 0x00f5_b942,
                    tool: Tool::Note,
                },
                cx,
            ))
            .child(self.tool_button(
                ToolButtonSpec {
                    id: "edit",
                    label: "Edit",
                    icon: HugeIcon::Edit,
                    color: 0x004e_9cff,
                    tool: Tool::Edit,
                },
                cx,
            ))
            .child(self.tool_button(
                ToolButtonSpec {
                    id: "add-text",
                    label: "Text",
                    icon: HugeIcon::Text,
                    color: 0x00d5_d7dc,
                    tool: Tool::AddText,
                },
                cx,
            ))
            .child(self.tool_button(
                ToolButtonSpec {
                    id: "highlight",
                    label: "Highlight",
                    icon: HugeIcon::Highlight,
                    color: 0x00ff_d84d,
                    tool: Tool::Highlight,
                },
                cx,
            ))
            .children(self.trailing_toolbar_items(cx))
    }

    fn trailing_toolbar_items(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        vec![
            self.tool_button(
                ToolButtonSpec {
                    id: "underline",
                    label: "Underline",
                    icon: HugeIcon::Underline,
                    color: 0x00b4_63ff,
                    tool: Tool::Underline,
                },
                cx,
            )
            .into_any_element(),
            self.tool_button(
                ToolButtonSpec {
                    id: "strike",
                    label: "Strike",
                    icon: HugeIcon::Strike,
                    color: 0x00ff_4f47,
                    tool: Tool::Strikeout,
                },
                cx,
            )
            .into_any_element(),
            self.tool_button(
                ToolButtonSpec {
                    id: "redact",
                    label: "Redact",
                    icon: HugeIcon::Redact,
                    color: 0x00ff_3d94,
                    tool: Tool::Redact,
                },
                cx,
            )
            .into_any_element(),
            self.tool_button(
                ToolButtonSpec {
                    id: "shapes",
                    label: "Shapes",
                    icon: HugeIcon::Shapes,
                    color: 0x00ca_61ff,
                    tool: Tool::Shape,
                },
                cx,
            )
            .into_any_element(),
            self.tool_button(
                ToolButtonSpec {
                    id: "sign",
                    label: "Sign",
                    icon: HugeIcon::Sign,
                    color: 0x004e_9cff,
                    tool: Tool::Signature,
                },
                cx,
            )
            .into_any_element(),
        ]
    }

    fn tool_button(&self, spec: ToolButtonSpec, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.tool == spec.tool;
        div()
            .id(spec.id)
            .w(px(70.0))
            .h(px(58.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_1()
            .rounded_lg()
            .text_xs()
            .text_color(solid(TEXT))
            .cursor_pointer()
            .when(selected, |item| item.bg(tint(ACTIVE)))
            .when(!selected, |item| item.hover(|style| style.bg(tint(HOVER))))
            .child(Icon::new(spec.icon).size_5().text_color(solid(spec.color)))
            .child(spec.label)
            .on_click(cx.listener(move |view, _, window, cx| {
                view.activate_tool_from_click(spec.tool, window, cx);
            }))
    }

    /// Floating page + zoom controls, with a live page number field.
    #[allow(clippy::too_many_lines)]
    fn render_floating_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let at_start = self.page_index == 0;
        let at_end = self.page_count == 0 || self.page_index + 1 >= self.page_count;
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_4()
            .flex()
            .justify_center()
            .child(
                div()
                    .h(px(48.0))
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .rounded_xl()
                    .border_1()
                    .border_color(tint(BORDER_STRONG))
                    .bg(tint(FLOAT_TINT))
                    .shadow_xl()
                    .text_color(solid(TEXT))
                    .child(
                        Button::new("first-page")
                            .label("⇤")
                            .ghost()
                            .disabled(at_start)
                            .when(at_start, gpui::Styled::cursor_not_allowed)
                            .when(!at_start, gpui::Styled::cursor_pointer)
                            .tooltip("First page (Cmd+↑)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.first_page(&FirstPage, window, cx);
                            })),
                    )
                    .child(
                        Button::new("previous")
                            .icon(HugeIcon::Previous)
                            .ghost()
                            .disabled(at_start)
                            .when(at_start, gpui::Styled::cursor_not_allowed)
                            .when(!at_start, gpui::Styled::cursor_pointer)
                            .tooltip("Previous page (←)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.previous_page(&PreviousPage, window, cx);
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_sm()
                            .child(
                                div()
                                    .w(px(52.0))
                                    .h(px(28.0))
                                    .flex()
                                    .items_center()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(tint(BORDER_STRONG))
                                    .bg(tint(WELL_TINT))
                                    .child(
                                        Input::new(&self.page_input)
                                            .small()
                                            .appearance(false)
                                            .bordered(false)
                                            .focus_bordered(false)
                                            .px(px(4.0))
                                            .text_center(),
                                    ),
                            )
                            .child(
                                div()
                                    .text_color(tint(TEXT_MUTED))
                                    .child(format!("/ {}", self.page_count.max(1))),
                            ),
                    )
                    .child(
                        Button::new("next")
                            .icon(HugeIcon::Next)
                            .ghost()
                            .disabled(at_end)
                            .when(at_end, gpui::Styled::cursor_not_allowed)
                            .when(!at_end, gpui::Styled::cursor_pointer)
                            .tooltip("Next page (→)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.next_page(&NextPage, window, cx);
                            })),
                    )
                    .child(
                        Button::new("last-page")
                            .label("⇥")
                            .ghost()
                            .disabled(at_end)
                            .when(at_end, gpui::Styled::cursor_not_allowed)
                            .when(!at_end, gpui::Styled::cursor_pointer)
                            .tooltip("Last page (Cmd+↓)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.last_page(&LastPage, window, cx);
                            })),
                    )
                    .child(div().h_6().w_px().mx_1().bg(tint(BORDER_STRONG)))
                    .child(
                        Button::new("zoom-out")
                            .icon(HugeIcon::ZoomOut)
                            .ghost()
                            .cursor_pointer()
                            .tooltip("Zoom out (Cmd+-)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.zoom_out(&ZoomOut, window, cx);
                            })),
                    )
                    .child(
                        Button::new("zoom-reset")
                            .label(format!("{}%", (self.zoom * 100.0).round()))
                            .ghost()
                            .cursor_pointer()
                            .tooltip("Actual size (Cmd+0)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.actual_size(&ActualSize, window, cx);
                            })),
                    )
                    .child(
                        Button::new("zoom-in")
                            .icon(HugeIcon::ZoomIn)
                            .ghost()
                            .cursor_pointer()
                            .tooltip("Zoom in (Cmd+=)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.zoom_in(&ZoomIn, window, cx);
                            })),
                    )
                    .child(
                        Button::new("fit")
                            .icon(HugeIcon::Fit)
                            .ghost()
                            .cursor_pointer()
                            .tooltip("Fit page (Cmd+1)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.fit_page(&FitPage, window, cx);
                            })),
                    )
                    .child(
                        Button::new("fit-width")
                            .icon(HugeIcon::FitWidth)
                            .ghost()
                            .cursor_pointer()
                            .tooltip("Fit width (Cmd+2)")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.fit_width(&FitWidth, window, cx);
                            })),
                    ),
            )
    }

    /// Persistent status bar. Transient messages replace the summary briefly
    /// and errors stay visually distinct.
    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let color = match self.severity {
            Severity::Error => solid(DANGER),
            Severity::Info => tint(TEXT_MUTED),
        };
        div()
            .id("status-bar")
            .h(px(26.0))
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .px_3()
            .bg(tint(CHROME_TINT))
            .border_t_1()
            .border_color(tint(BORDER))
            .text_xs()
            .child(div().size_2().rounded_full().bg(if self.busy {
                solid(ACCENT)
            } else {
                tint(BORDER_STRONG)
            }))
            .child(
                div()
                    .text_color(color)
                    .truncate()
                    .child(self.status.clone()),
            )
            .child(div().flex_1())
            .when_some(self.detail.clone(), |bar, detail| {
                bar.child(div().text_color(tint(TEXT_MUTED)).child(detail))
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|view, event, _, _| view.clear_menu_target(event)),
            )
            .context_menu_with_id("status-menu", {
                let view = cx.entity();
                move |menu, _, cx| view.read(cx).document_menu(menu)
            })
    }

    fn render_left_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut thumbnails = div()
            .id("thumbnails")
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .pb_4()
            .track_scroll(&self.thumbnail_scroll);

        for (page_index, page) in self.pages.iter().enumerate() {
            thumbnails = thumbnails.child(self.render_thumbnail(page_index, page, cx));
        }

        div()
            .w(px(148.0))
            .id("thumbnail-panel")
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .bg(tint(PANEL_TINT))
            .border_r_1()
            .border_color(tint(BORDER))
            .child(
                div()
                    .px_1()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(tint(TEXT_FAINT))
                    .child("PAGES"),
            )
            .child(thumbnails.overflow_y_scrollbar())
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|view, event, _, _| view.clear_menu_target(event)),
            )
            .context_menu_with_id("sidebar-menu", {
                let view = cx.entity();
                move |menu, _, cx| view.read(cx).view_menu(menu)
            })
    }

    fn render_thumbnail(
        &self,
        page_index: usize,
        page: &super::document_page::DocumentPage,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let current = page_index == self.page_index;
        let ratio = page.image_size.1 / page.image_size.0.max(1.0);
        let mut preview = div()
            .w(px(104.0))
            .h(px(104.0 * ratio))
            .bg(solid(0x00ff_ffff))
            .border_2()
            .border_color(if current {
                solid(ACCENT)
            } else {
                tint(BORDER_STRONG)
            })
            .rounded_sm()
            .overflow_hidden()
            .shadow_sm();
        if let Some(image) = page.image.clone() {
            preview = preview.child(img(image).size_full());
        } else {
            preview = preview.child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(solid(0x00e8_eaee)),
            );
        }
        div()
            .id(("page-thumbnail", page_index))
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .p_1()
            .rounded_md()
            .cursor_pointer()
            .when(current, |item| item.bg(tint(ACTIVE)))
            .when(!current, |item| item.hover(|style| style.bg(tint(HOVER))))
            .child(preview)
            .child(
                div()
                    .text_xs()
                    .text_color(if current {
                        solid(TEXT)
                    } else {
                        tint(TEXT_MUTED)
                    })
                    .child(format!("{}", page_index + 1)),
            )
            .on_click(cx.listener(move |view, _, window, cx| {
                view.jump_to_page(page_index, cx);
                view.sync_page_input(window, cx);
            }))
            // Right-clicking a thumbnail makes it the current page, so the page
            // commands in its menu act on what was clicked.
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |view, _, window, cx| {
                    view.jump_to_page(page_index, cx);
                    view.sync_page_input(window, cx);
                }),
            )
            // Unique per thumbnail, for the same reason as the page menu.
            .context_menu_with_id(("thumbnail-menu", page_index), {
                let view = cx.entity();
                move |menu, _, cx| view.read(cx).thumbnail_menu(page_index, menu)
            })
    }
}

impl Render for EditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Rendering is the one place every page change funnels through.
        self.sync_page_input(window, cx);
        // Page bounds only exist after the first layout pass, so this is the
        // earliest point the real visible range can be trusted.
        self.settle_visible_pages();
        div()
            .key_context("PdfEditor")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::open_picker))
            .on_action(cx.listener(Self::save_document))
            .on_action(cx.listener(Self::save_picker))
            .on_action(cx.listener(Self::previous_page))
            .on_action(cx.listener(Self::next_page))
            .on_action(cx.listener(Self::first_page))
            .on_action(cx.listener(Self::last_page))
            .on_action(cx.listener(Self::go_to_page))
            .on_action(cx.listener(Self::select_tool))
            .on_action(cx.listener(Self::edit_tool))
            .on_action(cx.listener(Self::hand_tool))
            .on_action(cx.listener(Self::highlight_tool))
            .on_action(cx.listener(Self::underline_tool))
            .on_action(cx.listener(Self::strikeout_tool))
            .on_action(cx.listener(Self::add_text_tool))
            .on_action(cx.listener(Self::note_tool))
            .on_action(cx.listener(Self::signature_tool))
            .on_action(cx.listener(Self::shape_tool))
            .on_action(cx.listener(Self::redact_tool))
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::copy_selection))
            .on_action(cx.listener(Self::select_all_text))
            .on_action(cx.listener(Self::delete_selection))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::scroll_up))
            .on_action(cx.listener(Self::scroll_down))
            .on_action(cx.listener(Self::scroll_page_up))
            .on_action(cx.listener(Self::scroll_page_down))
            .on_action(cx.listener(Self::scroll_to_top))
            .on_action(cx.listener(Self::scroll_to_bottom))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::toggle_properties_panel))
            .on_action(cx.listener(Self::open_search))
            .on_action(cx.listener(Self::next_search_result))
            .on_action(cx.listener(Self::previous_search_result))
            .on_action(cx.listener(Self::actual_size))
            .on_action(cx.listener(Self::fit_page))
            .on_action(cx.listener(Self::fit_width))
            .on_action(cx.listener(Self::highlight_selection))
            .on_action(cx.listener(Self::underline_selection))
            .on_action(cx.listener(Self::strikeout_selection))
            .on_action(cx.listener(Self::redact_selection))
            .on_action(cx.listener(Self::find_selection))
            .on_action(cx.listener(Self::deselect))
            .on_action(cx.listener(Self::copy_page_text))
            .on_action(cx.listener(Self::paste_text))
            .on_action(cx.listener(Self::add_text_here))
            .on_action(cx.listener(Self::add_note_here))
            .on_action(cx.listener(Self::edit_annotation))
            .on_action(cx.listener(Self::delete_annotation))
            .on_action(cx.listener(Self::clear_edits))
            .on_action(cx.listener(Self::reveal_in_finder))
            .on_action(cx.listener(Self::copy_file_path))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::window_mouse_up))
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .bg(tint(crate::theme::WINDOW_FROST))
            .text_color(solid(TEXT))
            .child(self.render_title_bar(cx))
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(self.panels.sidebar, |row| {
                        row.child(self.render_left_panel(cx))
                    })
                    .child(self.render_document(cx))
                    .child(self.render_floating_controls(cx))
                    .when(self.panels.properties, |row| {
                        row.child(self.render_properties(cx))
                    }),
            )
            .child(self.render_status_bar(cx))
    }
}
