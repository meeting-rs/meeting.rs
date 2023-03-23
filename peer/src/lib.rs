use futures::{
    channel::mpsc::{self, Sender},
    stream::SplitStream,
    SinkExt, StreamExt,
};
use gloo_console::log;
use gloo_dialogs::alert;
use gloo_net::websocket::{futures::WebSocket, Message};
use gloo_utils::{document, window};
use js_sys::{Array, Error, Object, Reflect};
use protocol::{Event, IceCandidate, Role};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    HtmlFormElement, HtmlInputElement, HtmlMediaElement, MediaStream, MediaStreamConstraints,
    RtcConfiguration, RtcIceCandidate, RtcIceCandidateInit, RtcIceConnectionState,
    RtcPeerConnection, RtcPeerConnectionIceEvent, RtcSdpType, RtcSessionDescriptionInit,
    RtcTrackEvent,
};

#[wasm_bindgen(start)]
pub async fn main() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let ws = WebSocket::open("ws://localhost:3000/websocket")
        .map_err(|err| Error::new(&err.to_string()))?;
    log!("WebSocket Connected.");
    let (mut write, read) = ws.split();

    let (tx, mut rx) = mpsc::channel(10);

    // Write task.
    spawn_local(async move {
        while let Some(msg) = rx.next().await {
            write.send(Message::Text(msg)).await.unwrap();
        }
    });

    let pc = peer_connection()?;
    log!("pc created: state:", pc.signaling_state());
    onicecandidate(&pc, tx.clone());
    onconnectionstatechange(&pc, tx.clone());
    ontrack(&pc);

    // Read task.
    handle_events(pc.clone(), tx.clone(), read);

    passphrase_listener(tx, pc);

    Ok(())
}

fn handle_events(pc: RtcPeerConnection, mut tx: Sender<String>, mut read: SplitStream<WebSocket>) {
    spawn_local(async move {
        while let Some(Ok(Message::Text(msg))) = read.next().await {
            let event: Event = serde_json::from_str(&msg).unwrap();
            match event {
                Event::Role(role) => {
                    log!("this peer's role is:", role.to_string());
                    if let Role::Initiator = role {
                        // Send offer.
                        let offer = JsFuture::from(pc.create_offer()).await.unwrap();
                        let offer_sdp = Reflect::get(&offer, &JsValue::from_str("sdp"))
                            .unwrap()
                            .as_string()
                            .unwrap();

                        let mut offer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                        offer_obj.sdp(&offer_sdp);
                        let sld_promise = pc.set_local_description(&offer_obj);
                        JsFuture::from(sld_promise).await.unwrap();
                        log!("pc: state:", pc.signaling_state());

                        tx.send(serde_json::to_string(&Event::Offer(offer_sdp)).unwrap())
                            .await
                            .unwrap();
                        log!("sent an offer.");
                    }
                }
                Event::Offer(offer) => {
                    log!("received offer");
                    let mut offer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                    offer_obj.sdp(&offer);
                    let srd_promise = pc.set_remote_description(&offer_obj);
                    JsFuture::from(srd_promise).await.unwrap();
                    log!("pc: state:", pc.signaling_state());

                    let answer = JsFuture::from(pc.create_answer()).await.unwrap();
                    let answer_sdp = Reflect::get(&answer, &JsValue::from_str("sdp"))
                        .unwrap()
                        .as_string()
                        .unwrap();

                    let mut answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                    answer_obj.sdp(&answer_sdp);
                    let sld_promise = pc.set_local_description(&answer_obj);
                    JsFuture::from(sld_promise).await.unwrap();
                    log!("pc: state:", pc.signaling_state());

                    tx.send(serde_json::to_string(&Event::Answer(answer_sdp)).unwrap())
                        .await
                        .unwrap();
                    log!("sent an answer.");
                }
                Event::Answer(answer) => {
                    log!("received answer");
                    let mut answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                    answer_obj.sdp(&answer);
                    let srd_promise = pc.set_remote_description(&answer_obj);
                    JsFuture::from(srd_promise).await.unwrap();
                    log!("pc: state:", pc.signaling_state());
                }
                Event::IceCandidate(candidate) => {
                    log!("received a candidate.");
                    let candidate = RtcIceCandidate::new(&{
                        let mut rtc_candidate = RtcIceCandidateInit::new("");
                        rtc_candidate.candidate(&candidate.candidate);
                        rtc_candidate.sdp_m_line_index(candidate.sdp_m_line_index);
                        rtc_candidate.sdp_mid(candidate.sdp_mid.as_deref());
                        rtc_candidate
                    })
                    .unwrap();
                    let promise = pc.add_ice_candidate_with_opt_rtc_ice_candidate(Some(&candidate));
                    JsFuture::from(promise).await.unwrap();
                }
                Event::Error(error) => {
                    log!("An error occurred:", &error);
                    alert(&error);
                    return;
                }
                _ => {}
            }
        }
        log!("WebSocket Closed.")
    });
}

fn ontrack(pc: &RtcPeerConnection) {
    let ontrack_callback = Closure::<dyn FnMut(_)>::new(move |ev: RtcTrackEvent| {
        let first_remote_stream = ev.streams().at(0);
        document()
            .get_element_by_id("remote-video")
            .expect("should have #remote-video on the page")
            .dyn_ref::<HtmlMediaElement>()
            .expect("#remote-video should be an `HtmlVideoElement`")
            .set_src_object(first_remote_stream.dyn_ref());
        log!("added remote stream.");
    });
    pc.set_ontrack(Some(ontrack_callback.as_ref().unchecked_ref()));
    ontrack_callback.forget();
}

fn onconnectionstatechange(pc: &RtcPeerConnection, tx: Sender<String>) {
    let pc_clone = pc.clone();
    let onconnectionstatechange_callback = Closure::<dyn FnMut()>::new(move || {
        log!("pc state:", pc_clone.ice_connection_state());
        if matches!(
            pc_clone.ice_connection_state(),
            RtcIceConnectionState::Connected | RtcIceConnectionState::Failed
        ) {
            let mut tx = tx.clone();
            spawn_local(async move {
                tx.send(serde_json::to_string(&Event::CloseConnection).unwrap())
                    .await
                    .unwrap();
            });
        }
    });
    pc.set_oniceconnectionstatechange(Some(
        onconnectionstatechange_callback.as_ref().unchecked_ref(),
    ));
    onconnectionstatechange_callback.forget();
}

fn onicecandidate(pc: &RtcPeerConnection, tx: Sender<String>) {
    let onicecandidate_callback =
        Closure::<dyn FnMut(_)>::new(move |ev: RtcPeerConnectionIceEvent| {
            if let Some(candidate) = ev.candidate() {
                let mut tx = tx.clone();
                spawn_local(async move {
                    tx.send(
                        serde_json::to_string(&Event::IceCandidate(IceCandidate {
                            candidate: candidate.candidate(),
                            sdp_mid: candidate.sdp_mid(),
                            sdp_m_line_index: candidate.sdp_m_line_index(),
                        }))
                        .unwrap(),
                    )
                    .await
                    .unwrap();
                    log!("successfully sent a candidate.");
                });
            }
        });
    pc.set_onicecandidate(Some(onicecandidate_callback.as_ref().unchecked_ref()));
    onicecandidate_callback.forget();
}

async fn handle_local_stream(pc: &RtcPeerConnection) -> Result<(), JsValue> {
    let local_stream = MediaStream::from(
        JsFuture::from(
            window()
                .navigator()
                .media_devices()?
                .get_user_media_with_constraints(&{
                    let mut media_stream_constraints = MediaStreamConstraints::new();
                    media_stream_constraints
                        .video(&JsValue::from_bool(true))
                        .audio(&JsValue::from_bool(false));
                    media_stream_constraints
                })?,
        )
        .await?,
    );
    local_stream
        .get_tracks()
        .for_each(&mut |track: JsValue, _, _| {
            pc.add_track_0(track.dyn_ref().unwrap(), &local_stream);
            log!("added a local track.");
        });
    Ok(())
}

fn peer_connection() -> Result<RtcPeerConnection, JsValue> {
    RtcPeerConnection::new_with_configuration(&{
        let ice_servers = Array::new();
        let server_entry = Object::new();
        Reflect::set(
            &server_entry,
            &"urls".into(),
            &"stun:stun.l.google.com:19302".into(),
        )?;
        ice_servers.push(&server_entry);

        let mut rtc_configuration = RtcConfiguration::new();
        rtc_configuration.ice_servers(&ice_servers);
        rtc_configuration
    })
}

fn passphrase_listener(tx: Sender<String>, pc: RtcPeerConnection) {
    let listener_callback = Closure::<dyn FnMut()>::new(move || {
        let passphrase = document()
            .get_element_by_id("passphrase")
            .expect("should have #passphrase on the page")
            .dyn_ref::<HtmlInputElement>()
            .expect("#passphrase should be an `HtmlInputElement`")
            .value();

        let mut tx = tx.clone();
        let pc = pc.clone();
        spawn_local(async move {
            // Send passphrase.
            tx.send(serde_json::to_string(&Event::Passphrase(passphrase)).unwrap())
                .await
                .unwrap();
            log!("successfully sent passphrase.");

            handle_local_stream(&pc).await.unwrap();
        });
    });
    document()
        .get_element_by_id("passphrase-form")
        .expect("should have #passphrase-form on the page")
        .dyn_ref::<HtmlFormElement>()
        .expect("#passphrase-form should be an `HtmlFormElement`")
        .add_event_listener_with_callback("submit", listener_callback.as_ref().unchecked_ref())
        .unwrap();
    listener_callback.forget();
}
