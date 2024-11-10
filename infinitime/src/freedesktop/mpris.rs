use super::super::bt;
use anyhow::Result;
use futures::{pin_mut, stream, Stream, StreamExt};
use mpris2_zbus::{
    metadata::Metadata,
    player::{LoopStatus, PlaybackStatus, Player},
};
use std::str::FromStr;
use zbus::{fdo::DBusProxy, names::OwnedBusName, Connection};

pub use mpris2_zbus::media_player::MediaPlayer;

const VOLUME_STEP: f64 = 0.1;

#[derive(Debug)]
pub enum PlayersListEvent {
    PlayerAdded(OwnedBusName),
    PlayerRemoved(OwnedBusName),
}

pub async fn get_players_update_stream(
    connection: &Connection,
) -> Result<impl Stream<Item = PlayersListEvent>> {
    let known_players = MediaPlayer::available_players(&connection).await?;
    let known_players_events = stream::iter(known_players).map(PlayersListEvent::PlayerAdded);

    let new_events = DBusProxy::new(connection).await?
        .receive_name_owner_changed().await?
        .filter_map(|msg| async move {
            msg.args().ok().and_then(|args| {
                if args.name.starts_with("org.mpris.MediaPlayer2") {
                    match (args.old_owner.as_ref(), args.new_owner.as_ref()) {
                        (Some(_), None) => Some(PlayersListEvent::PlayerRemoved(args.name.into())),
                        (None, Some(_)) => Some(PlayersListEvent::PlayerAdded(args.name.into())),
                        _ => None,
                    }
                } else {
                    None
                }
            })
        });

    Ok(known_players_events.chain(new_events))
}

pub async fn update_track_metadata(metadata: &Metadata, infinitime: &bt::InfiniTime) -> Result<()> {
    let artists = metadata.artists();
    let artist = artists
        .as_ref()
        .and_then(|s| s.first())
        .map(String::as_str)
        .unwrap_or("Unknown Artist");
    log::debug!("Artist: {}", artist);
    infinitime.write_mp_artist(artist).await?;

    let album = metadata.album();
    let album = album
        .as_ref()
        .map(String::as_str)
        .unwrap_or("Unknown Album");
    log::debug!("Album: {}", album);
    infinitime.write_mp_album(album).await?;

    let title = metadata.title();
    let track = title
        .as_ref()
        .map(String::as_str)
        .unwrap_or("Unknown Track");
    log::debug!("Track: {}", track);
    infinitime.write_mp_track(&track).await?;

    let length = metadata.length().unwrap_or_default().as_seconds_f32() as u32;
    log::debug!("Length: {}", length);
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
        infinitime
            .write_mp_shuffle(shuffle.unwrap_or(false))
            .await?;
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

pub async fn run_control_session(
    media_player: &MediaPlayer,
    infinitime: &bt::InfiniTime,
) -> Result<()> {
    let player = media_player.player().await?;

    // Obtain even streams
    log::debug!("Creating event streams...");
    let mut playback_status_stream = player.receive_playback_status_changed().await;
    let mut loop_status_stream = player.receive_loop_status_changed().await;
    let mut shuffle_stream = player.receive_shuffle_changed().await;
    let mut position_stream = player.receive_position_changed().await;
    let mut rate_stream = player.receive_rate_changed().await;
    let mut metadata_stream = player.receive_metadata_changed().await;
    let mut can_go_next_stream = player.receive_can_go_next_changed().await;
    let mut can_go_previous_stream = player.receive_can_go_previous_changed().await;
    let mut can_pause_stream = player.receive_can_pause_changed().await;
    let mut can_play_stream = player.receive_can_play_changed().await;
    let control_event_stream = infinitime.get_media_player_events_stream().await?;
    pin_mut!(control_event_stream);

    // Query player capabilities
    log::debug!("Querying player capabilities...");
    let mut can_go_next = player.can_go_next().await?;
    let mut can_go_previous = player.can_go_previous().await?;
    let mut can_pause = player.can_pause().await?;
    let mut can_play = player.can_play().await?;

    // Send initial player info to the watch
    log::debug!("Sending player info to the watch...");
    update_player_info(&player, infinitime).await?;

    // Process events
    log::info!(
        "Media Player Control session started for: {}",
        media_player.identity().await?
    );
    loop {
        tokio::select! {
            Some(event) = control_event_stream.next() => {
                log::debug!("Control event: {:?}", event);
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
                log::debug!("Playback status: {:?}", status);
                let is_playing = status == PlaybackStatus::Playing;
                infinitime.write_mp_playback_status(is_playing).await?;
            }
            Some(property) = loop_status_stream.next() => {
                let status = LoopStatus::from_str(&property.get().await?)?;
                log::debug!("Loop status: {:?}", status);
                let repeat = status == LoopStatus::Track;
                infinitime.write_mp_repeat(repeat).await?;
            }
            Some(property) = shuffle_stream.next() => {
                let shuffle = property.get().await?;
                log::debug!("Shuffle: {:?}", shuffle);
                infinitime.write_mp_shuffle(shuffle).await?;
            }
            Some(property) = position_stream.next() => {
                let position = (property.get().await? / 1_000_000) as u32;
                log::debug!("Position: {:?}", position);
                infinitime.write_mp_position(position).await?;
            }
            Some(property) = rate_stream.next() => {
                let rate = property.get().await? as f32;
                log::debug!("Rate: {:?}", rate);
                infinitime.write_mp_playback_speed(rate).await?;
            }
            Some(property) = metadata_stream.next() => {
                let metadata = Metadata::from(property.get().await?);
                log::debug!("Metadata: {:?}", metadata);
                update_track_metadata(&metadata, infinitime).await?;
            }
            Some(property) = can_go_next_stream.next() => {
                can_go_next = property.get().await?;
                log::debug!("Supports next: {:?}", can_go_next);
            }
            Some(property) = can_go_previous_stream.next() => {
                can_go_previous = property.get().await?;
                log::debug!("Supports previous: {:?}", can_go_previous);
            }
            Some(property) = can_pause_stream.next() => {
                can_pause = property.get().await?;
                log::debug!("Supports pause: {:?}", can_pause);
            }
            Some(property) = can_play_stream.next() => {
                can_play = property.get().await?;
                log::debug!("Supports play: {:?}", can_play);
            }
            else => break,
        }
    }
    Ok(())
}
