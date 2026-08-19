use gpui::{AppContext, Application, WindowOptions};
use gpui_component::Root;
use ui::ProbeView;

use crate::session;

pub fn run() {
    let path = std::env::args().nth(1);
    Application::new().run(move |cx| {
        gpui_component::init(cx);
        let updates = session::start(path.clone());
        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|cx| ProbeView::new(path.as_deref(), updates, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })?;
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
