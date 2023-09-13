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
    DisplayMediaStreamConstraints, HtmlMediaElement, MediaDeviceInfo, MediaDeviceKind,
    MediaDevices, MediaStream, MediaStreamConstraints, MediaStreamTrack, RtcPeerConnection,
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
    let local_stream = match rx.next().await.unwrap() {
        UserSharingOption::Media => {
            get_user_media(
                check_media_device(&MediaDeviceKind::Videoinput).await?,
                check_media_device(&MediaDeviceKind::Audioinput).await?,
            )
            .await?
        }
        UserSharingOption::Screen => {
            let media_stream = get_user_media(
                false,
                check_media_device(&MediaDeviceKind::Audioinput).await?,
            )
            .await?;
            let display_stream = get_display_media().await?;
            let tracks = media_stream
                .get_tracks()
                .concat(&display_stream.get_tracks());
            MediaStream::new_with_tracks(&tracks)?
        }
    };
    // Clean channel.
    rx.close();

    local_stream
        .get_tracks()
        .for_each(&mut |track: JsValue, _, _| {
            let track = track.dyn_into().unwrap();
            pc.add_track_0(&track, &local_stream);
            log!("added a local track.");

            if track.kind() == "video" {
                display_local_video(&track);
            }
            track_mute_listener(track);
        });

    Ok(())
}

async fn get_user_media(enable_video: bool, enable_audio: bool) -> Result<MediaStream, JsValue> {
    Ok(MediaStream::from(
        JsFuture::from(
            media_devices()?.get_user_media_with_constraints(
                MediaStreamConstraints::new()
                    .video(&JsValue::from_bool(enable_video))
                    .audio(&JsValue::from_bool(enable_audio)),
            )?,
        )
        .await?,
    ))
}

async fn get_display_media() -> Result<MediaStream, JsValue> {
    Ok(MediaStream::from(
        JsFuture::from(media_devices()?.get_display_media_with_constraints(
            &DisplayMediaStreamConstraints::new().audio(&JsValue::from_bool(true)),
        )?)
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

async fn check_media_device(kind: &MediaDeviceKind) -> Result<bool, JsValue> {
    Ok(
        Array::from(&JsFuture::from(media_devices()?.enumerate_devices()?).await?)
            .iter()
            .any(|media_device_info: JsValue| {
                media_device_info
                    .dyn_into::<MediaDeviceInfo>()
                    .unwrap()
                    .kind()
                    .eq(kind)
            }),
    )
}

fn media_devices() -> Result<MediaDevices, JsValue> {
    window().navigator().media_devices()
}
