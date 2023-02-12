use super::{uuids, InfiniTime};
use anyhow::Result;
use futures::{Stream, StreamExt};

#[derive(Debug)]
pub enum MediaPlayerEvent {
    AppOpenned,
    Play,
    Pause,
    Next,
    Previous,
    VolumeUp,
    VolumeDown,
}

impl MediaPlayerEvent {
    fn from_raw(v: u8) -> Option<Self> {
        match v {
            0xe0 => Some(MediaPlayerEvent::AppOpenned),
            0x00 => Some(MediaPlayerEvent::Play),
            0x01 => Some(MediaPlayerEvent::Pause),
            0x03 => Some(MediaPlayerEvent::Next),
            0x04 => Some(MediaPlayerEvent::Previous),
            0x05 => Some(MediaPlayerEvent::VolumeUp),
            0x06 => Some(MediaPlayerEvent::VolumeDown),
            _ => None,
        }
    }
}


impl InfiniTime {
    pub async fn get_media_player_events_stream(&self) -> Result<impl Stream<Item = MediaPlayerEvent>> {
        let stream = self.chr(&uuids::CHR_MP_EVENTS)?.notify().await?;
        Ok(stream.filter_map(|v| async move { MediaPlayerEvent::from_raw(v[0]) }))
    }

    pub async fn write_mp_artist(&self, artist: &str) -> Result<()> {
        Ok(self.chr(&uuids::CHR_MP_ARTIST)?.write(artist.as_ref()).await?)
    }

    pub async fn write_mp_album(&self, album: &str) -> Result<()> {
        Ok(self.chr(&uuids::CHR_MP_ALBUM)?.write(album.as_ref()).await?)
    }

    pub async fn write_mp_track(&self, track: &str) -> Result<()> {
        Ok(self.chr(&uuids::CHR_MP_TRACK)?.write(track.as_ref()).await?)
    }

    pub async fn write_mp_playback_status(&self, playing: bool) -> Result<()> {
        Ok(self.chr(&uuids::CHR_MP_STATUS)?.write(&[u8::from(playing)]).await?)
    }

    pub async fn write_mp_position(&self, position: u32) -> Result<()> {
        Ok(self.chr(&uuids::CHR_MP_POSITION)?.write(&position.to_be_bytes()).await?)
    }

    pub async fn write_mp_duration(&self, duration: u32) -> Result<()> {
        Ok(self.chr(&uuids::CHR_MP_DURATION)?.write(&duration.to_be_bytes()).await?)
    }

    pub async fn write_mp_playback_speed(&self, speed: f32) -> Result<()> {
        let percentage = (speed * 100.0) as u32;
        Ok(self.chr(&uuids::CHR_MP_SPEED)?.write(&percentage.to_be_bytes()).await?)
    }

    pub async fn write_mp_repeat(&self, repeat: bool) -> Result<()> {
        Ok(self.chr(&uuids::CHR_MP_REPEAT)?.write(&[u8::from(repeat)]).await?)
    }

    pub async fn write_mp_shuffle(&self, shuffle: bool) -> Result<()> {
        Ok(self.chr(&uuids::CHR_MP_SHUFFLE)?.write(&[u8::from(shuffle)]).await?)
    }
}