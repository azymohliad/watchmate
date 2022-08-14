use crate::bt;
use std::str::FromStr;
use futures::{pin_mut, StreamExt};
use anyhow::Result;
use zbus::Connection;
use mpris2_zbus::{media_player::MediaPlayer, player::{Player, PlaybackStatus, LoopStatus}, metadata::Metadata};

const VOLUME_STEP: f64 = 0.1;


pub async fn get_players(connection: &Connection) -> Result<Vec<MediaPlayer>> {
    let players = MediaPlayer::new_all(connection).await?;
    // Cache identities
    for player in &players {
        player.identity().await?;
    }
    Ok(players)
}

pub async fn update_track_metadata(metadata: &Metadata, infinitime: &bt::InfiniTime) -> Result<()> {
    match metadata.artists().as_ref().map(|s| s.first()).flatten() {
        Some(artist) => infinitime.write_mp_artist(artist).await?,
        None => infinitime.write_mp_artist("Unknown Artist").await?,
    }
    match metadata.album() {
        Some(album) => infinitime.write_mp_album(&album).await?,
        None => infinitime.write_mp_album("Unknown Album").await?,
    }
    match metadata.title() {
        Some(track) => infinitime.write_mp_track(&track).await?,
        None => infinitime.write_mp_track("Unknown Track").await?,
    }
    let length = metadata.length().unwrap_or_default().as_seconds_f32() as u32;
    infinitime.write_mp_duration(length).await?;
    Ok(())
}

pub async fn update_player_info(player: &Player, infinitime: &bt::InfiniTime) -> Result<()> {
    if let Ok(status) = player.playback_status().await {
        let is_playing = status == PlaybackStatus::Playing;
        infinitime.write_mp_playback_status(is_playing).await?;
    }
    if let Ok(status) = player.loop_status().await {
        let repeat = status == Some(LoopStatus::Track);
        infinitime.write_mp_repeat(repeat).await?;
    }
    if let Ok(shuffle) = player.shuffle().await {
        infinitime.write_mp_shuffle(shuffle.unwrap_or(false)).await?;
    }
    if let Ok(position) = player.position().await {
        let position = position.unwrap_or_default().as_seconds_f32() as u32;
        infinitime.write_mp_position(position).await?;
    }
    if let Ok(Some(rate)) = player.rate().await {
        if rate != 0.0 {
            infinitime.write_mp_playback_speed(rate as f32).await?;
        }
    }
    if let Ok(metadata) = player.metadata().await {
        update_track_metadata(&metadata, infinitime).await?;
    }
    Ok(())
}

pub async fn run_session(media_player: &MediaPlayer, infinitime: &bt::InfiniTime) -> Result<()> {
    let player = media_player.player().await?;

    // Query player capabilities
    let can_go_next = player.can_go_next().await?;
    let can_go_previous = player.can_go_previous().await?;
    let can_pause = player.can_pause().await?;
    let can_play = player.can_play().await?;

    // Send initial player info to the watch
    update_player_info(&player, infinitime).await?;

    // Obtain even streams
    let mut playback_status_stream = player.receive_playback_status_changed().await;
    let mut loop_status_stream = player.receive_loop_status_changed().await;
    let mut shuffle_stream = player.receive_shuffle_changed().await;
    let mut position_stream = player.receive_position_changed().await;
    let mut rate_stream = player.receive_rate_changed().await;
    let mut metadata_stream = player.receive_metadata_changed().await;
    let control_event_stream = infinitime.get_media_player_events_stream().await?;
    pin_mut!(control_event_stream);

    // Process events
    println!("Media Player Control session started for: {}", media_player.identity().await?);
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
                infinitime.write_mp_playback_status(is_playing).await?;
            }
            Some(property) = loop_status_stream.next() => {
                let status = LoopStatus::from_str(&property.get().await?)?;
                let repeat = status == LoopStatus::Track;
                infinitime.write_mp_repeat(repeat).await?;
            }
            Some(property) = shuffle_stream.next() => {
                let shuffle = property.get().await?;
                infinitime.write_mp_shuffle(shuffle).await?;
            }
            Some(property) = position_stream.next() => {
                let position = (property.get().await? / 1_000_000) as u32;
                infinitime.write_mp_position(position).await?;
            }
            Some(property) = rate_stream.next() => {
                let rate = property.get().await? as f32;
                infinitime.write_mp_playback_speed(rate).await?;
            }
            Some(property) = metadata_stream.next() => {
                let metadata = Metadata::from(property.get().await?);
                update_track_metadata(&metadata, infinitime).await?;
            }
            else => break,
        }
    }
    Ok(())
}
