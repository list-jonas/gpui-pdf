use std::sync::Arc;

use async_channel::Receiver;
use gpui::{
    Context, FontWeight, IntoElement, ParentElement, Render, RenderImage, SharedString, Styled,
    Window, div, img, prelude::FluentBuilder,
};

#[derive(Debug)]
pub struct PagePreview {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug)]
pub enum ProbeUpdate {
    Ready {
        summary: String,
        text: String,
        preview: PagePreview,
    },
    Failed(String),
}

pub struct ProbeView {
    title: SharedString,
    status: SharedString,
    detail: Option<SharedString>,
    text: Option<SharedString>,
    image: Option<Arc<RenderImage>>,
}

impl ProbeView {
    pub fn new(path: Option<&str>, updates: Receiver<ProbeUpdate>, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |view, cx| {
            while let Ok(update) = updates.recv().await {
                if view
                    .update(cx, |view, cx| {
                        view.apply(update);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            title: "GPUI PDF engine probe".into(),
            status: "Loading PDF on document worker…".into(),
            detail: path.map(|path| SharedString::from(path.to_owned())),
            text: None,
            image: None,
        }
    }

    fn apply(&mut self, update: ProbeUpdate) {
        match update {
            ProbeUpdate::Ready {
                summary,
                text,
                preview,
            } => {
                self.status = "Render complete".into();
                self.detail = Some(summary.into());
                self.text = Some(text.into());
                self.image = render_image(preview);
            }
            ProbeUpdate::Failed(message) => {
                self.status = "Unable to render PDF".into();
                self.detail = Some(message.into());
                self.text = None;
                self.image = None;
            }
        }
    }
}

impl Render for ProbeView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .p_6()
            .bg(gpui::rgb(0x00f7_f7f7))
            .text_color(gpui::rgb(0x0020_2124))
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.title.clone()),
            )
            .child(self.status.clone())
            .when_some(self.detail.clone(), ParentElement::child)
            .when_some(self.text.clone(), |view, text| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(gpui::rgb(0x005f_6368))
                        .child(text),
                )
            })
            .when_some(self.image.clone(), |view, image| {
                view.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .items_center()
                        .justify_center()
                        .child(img(image).size_full()),
                )
            })
    }
}

fn render_image(mut preview: PagePreview) -> Option<Arc<RenderImage>> {
    for pixel in preview.rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let image = image::RgbaImage::from_raw(preview.width, preview.height, preview.rgba)?;
    Some(Arc::new(RenderImage::new([image::Frame::new(image)])))
}
