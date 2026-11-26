//! The lyrics of the track playing, in Now Playing in place of the cover: timed lyrics follow
//! the song, the line sung lit and kept in the middle, and clicking a line plays from there.
//!
//! Lyrics are only looked up while shown, once per track.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

use relm4::{
    Component, ComponentParts, ComponentSender,
    gtk::{self, glib, graphene, prelude::*},
};

use crate::{
    lyrics::{self, Line, Lyrics, Song},
    player::{Player, State, Status},
};

/// How often the line sung is checked.
const TICK: Duration = Duration::from_millis(100);

/// How long following the song waits after the user scrolls the lyrics themselves.
const FOLLOW_PAUSE: Duration = Duration::from_secs(4);

/// How long scrolling to the line sung takes.
const GLIDE: Duration = Duration::from_millis(350);

#[derive(Debug, Clone)]
enum Shown {
    Loading,
    Lyrics(Lyrics),
    Failed,
}

pub struct LyricsView {
    player: Player,
    http: reqwest::Client,
    status: Status,
    /// Whether the view is shown: lyrics are only looked up then.
    visible: bool,
    /// The track whose lyrics are shown or loading.
    track_id: Option<u64>,
    shown: Shown,
    /// Lyrics already found, by track.
    found: HashMap<u64, Lyrics>,
    lines_box: gtk::Box,
    scroller: gtk::ScrolledWindow,
    /// The timed lines, and their labels.
    lines: Vec<Line>,
    labels: Vec<gtk::Label>,
    current: Option<usize>,
    /// Until when following the song waits, as the user scrolled.
    paused_until: Option<Instant>,
    glide: Rc<Glide>,
}

#[derive(Debug)]
pub enum LyricsViewMsg {
    SetVisible(bool),
    PlaybackChanged(Status),
    Found {
        track_id: u64,
        result: Result<Lyrics, String>,
    },
    Tick,
    /// Play from this line.
    Seek(Duration),
    /// The user scrolled the lyrics.
    Scrolled,
}

#[relm4::component(pub)]
impl Component for LyricsView {
    type Init = Player;
    type Input = LyricsViewMsg;
    type Output = ();
    type CommandOutput = ();

    view! {
        gtk::ScrolledWindow {
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_vexpand: true,
            add_css_class: "pochette-lyrics",

            add_controller = gtk::EventControllerScroll {
                set_flags: gtk::EventControllerScrollFlags::VERTICAL,
                // Seen before the scrolled window scrolls, which it still does.
                set_propagation_phase: gtk::PropagationPhase::Capture,
                connect_scroll[sender] => move |_, _, _| {
                    sender.input(LyricsViewMsg::Scrolled);
                    glib::Propagation::Proceed
                },
            },

            #[local_ref]
            lines_box -> gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 14,
                set_margin_start: 12,
                set_margin_end: 12,
            },
        }
    }

    fn init(
        player: Player,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        follow_playback(&player, &sender);
        tick(&sender);
        let model = Self {
            status: player.status().borrow().clone(),
            player,
            http: crate::account::http_client(),
            visible: false,
            track_id: None,
            shown: Shown::Loading,
            found: HashMap::new(),
            lines_box: gtk::Box::default(),
            scroller: root.clone(),
            lines: Vec::new(),
            labels: Vec::new(),
            current: None,
            paused_until: None,
            glide: Rc::default(),
        };
        let lines_box = &model.lines_box;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: LyricsViewMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            LyricsViewMsg::SetVisible(visible) => {
                self.visible = visible;
                if visible {
                    self.look_up(&sender);
                    self.follow(true);
                }
            }
            LyricsViewMsg::PlaybackChanged(status) => {
                self.status = status;
                self.look_up(&sender);
            }
            LyricsViewMsg::Found { track_id, result } => {
                if let Ok(lyrics) = &result {
                    self.found.insert(track_id, lyrics.clone());
                }
                if self.track_id == Some(track_id) {
                    self.shown = result.map_or(Shown::Failed, Shown::Lyrics);
                    self.fill(&sender);
                }
            }
            LyricsViewMsg::Tick => {
                if self.visible && self.status.state != State::Stopped {
                    // Back to following the song once the user stopped scrolling.
                    let resumed = self
                        .paused_until
                        .is_some_and(|until| Instant::now() >= until);
                    if resumed {
                        self.paused_until = None;
                    }
                    self.follow(resumed);
                }
            }
            LyricsViewMsg::Seek(at) => {
                self.player.seek(at);
                self.paused_until = None;
                self.light(lyrics::current_line(&self.lines, at + lyrics::LEAD), true);
            }
            LyricsViewMsg::Scrolled => {
                self.paused_until = Some(Instant::now() + FOLLOW_PAUSE);
                self.glide.stop();
            }
        }
    }
}

impl LyricsView {
    /// Show the lyrics of the track playing, looking them up if they aren't known yet.
    fn look_up(&mut self, sender: &ComponentSender<Self>) {
        if !self.visible || self.status.track_id == self.track_id {
            return;
        }
        self.track_id = self.status.track_id;
        let Some(track_id) = self.track_id else {
            self.shown = Shown::Lyrics(Lyrics::Missing);
            self.fill(sender);
            return;
        };
        if let Some(lyrics) = self.found.get(&track_id) {
            self.shown = Shown::Lyrics(lyrics.clone());
            self.fill(sender);
            return;
        }
        let Some(song) = self.song() else {
            self.shown = Shown::Lyrics(Lyrics::Missing);
            self.fill(sender);
            return;
        };
        self.shown = Shown::Loading;
        self.fill(sender);
        let (http, sender) = (self.http.clone(), sender.clone());
        relm4::spawn(async move {
            let result = lyrics::find(&http, &song).await.map_err(|err| {
                tracing::warn!("cannot find the lyrics of {:?}: {err}", song.title);
                err.to_string()
            });
            sender.input(LyricsViewMsg::Found { track_id, result });
        });
    }

    /// The track playing, as LRCLIB looks songs up.
    fn song(&self) -> Option<Song> {
        Some(Song::of(self.status.album.as_ref()?, self.status.track()?))
    }

    /// Fill the view with what's shown.
    fn fill(&mut self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.lines_box.first_child() {
            self.lines_box.remove(&child);
        }
        self.lines.clear();
        self.labels.clear();
        self.current = None;
        self.paused_until = None;
        self.glide.stop();
        self.scroller.vadjustment().set_value(0.0);

        match &self.shown {
            Shown::Loading => self.lines_box.append(&message_box(
                gtk::Spinner::builder()
                    .spinning(true)
                    .width_request(32)
                    .height_request(32)
                    .build()
                    .upcast_ref(),
            )),
            Shown::Failed => self.lines_box.append(&message("Can't load the lyrics")),
            Shown::Lyrics(Lyrics::Missing) => {
                self.lines_box.append(&message("No lyrics for this track"));
            }
            Shown::Lyrics(Lyrics::Instrumental) => self.lines_box.append(&message("Instrumental")),
            Shown::Lyrics(Lyrics::Plain(text)) => {
                let label = gtk::Label::builder()
                    .label(text)
                    .wrap(true)
                    .justify(gtk::Justification::Center)
                    .margin_top(12)
                    .margin_bottom(12)
                    .css_classes(["pochette-lyrics-plain"])
                    .build();
                self.lines_box.append(&label);
            }
            Shown::Lyrics(Lyrics::Synced(lines)) => {
                let lines = lines.clone();
                // Room for the first and last lines to come to the middle.
                self.lines_box.append(&spacer());
                for line in lines {
                    let label = line_label(&line, sender);
                    self.lines_box.append(&label);
                    self.lines.push(line);
                    self.labels.push(label);
                }
                self.lines_box.append(&spacer());
                self.follow(true);
            }
        }
    }

    /// Light the line sung now, and bring it to the middle unless the user is scrolling.
    fn follow(&mut self, recenter: bool) {
        if self.lines.is_empty() {
            return;
        }
        let position = self.player.position().unwrap_or_default();
        let current = lyrics::current_line(&self.lines, position + lyrics::LEAD);
        self.light(current, recenter);
    }

    fn light(&mut self, current: Option<usize>, recenter: bool) {
        let changed = current != self.current;
        if changed {
            if let Some(label) = self.current.and_then(|index| self.labels.get(index)) {
                label.remove_css_class("pochette-lyric-current");
            }
            if let Some(label) = current.and_then(|index| self.labels.get(index)) {
                label.add_css_class("pochette-lyric-current");
            }
            self.current = current;
        }
        if (changed || recenter) && self.paused_until.is_none() {
            self.center(current);
        }
    }

    /// Scroll so that the line at `index`, or the top before any line, is in the middle.
    fn center(&self, index: Option<usize>) {
        let adjustment = self.scroller.vadjustment();
        let target = index
            .and_then(|index| self.labels.get(index))
            .and_then(|label| {
                let top = label.compute_point(&self.lines_box, &graphene::Point::zero())?;
                Some(
                    f64::from(top.y()) + f64::from(label.height()) / 2.0
                        - adjustment.page_size() / 2.0,
                )
            })
            .unwrap_or(0.0);
        self.glide.to(&self.scroller, target);
    }
}

/// A smooth scroll to a point, which a new one or the user's scrolling stops.
#[derive(Default)]
struct Glide {
    tick: RefCell<Option<gtk::TickCallbackId>>,
    /// Tells the latest glide from the earlier ones, whose callback may still run once.
    count: Cell<u64>,
}

impl Glide {
    fn to(self: &Rc<Self>, scroller: &gtk::ScrolledWindow, target: f64) {
        self.stop();
        let adjustment = scroller.vadjustment();
        let target = target.clamp(
            adjustment.lower(),
            (adjustment.upper() - adjustment.page_size()).max(adjustment.lower()),
        );
        let start = adjustment.value();
        if (target - start).abs() < 1.0 {
            return;
        }
        let count = self.count.get() + 1;
        self.count.set(count);
        let began = Instant::now();
        let glide = Rc::downgrade(self);
        let id = scroller.add_tick_callback(move |_, _| {
            let Some(glide) = glide.upgrade().filter(|glide| glide.count.get() == count) else {
                return glib::ControlFlow::Break;
            };
            let progress = (began.elapsed().as_secs_f64() / GLIDE.as_secs_f64()).min(1.0);
            // Fast, then slowing down.
            let eased = 1.0 - (1.0 - progress).powi(3);
            adjustment.set_value(start + (target - start) * eased);
            if progress < 1.0 {
                glib::ControlFlow::Continue
            } else {
                glide.tick.replace(None);
                glib::ControlFlow::Break
            }
        });
        self.tick.replace(Some(id));
    }

    fn stop(&self) {
        self.count.set(self.count.get() + 1);
        if let Some(id) = self.tick.take() {
            id.remove();
        }
    }
}

fn line_label(line: &Line, sender: &ComponentSender<LyricsView>) -> gtk::Label {
    let text = if line.text.is_empty() {
        "\u{266a}"
    } else {
        line.text.as_str()
    };
    let label = gtk::Label::builder()
        .label(text)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["pochette-lyric"])
        .build();
    label.set_cursor_from_name(Some("pointer"));
    let click = gtk::GestureClick::new();
    let (at, input) = (line.at, sender.input_sender().clone());
    click.connect_released(move |_, _, _, _| input.emit(LyricsViewMsg::Seek(at)));
    label.add_controller(click);
    label
}

fn spacer() -> gtk::Box {
    gtk::Box::builder().height_request(140).build()
}

fn message(text: &str) -> gtk::Box {
    let label = gtk::Label::builder()
        .label(text)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["dim-label"])
        .build();
    message_box(label.upcast_ref())
}

/// `child` in the middle of the view.
fn message_box(child: &gtk::Widget) -> gtk::Box {
    let holder = gtk::Box::builder()
        .vexpand(true)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .margin_top(140)
        .build();
    holder.append(child);
    holder
}

/// Pass the player's status changes on to the view.
fn follow_playback(player: &Player, sender: &ComponentSender<LyricsView>) {
    let mut status = player.status();
    let sender = sender.clone();
    relm4::spawn(async move {
        while status.changed().await.is_ok() {
            let current = status.borrow_and_update().clone();
            if sender
                .input_sender()
                .send(LyricsViewMsg::PlaybackChanged(current))
                .is_err()
            {
                break;
            }
        }
    });
}

fn tick(sender: &ComponentSender<LyricsView>) {
    let input = sender.input_sender().clone();
    glib::timeout_add_local(TICK, move || {
        if input.send(LyricsViewMsg::Tick).is_ok() {
            glib::ControlFlow::Continue
        } else {
            glib::ControlFlow::Break
        }
    });
}
