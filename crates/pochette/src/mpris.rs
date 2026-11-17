//! Tells the desktop what's playing, and takes its commands, through MPRIS: the D-Bus
//! standard that GNOME, KDE Plasma and the others all read for their media controls,
//! the lock screen, and media keys.
//!
//! The MPRIS server isn't thread-safe and runs on Tokio, so it gets a thread of its own.
//! It only needs the player, which can be driven from any thread.

use std::time::{Duration, Instant};

use mpris_server::{Metadata, PlaybackStatus, Time, TrackId};
use qobuz::models::Album;

use crate::{
    covers,
    player::{Player, State, Status},
};

/// The bus name is `org.mpris.MediaPlayer2.` and this.
const BUS_NAME: &str = "Pochette";

/// How often the position the desktop shows is brought up to date.
const POSITION_EVERY: Duration = Duration::from_secs(1);

/// A jump in the position bigger than this, while playing, is a seek.
const SEEK_THRESHOLD: Duration = Duration::from_millis(1500);

/// Start telling the desktop about `player`. `raise` brings the window to the front,
/// from the MPRIS thread.
pub fn start(player: Player, raise: impl Fn() + Send + 'static) {
    let started = std::thread::Builder::new()
        .name("mpris".to_owned())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    tracing::warn!("cannot start telling the desktop what's playing: {err}");
                    return;
                }
            };
            let local = tokio::task::LocalSet::new();
            if let Err(err) = local.block_on(&runtime, serve(player, raise)) {
                tracing::warn!("cannot tell the desktop what's playing: {err}");
            }
        });
    if let Err(err) = started {
        tracing::warn!("cannot start telling the desktop what's playing: {err}");
    }
}

async fn serve(player: Player, raise: impl Fn() + 'static) -> mpris_server::zbus::Result<()> {
    let mpris = mpris_server::Player::builder(BUS_NAME)
        .identity("Pochette")
        .desktop_entry(crate::APP_ID)
        .can_raise(true)
        .can_control(true)
        .build()
        .await?;
    connect_commands(&mpris, &player, raise);
    tokio::task::spawn_local(mpris.run());

    let mut status = player.status();
    let mut ticker = tokio::time::interval(POSITION_EVERY);
    // The position last told, and when, to tell a seek from the time passing.
    let mut last_position: Option<(Duration, Instant)> = None;
    loop {
        tokio::select! {
            changed = status.changed() => {
                if changed.is_err() {
                    break;
                }
                let current = status.borrow_and_update().clone();
                show(&mpris, &current).await?;
                last_position = None;
            }
            _ = ticker.tick() => {
                let playing = status.borrow().state == State::Playing;
                let position = player.position().unwrap_or_default();
                mpris.set_position(time(position));
                if playing
                    && let Some((before, at)) = last_position
                {
                    let expected = before + at.elapsed();
                    if position.abs_diff(expected) > SEEK_THRESHOLD {
                        mpris.seeked(time(position)).await?;
                    }
                }
                last_position = playing.then(|| (position, Instant::now()));
            }
        }
    }
    Ok(())
}

/// Do what the desktop asks.
fn connect_commands(mpris: &mpris_server::Player, player: &Player, raise: impl Fn() + 'static) {
    let state = |player: &Player| player.status().borrow().state;

    let p = player.clone();
    mpris.connect_play_pause(move |_| p.toggle_pause());
    let p = player.clone();
    mpris.connect_play(move |_| {
        if state(&p) == State::Paused {
            p.toggle_pause();
        }
    });
    let p = player.clone();
    mpris.connect_pause(move |_| {
        if matches!(state(&p), State::Playing | State::Loading) {
            p.toggle_pause();
        }
    });
    let p = player.clone();
    mpris.connect_stop(move |_| p.stop());
    let p = player.clone();
    mpris.connect_next(move |_| p.next());
    let p = player.clone();
    mpris.connect_previous(move |_| p.previous());
    let p = player.clone();
    mpris.connect_seek(move |_, offset| {
        let position = p.position().unwrap_or_default();
        let target = if offset.is_negative() {
            position.saturating_sub(duration(offset.abs()))
        } else {
            position + duration(offset)
        };
        p.seek(target);
    });
    let p = player.clone();
    mpris.connect_set_position(move |_, track_id, position| {
        // Only for the track playing: the desktop may ask after it changed.
        let current = p.status().borrow().track_id;
        if current.is_some_and(|id| *track_id == track_path(id)) && !position.is_negative() {
            p.seek(duration(position));
        }
    });
    mpris.connect_raise(move |_| raise());
}

/// Show what's playing, and what can be done about it.
async fn show(mpris: &mpris_server::Player, status: &Status) -> mpris_server::zbus::Result<()> {
    let playback = match status.state {
        State::Stopped => PlaybackStatus::Stopped,
        State::Paused => PlaybackStatus::Paused,
        State::Playing | State::Loading => PlaybackStatus::Playing,
    };
    let active = status.state != State::Stopped;
    mpris.set_metadata(metadata(status)).await?;
    mpris.set_playback_status(playback).await?;
    mpris.set_can_play(active).await?;
    mpris.set_can_pause(active).await?;
    mpris.set_can_seek(active).await?;
    mpris.set_can_go_previous(active).await?;
    mpris.set_can_go_next(status.has_next()).await?;
    Ok(())
}

fn metadata(status: &Status) -> Metadata {
    let (Some(track), Some(album)) = (status.track(), status.album.as_deref()) else {
        return Metadata::new();
    };
    let artist = track
        .performer
        .as_ref()
        .map_or(album.artist.name.as_str(), |performer| {
            performer.name.as_str()
        });
    let mut builder = Metadata::builder()
        .trackid(track_path(track.id))
        .title(track.full_title())
        .artist([artist])
        .album(album.full_title())
        .album_artist([album.artist.name.as_str()])
        .length(time(Duration::from_secs(track.duration.into())))
        .track_number(i32::try_from(track.track_number).unwrap_or_default())
        .disc_number(i32::try_from(track.media_number).unwrap_or_default());
    if let Some(art) = art_url(album) {
        builder = builder.art_url(art);
    }
    builder.build()
}

/// The cover, from the disk cache when it's there, as desktops show local files best.
fn art_url(album: &Album) -> Option<String> {
    let url = album.image.large.as_ref().or(album.image.small.as_ref())?;
    Some(
        covers::cached_file(url)
            .map_or_else(|| url.clone(), |path| format!("file://{}", path.display())),
    )
}

/// The D-Bus object path that names a track.
fn track_path(track_id: u64) -> TrackId {
    TrackId::try_from(format!("/io/github/mcrampon/Pochette/track/{track_id}"))
        .unwrap_or(TrackId::NO_TRACK)
}

fn time(duration: Duration) -> Time {
    Time::from_micros(i64::try_from(duration.as_micros()).unwrap_or(i64::MAX))
}

fn duration(time: Time) -> Duration {
    Duration::from_micros(u64::try_from(time.as_micros()).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_tracks_with_valid_paths() {
        assert_eq!(
            track_path(446_392_172).as_str(),
            "/io/github/mcrampon/Pochette/track/446392172"
        );
    }

    #[test]
    fn converts_times_both_ways() {
        let position = Duration::from_millis(83_500);
        assert_eq!(duration(time(position)), position);
    }
}
