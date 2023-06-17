use futures::{channel::mpsc::Sender, SinkExt};
use gloo_console::log;
use gloo_events::EventListener;
use gloo_utils::document;
use protocol::Event;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlButtonElement, HtmlFormElement, HtmlInputElement, MediaStreamTrack};

pub(crate) fn passphrase_listener(mut tx: Sender<String>) {
    let listener = EventListener::once(
        {
            document()
                .get_element_by_id("passphrase-form")
                .expect("should have #passphrase-form on the page")
                .dyn_ref::<HtmlFormElement>()
                .expect("#passphrase-form should be an `HtmlFormElement`")
        },
        "submit",
        move |_| {
            let passphrase = document()
                .get_element_by_id("passphrase")
                .expect("should have #passphrase on the page")
                .dyn_ref::<HtmlInputElement>()
                .expect("#passphrase should be an `HtmlInputElement`")
                .value();

            spawn_local(async move {
                // Send passphrase.
                tx.send(serde_json::to_string(&Event::Passphrase(passphrase)).unwrap())
                    .await
                    .unwrap();
                log!("successfully sent passphrase.");
            });
        },
    );
    listener.forget();
}

pub(crate) fn track_mute_listener(track: MediaStreamTrack) {
    let element_id = if track.kind() == "audio" {
        "mute-audio"
    } else {
        "mute-video"
    };

    let listener = EventListener::new(
        {
            document()
                .get_element_by_id(element_id)
                .unwrap_or_else(|| panic!("should have #{} on the page", element_id))
                .dyn_ref::<HtmlButtonElement>()
                .unwrap_or_else(|| panic!("#{} should be an `HtmlButtonElement`", element_id))
        },
        "click",
        move |_| {
            if track.enabled() {
                track.set_enabled(false);
            } else {
                track.set_enabled(true);
            }
        },
    );
    listener.forget();
}
