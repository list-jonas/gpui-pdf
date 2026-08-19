use gpui::{
    Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, img, prelude::FluentBuilder, px, rgb,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::scroll::ScrollableElement;
use gpui_component::{Disableable, Icon, IconName, TitleBar};

use crate::EditorView;
use crate::actions::{
    AddTextTool, FitPage, HandTool, HighlightTool, NextPage, OpenDocument, PreviousPage,
    RedactTool, SaveDocument, SelectTool, ZoomIn, ZoomOut,
};

use super::model::Tool;

struct ToolButtonSpec {
    id: &'static str,
    label: &'static str,
    icon: IconName,
    color: u32,
    tool: Tool,
}

impl EditorView {
    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self
            .path
            .as_deref()
            .map_or_else(|| "GPUI PDF".to_owned(), super::document_io::file_name);
        let page_status = if self.page_count == 0 {
            self.status.clone()
        } else {
            format!("Page {} of {}", self.page_index + 1, self.page_count).into()
        };

        TitleBar::new()
            .h(px(72.0))
            .bg(rgb(0x0014_171b))
            .border_color(rgb(0x0028_2c32))
            .text_color(rgb(0x00f5_f7fa))
            .child(
                div()
                    .flex()
                    .items_center()
                    .size_full()
                    .gap_3()
                    .pr_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x009c_a3ad))
                                    .child(page_status),
                            ),
                    )
                    .when(!self.edits.is_empty(), |bar| {
                        bar.child(
                            div()
                                .px_2()
                                .py_1()
                                .rounded_full()
                                .bg(rgb(0x0027_334a))
                                .text_xs()
                                .text_color(rgb(0x008b_a9ff))
                                .child(format!("{} edits", self.edits.len())),
                        )
                    })
                    .child(
                        div()
                            .ml_auto()
                            .w(px(190.0))
                            .h_9()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .rounded_lg()
                            .border_1()
                            .border_color(rgb(0x0031_353c))
                            .bg(rgb(0x0011_1316))
                            .text_sm()
                            .text_color(rgb(0x008c_939d))
                            .child(Icon::new(IconName::Search).size_4())
                            .child("Search"),
                    )
                    .child(
                        Button::new("open")
                            .icon(IconName::FolderOpen)
                            .ghost()
                            .tooltip("Open PDF")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_picker(&OpenDocument, window, cx);
                            })),
                    )
                    .child(
                        Button::new("save")
                            .icon(IconName::ExternalLink)
                            .ghost()
                            .tooltip("Save As…")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.save_picker(&SaveDocument, window, cx);
                            })),
                    )
                    .child(
                        Button::new("more")
                            .icon(IconName::Ellipsis)
                            .ghost()
                            .tooltip("More"),
                    ),
            )
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h(px(96.0))
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
                    icon: IconName::Frame,
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
                    icon: IconName::Maximize,
                    color: 0x00c5_cbd3,
                    tool: Tool::Hand,
                },
                HandTool,
                cx,
            ))
            .child(Self::tool_placeholder(
                "annotate",
                "Annotate",
                IconName::Inspector,
                0x00c5_cbd3,
            ))
            .child(Self::tool_placeholder(
                "edit",
                "Edit",
                IconName::CaseSensitive,
                0x004e_9cff,
            ))
            .child(self.tool_button(
                ToolButtonSpec {
                    id: "add-text",
                    label: "Text",
                    icon: IconName::ALargeSmall,
                    color: 0x00d5_d7dc,
                    tool: Tool::AddText,
                },
                AddTextTool,
                cx,
            ))
            .child(Self::tool_placeholder(
                "image",
                "Image",
                IconName::GalleryVerticalEnd,
                0x006b_d96b,
            ))
            .child(Self::tool_placeholder(
                "link",
                "Link",
                IconName::ExternalLink,
                0x00c5_cbd3,
            ))
            .child(self.tool_button(
                ToolButtonSpec {
                    id: "highlight",
                    label: "Highlight",
                    icon: IconName::Palette,
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
            Self::tool_placeholder("underline", "Underline", IconName::Dash, 0x00b4_63ff)
                .into_any_element(),
            Self::tool_placeholder("strike", "Strike", IconName::Minus, 0x00ff_4f47)
                .into_any_element(),
            self.tool_button(
                ToolButtonSpec {
                    id: "redact",
                    label: "Redact",
                    icon: IconName::Delete,
                    color: 0x00ff_3d94,
                    tool: Tool::Redact,
                },
                RedactTool,
                cx,
            )
            .into_any_element(),
            Self::tool_placeholder("shapes", "Shapes", IconName::Frame, 0x00ca_61ff)
                .into_any_element(),
            Self::tool_placeholder("sign", "Sign", IconName::CaseSensitive, 0x004e_a4ff)
                .into_any_element(),
            Self::tool_placeholder("tools-more", "More", IconName::Ellipsis, 0x00c5_cbd3)
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
            .h(px(76.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .rounded_lg()
            .text_sm()
            .text_color(rgb(0x00eb_edf0))
            .cursor_pointer()
            .when(selected, |item| item.bg(rgb(0x0027_2d38)))
            .when(!selected, |item| {
                item.hover(|style| style.bg(rgb(0x0020_242a)))
            })
            .child(Icon::new(spec.icon).size_6().text_color(rgb(spec.color)))
            .child(spec.label)
            .on_click(cx.listener(move |_, _, window, cx| {
                window.dispatch_action(action.boxed_clone(), cx);
            }))
    }

    fn tool_placeholder(
        id: &'static str,
        label: &'static str,
        icon: IconName,
        color: u32,
    ) -> impl IntoElement {
        div()
            .id(id)
            .w(px(70.0))
            .h(px(76.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .rounded_lg()
            .text_sm()
            .text_color(rgb(0x00d5_d8dd))
            .opacity(0.72)
            .child(Icon::new(icon).size_6().text_color(rgb(color)))
            .child(label)
    }

    fn render_floating_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_4()
            .flex()
            .justify_center()
            .child(
                div()
                    .h(px(54.0))
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_3()
                    .rounded_xl()
                    .border_1()
                    .border_color(rgb(0x0030_343b))
                    .bg(rgb(0x0019_1d23))
                    .shadow_xl()
                    .text_color(rgb(0x00f3_f4f6))
                    .child(
                        Button::new("previous")
                            .icon(IconName::ChevronLeft)
                            .ghost()
                            .disabled(self.page_index == 0)
                            .tooltip("Previous page")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.previous_page(&PreviousPage, window, cx);
                            })),
                    )
                    .child(div().min_w(px(84.0)).text_center().text_sm().child(format!(
                        "{} / {}",
                        self.page_index + 1,
                        self.page_count.max(1)
                    )))
                    .child(
                        Button::new("next")
                            .icon(IconName::ChevronRight)
                            .ghost()
                            .disabled(
                                self.page_count == 0 || self.page_index + 1 >= self.page_count,
                            )
                            .tooltip("Next page")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.next_page(&NextPage, window, cx);
                            })),
                    )
                    .child(div().h_6().w_px().mx_2().bg(rgb(0x0033_3740)))
                    .child(
                        Button::new("zoom-out")
                            .icon(IconName::Minus)
                            .ghost()
                            .tooltip("Zoom out")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.zoom_out(&ZoomOut, window, cx);
                            })),
                    )
                    .child(
                        div()
                            .min_w(px(64.0))
                            .text_center()
                            .text_sm()
                            .child(format!("{}%", (self.zoom * 100.0).round())),
                    )
                    .child(
                        Button::new("zoom-in")
                            .icon(IconName::Plus)
                            .ghost()
                            .tooltip("Zoom in")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.zoom_in(&ZoomIn, window, cx);
                            })),
                    )
                    .child(
                        Button::new("fit")
                            .icon(IconName::Maximize)
                            .ghost()
                            .tooltip("Fit page")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.fit_page(&FitPage, window, cx);
                            })),
                    ),
            )
    }

    fn render_left_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut thumbnails = div()
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .gap_4()
            .pb_4();

        for (page_index, page) in self.pages.iter().enumerate() {
            let ratio = page.image_size.1 / page.image_size.0.max(1.0);
            let mut preview = div()
                .w(px(112.0))
                .h(px(112.0 * ratio))
                .p_1()
                .bg(rgb(0x00ff_ffff))
                .border_2()
                .border_color(if page_index == self.page_index {
                    rgb(0x004e_9cff)
                } else {
                    rgb(0x0033_3740)
                })
                .shadow_sm();
            if let Some(image) = page.image.clone() {
                preview = preview.child(img(image).size_full());
            }
            thumbnails = thumbnails.child(
                div()
                    .id(("page-thumbnail", page_index))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .child(preview)
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x00c5_cbd3))
                            .child(format!("Page {}", page_index + 1)),
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.jump_to_page(page_index, cx);
                    })),
            );
        }

        div()
            .w_40()
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .bg(rgb(0x0011_1418))
            .border_r_1()
            .border_color(rgb(0x0025_292f))
            .child(div().font_weight(FontWeight::SEMIBOLD).child("Pages"))
            .child(thumbnails.overflow_y_scrollbar())
    }
}

impl Render for EditorView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_current_page_from_scroll();
        div()
            .key_context("PdfEditor")
            .on_action(cx.listener(Self::open_picker))
            .on_action(cx.listener(Self::save_picker))
            .on_action(cx.listener(Self::previous_page))
            .on_action(cx.listener(Self::next_page))
            .on_action(cx.listener(Self::select_tool))
            .on_action(cx.listener(Self::hand_tool))
            .on_action(cx.listener(Self::highlight_tool))
            .on_action(cx.listener(Self::add_text_tool))
            .on_action(cx.listener(Self::redact_tool))
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::actual_size))
            .on_action(cx.listener(Self::fit_page))
            .on_action(cx.listener(Self::commit_text))
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
                    .child(self.render_left_panel(cx))
                    .child(self.render_document(cx))
                    .child(self.render_floating_controls(cx))
                    .child(self.render_properties(cx)),
            )
    }
}
