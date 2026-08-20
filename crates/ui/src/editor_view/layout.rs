use gpui::{
    Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, img, prelude::FluentBuilder, px, rgb, rgba,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Input;
use gpui_component::scroll::ScrollableElement;
use gpui_component::{Disableable, Icon, Sizable, TitleBar};

use crate::EditorView;
use crate::actions::{
    AddTextTool, EditTool, FitPage, FitWidth, HandTool, HighlightTool, NextPage, NextSearchResult,
    NoteTool, OpenDocument, PreviousPage, PreviousSearchResult, RedactTool, Redo, SaveDocument,
    SaveDocumentAs, SelectTool, ShapeTool, SignatureTool, StrikeoutTool, UnderlineTool, Undo,
    ZoomIn, ZoomOut,
};
use crate::icons::HugeIcon;

use super::Severity;
use super::model::Tool;

const SURFACE: u32 = 0x0011_1418;
const BORDER: u32 = 0x0025_292f;
const TEXT_MUTED: u32 = 0x009c_a3ad;
const ACCENT: u32 = 0x004e_9cff;
const DANGER: u32 = 0x00ff_6b6b;

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
        let subtitle = self.path.as_deref().map_or_else(
            || "No document open".to_owned(),
            |path| path.display().to_string(),
        );

        TitleBar::new()
            .h(px(52.0))
            .bg(rgb(0x0014_171b))
            .border_color(rgb(0x0028_2c32))
            .text_color(rgb(0x00f5_f7fa))
            .child(
                div()
                    .flex()
                    .items_center()
                    .size_full()
                    .gap_3()
                    .pr_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(TEXT_MUTED))
                                    .truncate()
                                    .child(subtitle),
                            ),
                    )
                    .when(!self.history.is_empty(), |bar| {
                        bar.child(
                            div()
                                .flex_shrink_0()
                                .px_2()
                                .py_1()
                                .rounded_full()
                                .bg(rgb(0x0027_334a))
                                .text_xs()
                                .text_color(rgb(0x008b_a9ff))
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
                .border_color(rgb(0x0031_353c))
                .bg(rgb(0x0011_1316))
                .text_sm()
                .child(
                    Icon::new(HugeIcon::Search)
                        .size_4()
                        .text_color(rgb(TEXT_MUTED)),
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
                            .text_color(rgb(TEXT_MUTED))
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
            .h(px(82.0))
            .flex()
            .items_center()
            .justify_center()
            .gap_1()
            .px_4()
            .bg(rgb(0x0017_1a1e))
            .border_b_1()
            .border_color(rgb(0x0025_292f))
            .child(self.tool_button(
                ToolButtonSpec {
                    id: "select",
                    label: "Select",
                    icon: HugeIcon::Select,
                    color: 0x004e_9cff,
                    tool: Tool::Select,
                },
                SelectTool,
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
                HandTool,
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
                NoteTool,
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
                EditTool,
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
                AddTextTool,
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
                HighlightTool,
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
                UnderlineTool,
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
                StrikeoutTool,
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
                RedactTool,
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
                ShapeTool,
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
                SignatureTool,
                cx,
            )
            .into_any_element(),
        ]
    }

    fn tool_button<A: gpui::Action>(
        &self,
        spec: ToolButtonSpec,
        action: A,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.tool == spec.tool;
        div()
            .id(spec.id)
            .w(px(70.0))
            .h(px(64.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_1()
            .rounded_lg()
            .text_xs()
            .text_color(rgb(0x00eb_edf0))
            .cursor_pointer()
            .when(selected, |item| item.bg(rgb(0x0027_2d38)))
            .when(!selected, |item| {
                item.hover(|style| style.bg(rgb(0x0020_242a)))
            })
            .child(Icon::new(spec.icon).size_5().text_color(rgb(spec.color)))
            .child(spec.label)
            .on_click(cx.listener(move |_, _, window, cx| {
                window.dispatch_action(action.boxed_clone(), cx);
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
                    .border_color(rgb(0x0030_343b))
                    .bg(rgba(0x191d_23f2))
                    .shadow_xl()
                    .text_color(rgb(0x00f3_f4f6))
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
                                    .w(px(44.0))
                                    .h(px(28.0))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(0x0031_353c))
                                    .bg(rgb(0x0011_1316))
                                    .child(
                                        Input::new(&self.page_input)
                                            .appearance(false)
                                            .bordered(false)
                                            .focus_bordered(false),
                                    ),
                            )
                            .child(
                                div()
                                    .text_color(rgb(TEXT_MUTED))
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
                    .child(div().h_6().w_px().mx_1().bg(rgb(0x0033_3740)))
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
                        div()
                            .id("zoom-reset")
                            .min_w(px(56.0))
                            .text_center()
                            .text_sm()
                            .cursor_pointer()
                            .child(format!("{}%", (self.zoom * 100.0).round()))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.actual_size(&crate::actions::ActualSize, window, cx);
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
    fn render_status_bar(&self) -> impl IntoElement {
        let color = match self.severity {
            Severity::Error => DANGER,
            Severity::Info => TEXT_MUTED,
        };
        div()
            .h(px(26.0))
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .px_3()
            .bg(rgb(SURFACE))
            .border_t_1()
            .border_color(rgb(BORDER))
            .text_xs()
            .child(div().size_2().rounded_full().bg(rgb(if self.busy {
                ACCENT
            } else {
                0x0035_3a41
            })))
            .child(
                div()
                    .text_color(rgb(color))
                    .truncate()
                    .child(self.status.clone()),
            )
            .child(div().flex_1())
            .when_some(self.detail.clone(), |bar, detail| {
                bar.child(div().text_color(rgb(TEXT_MUTED)).child(detail))
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
            let current = page_index == self.page_index;
            let ratio = page.image_size.1 / page.image_size.0.max(1.0);
            let mut preview = div()
                .w(px(104.0))
                .h(px(104.0 * ratio))
                .bg(rgb(0x00ff_ffff))
                .border_2()
                .border_color(if current {
                    rgb(ACCENT)
                } else {
                    rgb(0x0033_3740)
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
                        .bg(rgb(0x00e8_eaee)),
                );
            }
            thumbnails = thumbnails.child(
                div()
                    .id(("page-thumbnail", page_index))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .p_1()
                    .rounded_md()
                    .cursor_pointer()
                    .when(current, |item| item.bg(rgb(0x001d_2530)))
                    .when(!current, |item| {
                        item.hover(|style| style.bg(rgb(0x0018_1c22)))
                    })
                    .child(preview)
                    .child(
                        div()
                            .text_xs()
                            .text_color(if current {
                                rgb(0x00e8_eef8)
                            } else {
                                rgb(TEXT_MUTED)
                            })
                            .child(format!("{}", page_index + 1)),
                    )
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.jump_to_page(page_index, cx);
                        view.sync_page_input(window, cx);
                    })),
            );
        }

        div()
            .w(px(148.0))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .bg(rgb(SURFACE))
            .border_r_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .px_1()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(TEXT_MUTED))
                    .child("PAGES"),
            )
            .child(thumbnails.overflow_y_scrollbar())
    }
}

impl Render for EditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(self.key_context(window, cx))
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
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::open_search))
            .on_action(cx.listener(Self::next_search_result))
            .on_action(cx.listener(Self::previous_search_result))
            .on_action(cx.listener(Self::actual_size))
            .on_action(cx.listener(Self::fit_page))
            .on_action(cx.listener(Self::fit_width))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::window_mouse_up))
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .bg(rgb(0x0009_0c10))
            .text_color(rgb(0x00f1_f3f5))
            .child(self.render_title_bar(cx))
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .bg(rgb(0x0009_0c10))
                    .when(self.panels.sidebar, |row| {
                        row.child(self.render_left_panel(cx))
                    })
                    .child(self.render_document(cx))
                    .child(self.render_floating_controls(cx))
                    .when(self.panels.properties, |row| {
                        row.child(self.render_properties(cx))
                    }),
            )
            .child(self.render_status_bar())
    }
}
