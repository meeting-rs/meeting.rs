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
        &get_element_by_id::<HtmlFormElement>("passphrase-form")
            .expect("#passphrase-form should be an `HtmlFormElement`"),
        "submit",
        move |_| {
            let passphrase = get_element_by_id::<HtmlInputElement>("passphrase")
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
        &get_element_by_id::<HtmlButtonElement>(element_id)
            .unwrap_or_else(|_| panic!("#{} should be an `HtmlButtonElement`", element_id)),
        "click",
        move |_| {
            track.set_enabled(!track.enabled());
        },
    );
    listener.forget();
}

pub(crate) fn sharing_option_listener(element_id: String, mut tx: Sender<UserSharingOption>) {
    let listener = EventListener::once(
        &get_element_by_id::<HtmlButtonElement>(&element_id)
            .unwrap_or_else(|_| panic!("#{} should be an `HtmlButtonElement`", element_id)),
        "click",
        move |_| {
            spawn_local(async move {
                let _ = tx
                    .send({
                        if element_id == "option-media" {
                            UserSharingOption::Media
                        } else {
                            UserSharingOption::Screen
                        }
                    })
                    .await;
            });
        },
    );
    listener.forget();
}

pub(crate) fn get_element_by_id<T: wasm_bindgen::JsCast>(
    element_id: &str,
) -> Result<T, web_sys::Element> {
    document()
        .get_element_by_id(element_id)
        .unwrap_or_else(|| panic!("should have #{} on the page", element_id))
        .dyn_into::<T>()
}

pub(crate) enum UserSharingOption {
    Media,
    Screen,
}
