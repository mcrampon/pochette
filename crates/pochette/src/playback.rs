//! Playing an album straight from its cover: the round button over album tiles.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use qobuz::{Client, models::Album};
use relm4::gtk::{self, prelude::*};

use crate::{
    favorites::Favorites,
    player::{Player, State},
};

/// What the buttons over a cover do: play the album, add it to the favorites.
#[derive(Clone)]
pub struct CoverActions {
    pub favorites: Favorites,
    pub playback: Playback,
}

/// Plays albums from their covers. Cheap to clone: every clone is the same.
#[derive(Clone)]
pub struct Playback(Rc<Inner>);

struct Inner {
    player: Player,
    client: RefCell<Option<Client>>,
    /// The buttons to update when what's playing changes.
    buttons: RefCell<Vec<Weak<ButtonInner>>>,
    /// The album playing, and whether it's playing rather than paused, as last shown.
    shown: RefCell<(Option<String>, bool)>,
}

impl Playback {
    pub fn new(player: Player) -> Self {
        let playback = Self(Rc::new(Inner {
            player,
            client: RefCell::default(),
            buttons: RefCell::default(),
            shown: RefCell::default(),
        }));

        let weak = Rc::downgrade(&playback.0);
        let mut status = playback.0.player.status();
        relm4::spawn_local(async move {
            while status.changed().await.is_ok() {
                let Some(inner) = weak.upgrade() else {
                    break;
                };
                let now = {
                    let status = status.borrow_and_update();
                    let playing = matches!(status.state, State::Playing | State::Loading);
                    (status.album_id.clone(), playing)
                };
                if *inner.shown.borrow() != now {
                    inner.shown.replace(now);
                    Self(inner).refresh_buttons();
                }
            }
        });
        playback
    }

    /// Play as the user `client` signs in as, or not at all.
    pub fn set_client(&self, client: Option<Client>) {
        self.0.client.replace(client);
    }

    /// Whether `album_id` is the album playing, rather than paused or stopped.
    fn is_playing(&self, album_id: &str) -> bool {
        let shown = self.0.shown.borrow();
        shown.1 && shown.0.as_deref() == Some(album_id)
    }

    /// Play `album` from the start, or pause or resume it if it's the album playing.
    fn play(&self, album: &Album) {
        let player = self.0.player.clone();
        if player.status().borrow().is_current(&album.id) {
            player.toggle_pause();
            return;
        }
        let Some(client) = self.0.client.borrow().clone() else {
            return;
        };
        // Covers only know about the album; playing it needs its tracks.
        let id = album.id.clone();
        relm4::spawn(async move {
            match client.album(&id).await {
                Ok(album) => player.play_album(client, album, None),
                Err(err) => tracing::warn!("cannot load album {id} to play it: {err}"),
            }
        });
    }

    fn register(&self, button: &Rc<ButtonInner>) {
        let mut buttons = self.0.buttons.borrow_mut();
        buttons.retain(|button| button.strong_count() > 0);
        buttons.push(Rc::downgrade(button));
    }

    fn refresh_buttons(&self) {
        let buttons: Vec<Rc<ButtonInner>> = self
            .0
            .buttons
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for button in buttons {
            PlayButton(button).refresh();
        }
    }
}

/// A round button over a cover, that shows on hover and plays the album, or pauses it.
#[derive(Clone)]
pub struct PlayButton(Rc<ButtonInner>);

struct ButtonInner {
    button: gtk::Button,
    album: RefCell<Option<Rc<Album>>>,
    playback: RefCell<Option<Playback>>,
}

impl PlayButton {
    pub fn new() -> Self {
        let button = gtk::Button::builder()
            .icon_name("media-playback-start-symbolic")
            .css_classes(["pochette-cover-play", "circular", "suggested-action"])
            .halign(gtk::Align::Start)
            .valign(gtk::Align::End)
            .margin_start(8)
            .margin_bottom(8)
            .visible(false)
            .build();
        let play = Self(Rc::new(ButtonInner {
            button,
            album: RefCell::default(),
            playback: RefCell::default(),
        }));
        let weak = Rc::downgrade(&play.0);
        play.0.button.connect_clicked(move |_| {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            let album = inner.album.borrow().clone();
            let playback = inner.playback.borrow().clone();
            if let (Some(album), Some(playback)) = (album, playback) {
                playback.play(&album);
            }
        });
        play
    }

    pub fn widget(&self) -> &gtk::Button {
        &self.0.button
    }

    /// Be the button of `album`, or of none.
    pub fn show(&self, album: Option<Rc<Album>>, playback: &Playback) {
        if self.0.playback.borrow().is_none() {
            self.0.playback.replace(Some(playback.clone()));
            playback.register(&self.0);
        }
        self.0.album.replace(album);
        self.refresh();
    }

    /// Be the button of no album, as the tile was recycled.
    pub fn clear(&self) {
        self.0.album.replace(None);
        self.refresh();
    }

    fn refresh(&self) {
        let inner = &self.0;
        let album = inner.album.borrow();
        let playback = inner.playback.borrow();
        let (Some(album), Some(playback)) = (album.as_ref(), playback.as_ref()) else {
            inner.button.set_visible(false);
            return;
        };
        let playing = playback.is_playing(&album.id);
        inner.button.set_icon_name(if playing {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        });
        inner
            .button
            .set_tooltip_text(Some(if playing { "Pause" } else { "Play" }));
        inner.button.set_visible(album.streamable);
    }
}
