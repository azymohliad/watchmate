use crate::bt;
use std::{sync::Arc, str::FromStr};
use zbus::Connection;

use futures::{pin_mut, StreamExt};
use mpris2_zbus::{media_player::MediaPlayer, player::PlaybackStatus, metadata::Metadata};

const VOLUME_STEP: f64 = 0.1;

pub async fn run_media_player_control_session(infinitime: Arc<bt::InfiniTime>) {
    let connection = Connection::session().await.unwrap();
	let media_players = MediaPlayer::new_all(&connection).await.unwrap();
    if let Some(media_player) = media_players.first() {
        let player = media_player.player().await.unwrap();
        let control_event_stream = infinitime.get_media_player_events_stream().await.unwrap();
        let mut playback_status_stream = player.receive_playback_status_changed().await;
        let mut metadata_stream = player.receive_metadata_changed().await;
        pin_mut!(control_event_stream);
        loop {
            tokio::select! {
                Some(event) = control_event_stream.next() => {
                    match event {
                        bt::MediaPlayerEvent::AppOpenned => (),
                        bt::MediaPlayerEvent::Play => player.play().await.unwrap(),
                        bt::MediaPlayerEvent::Pause => player.pause().await.unwrap(),
                        bt::MediaPlayerEvent::Next => player.next().await.unwrap(),
                        bt::MediaPlayerEvent::Previous => player.previous().await.unwrap(),
                        bt::MediaPlayerEvent::VolumeUp => {
                            let volume = player.volume().await.unwrap();
                            player.set_volume(1.0f64.min(volume + VOLUME_STEP)).await.unwrap();
                        }
                        bt::MediaPlayerEvent::VolumeDown => {
                            let volume = player.volume().await.unwrap();
                            player.set_volume(0.0f64.max(volume - VOLUME_STEP)).await.unwrap();
                        }
                    }
                }
                Some(property) = playback_status_stream.next() => {
                    let status = PlaybackStatus::from_str(&property.get().await.unwrap()).unwrap();
                    let is_playing = status == PlaybackStatus::Playing;
                    infinitime.write_media_player_status(is_playing).await.unwrap();
                }
                Some(property) = metadata_stream.next() => {
                    let metadata = Metadata::from(property.get().await.unwrap());
                    if let Some(artists) = metadata.artists() {
                        infinitime.write_media_player_artist(&artists[0]).await.unwrap();
                    }
                    if let Some(album) = metadata.album() {
                        infinitime.write_media_player_album(&album).await.unwrap();
                    }
                    if let Some(title) = metadata.title() {
                        infinitime.write_media_player_track(&title).await.unwrap();
                    }
                }
                else => break,
            }
        }
    }
}
