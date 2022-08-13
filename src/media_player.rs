use crate::bt;
use std::str::FromStr;
use futures::{pin_mut, StreamExt};
use anyhow::Result;
use zbus::Connection;
use mpris2_zbus::{media_player::MediaPlayer, player::PlaybackStatus, metadata::Metadata};

const VOLUME_STEP: f64 = 0.1;


pub async fn get_players(connection: &Connection) -> Result<Vec<MediaPlayer>> {
    let players = MediaPlayer::new_all(connection).await?;
    // Cache identities
    for player in &players {
        player.identity().await?;
    }
    Ok(players)
}

pub async fn run_session(media_player: &MediaPlayer, infinitime: &bt::InfiniTime) -> Result<()> {
    let player = media_player.player().await?;
    let can_go_next = player.can_go_next().await?;
    let can_go_previous = player.can_go_previous().await?;
    let can_pause = player.can_pause().await?;
    let can_play = player.can_play().await?;

    let control_event_stream = infinitime.get_media_player_events_stream().await?;
    let mut playback_status_stream = player.receive_playback_status_changed().await;
    let mut metadata_stream = player.receive_metadata_changed().await;
    pin_mut!(control_event_stream);

    loop {
        tokio::select! {
            Some(event) = control_event_stream.next() => {
                match event {
                    bt::MediaPlayerEvent::AppOpenned => (),
                    bt::MediaPlayerEvent::Play => if can_play {
                        player.play().await?;
                    }
                    bt::MediaPlayerEvent::Pause => if can_pause {
                        player.pause().await?;
                    }
                    bt::MediaPlayerEvent::Next => if can_go_next {
                        player.next().await?;
                    }
                    bt::MediaPlayerEvent::Previous => if can_go_previous {
                        player.previous().await?;
                    }
                    bt::MediaPlayerEvent::VolumeUp => {
                        let volume = player.volume().await?;
                        player.set_volume(1.0f64.min(volume + VOLUME_STEP)).await?;
                    }
                    bt::MediaPlayerEvent::VolumeDown => {
                        let volume = player.volume().await?;
                        player.set_volume(0.0f64.max(volume - VOLUME_STEP)).await?;
                    }
                }
            }
            Some(property) = playback_status_stream.next() => {
                let status = PlaybackStatus::from_str(&property.get().await?)?;
                let is_playing = status == PlaybackStatus::Playing;
                infinitime.write_media_player_status(is_playing).await?;
            }
            Some(property) = metadata_stream.next() => {
                let metadata = Metadata::from(property.get().await?);
                if let Some(artists) = metadata.artists() {
                    infinitime.write_media_player_artist(&artists[0]).await?;
                }
                if let Some(album) = metadata.album() {
                    infinitime.write_media_player_album(&album).await?;
                }
                if let Some(title) = metadata.title() {
                    infinitime.write_media_player_track(&title).await?;
                }
            }
            else => break,
        }
    }
    Ok(())
}
