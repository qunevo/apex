//! APEX desktop display client. It never changes planning state: agents, scripts
//! and other clients act through the control platform, and this window follows.
mod client;
mod i18n;
mod view;

use client::{Client, Signal};
use gpui_kit::*;

fn main() {
    let url = std::env::var("APEX_CONTROL_URL").unwrap_or_else(|_| "http://127.0.0.1:8780".into());
    let Ok(token) = std::env::var("APEX_CONTROL_TOKEN") else {
        eprintln!("Set APEX_CONTROL_TOKEN to a viewer token (see `apex-control init`).");
        std::process::exit(2);
    };
    let client = Client::new(&url, &token);
    let (signals, receiver) = futures_channel::mpsc::unbounded::<Signal>();
    {
        let (client, signals) = (client.clone(), signals.clone());
        std::thread::spawn(move || client.follow(signals));
    }

    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("APEX".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |_, cx| {
            cx.new(|cx| view::Dashboard::new(client, signals, receiver, cx))
        })
        .expect("failed to open window");
        cx.activate(true);
    });
}
