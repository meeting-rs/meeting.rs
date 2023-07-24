use crate::listener::{
    get_element_by_id, sharing_option_listener, track_mute_listener, UserSharingOption,
};

use futures::{
    channel::mpsc::{channel, Receiver},
    StreamExt,
};
use gloo_console::log;
use gloo_utils::window;
use js_sys::Array;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    DisplayMediaStreamConstraints, HtmlMediaElement, MediaStream, MediaStreamConstraints,
    MediaStreamTrack, RtcPeerConnection,
};

pub(crate) async fn init(pc: &RtcPeerConnection) -> Result<(), JsValue> {
    let (tx, rx) = channel(1);
    sharing_option_listener("option-media".into(), tx.clone());
    sharing_option_listener("option-screen".into(), tx);
    handle_local_stream(pc, rx).await
}

async fn handle_local_stream(
    pc: &RtcPeerConnection,
    mut rx: Receiver<UserSharingOption>,
) -> Result<(), JsValue> {
    // We receive the first message since there will only be one user sharing option.
    let local_streams = match rx.next().await.unwrap() {
        UserSharingOption::Media => {
            let stream = get_user_media(true).await?;
            vec![stream]
        }
        UserSharingOption::Screen => {
            let media_stream = get_user_media(false).await?;
            let display_stream = get_display_media().await?;
            vec![media_stream, display_stream]
        }
    };
    // Clean channel.
    rx.close();

    local_streams.iter().for_each(|local_stream| {
        local_stream
            .get_tracks()
            .for_each(&mut |track: JsValue, _, _| {
                let track = track.dyn_into().unwrap();
                pc.add_track_0(&track, local_stream);
                log!("added a local track.");

                if track.kind() == "video" {
                    display_local_video(&track);
                }
                track_mute_listener(track);
            });
    });

    Ok(())
}

async fn get_user_media(enable_video: bool) -> Result<MediaStream, JsValue> {
    Ok(MediaStream::from(
        JsFuture::from(
            window()
                .navigator()
                .media_devices()?
                .get_user_media_with_constraints(
                    MediaStreamConstraints::new()
                        .video(&JsValue::from_bool(enable_video))
                        .audio(&JsValue::from_bool(true)),
                )?,
        )
        .await?,
    ))
}

async fn get_display_media() -> Result<MediaStream, JsValue> {
    Ok(MediaStream::from(
        JsFuture::from(
            window()
                .navigator()
                .media_devices()?
                .get_display_media_with_constraints(&DisplayMediaStreamConstraints::default())?,
        )
        .await?,
    ))
}

fn display_local_video(track: &MediaStreamTrack) {
    let video_stream = {
        let tracks = Array::new();
        tracks.push(track);
        MediaStream::new_with_tracks(&tracks.into()).unwrap()
    };
    get_element_by_id::<HtmlMediaElement>("local-video")
        .expect("#local-video should be an `HtmlVideoElement`")
        .set_src_object(video_stream.dyn_ref());
}
