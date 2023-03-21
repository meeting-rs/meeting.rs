use futures::{channel::mpsc, SinkExt, StreamExt};
use gloo_console::log;
use gloo_net::websocket::{futures::WebSocket, Message};
use gloo_utils::{document, window};
use js_sys::{Array, Error, Object, Reflect};
use protocol::{Event, IceCandidate, Role};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    HtmlMediaElement, HtmlVideoElement, MediaStream, MediaStreamConstraints, RtcConfiguration,
    RtcIceCandidate, RtcIceCandidateInit, RtcPeerConnection, RtcPeerConnectionIceEvent, RtcSdpType,
    RtcSessionDescriptionInit, RtcTrackEvent,
};

#[wasm_bindgen(start)]
pub async fn main() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let ws = WebSocket::open("ws://localhost:3000/websocket")
        .map_err(|err| Error::new(&err.to_string()))?;
    log!("WebSocket connected.");
    let (mut write, mut read) = ws.split();

    let (mut tx, mut rx) = mpsc::channel(10);
    // Write task.
    spawn_local(async move {
        while let Some(msg) = rx.next().await {
            write.send(Message::Text(msg)).await.unwrap();
        }
    });

    let pc = RtcPeerConnection::new_with_configuration(&{
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
    })?;
    log!("pc created: state:", pc.signaling_state());

    // Get local stream.
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

    document()
        .get_element_by_id("localVideo")
        .expect("should have #localVideo on the page")
        .dyn_ref::<HtmlVideoElement>()
        .expect("#localVideo should be an `HtmlVideoElement`")
        .set_src_object(Some(&local_stream));

    local_stream
        .get_tracks()
        .for_each(&mut |track: JsValue, _, _| {
            pc.add_track_0(track.dyn_ref().unwrap(), &local_stream);
            log!("added a local track.");
        });

    let tx_clone = tx.clone();
    let onicecandidate_callback =
        Closure::<dyn FnMut(_)>::new(move |ev: RtcPeerConnectionIceEvent| {
            if let Some(candidate) = ev.candidate() {
                // log!("pc.onicecandidate:", &candidate);
                let mut tx = tx_clone.clone();
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

    let pc_clone = pc.clone();
    let onconnectionstatechange_callback = Closure::<dyn FnMut()>::new(move || {
        log!("pc state:", pc_clone.ice_connection_state());
    });
    pc.set_oniceconnectionstatechange(Some(
        onconnectionstatechange_callback.as_ref().unchecked_ref(),
    ));
    onconnectionstatechange_callback.forget();

    let ontrack_callback = Closure::<dyn FnMut(_)>::new(move |ev: RtcTrackEvent| {
        let first_remote_stream = ev.streams().at(0);
        document()
            .get_element_by_id("remoteVideo")
            .expect("should have #remoteVideo on the page")
            .dyn_ref::<HtmlMediaElement>()
            .expect("#remoteVideo should be an `HtmlVideoElement`")
            .set_src_object(first_remote_stream.dyn_ref());
        log!("added remote stream.");
    });
    pc.set_ontrack(Some(ontrack_callback.as_ref().unchecked_ref()));
    ontrack_callback.forget();

    // Read task.
    let pc_clone = pc.clone();
    let mut tx_clone = tx.clone();
    spawn_local(async move {
        while let Some(Ok(Message::Text(msg))) = read.next().await {
            let event: Event = serde_json::from_str(&msg).unwrap();
            match event {
                Event::Role(role) => {
                    log!("this peer's role is:", role.to_string());
                    if let Role::Initiator = role {
                        // Send offer.
                        let offer = JsFuture::from(pc_clone.create_offer()).await.unwrap();
                        let offer_sdp = Reflect::get(&offer, &JsValue::from_str("sdp"))
                            .unwrap()
                            .as_string()
                            .unwrap();
                        // log!("offer:", &offer_sdp);

                        let mut offer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                        offer_obj.sdp(&offer_sdp);
                        let sld_promise = pc_clone.set_local_description(&offer_obj);
                        JsFuture::from(sld_promise).await.unwrap();
                        log!("pc: state:", pc_clone.signaling_state());

                        tx_clone
                            .send(serde_json::to_string(&Event::Offer(offer_sdp)).unwrap())
                            .await
                            .unwrap();
                        log!("sent an offer.");
                    }
                }
                Event::Offer(offer) => {
                    log!("received offer");
                    let mut offer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                    offer_obj.sdp(&offer);
                    let srd_promise = pc_clone.set_remote_description(&offer_obj);
                    JsFuture::from(srd_promise).await.unwrap();
                    log!("pc: state:", pc_clone.signaling_state());

                    let answer = JsFuture::from(pc_clone.create_answer()).await.unwrap();
                    let answer_sdp = Reflect::get(&answer, &JsValue::from_str("sdp"))
                        .unwrap()
                        .as_string()
                        .unwrap();
                    // log!("pc: answer:", &answer_sdp);

                    let mut answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                    answer_obj.sdp(&answer_sdp);
                    let sld_promise = pc_clone.set_local_description(&answer_obj);
                    JsFuture::from(sld_promise).await.unwrap();
                    log!("pc: state:", pc_clone.signaling_state());

                    tx_clone
                        .send(serde_json::to_string(&Event::Answer(answer_sdp)).unwrap())
                        .await
                        .unwrap();
                    log!("sent an answer.");
                }
                Event::Answer(answer) => {
                    log!("received answer");
                    let mut answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                    answer_obj.sdp(&answer);
                    let srd_promise = pc_clone.set_remote_description(&answer_obj);
                    JsFuture::from(srd_promise).await.unwrap();
                    log!("pc: state:", pc_clone.signaling_state());
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
                    let promise =
                        pc_clone.add_ice_candidate_with_opt_rtc_ice_candidate(Some(&candidate));
                    JsFuture::from(promise).await.unwrap();
                }
                Event::Error(error) => {
                    log!("An error occurred:", error);
                    return;
                }
                _ => {}
            }
        }
        log!("WebSocket Closed.")
    });

    // Send passphrase.
    tx.send(serde_json::to_string(&Event::Passphrase("some_passphrase".into())).unwrap())
        .await
        .unwrap();

    Ok(())
}
