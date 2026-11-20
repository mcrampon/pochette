//! The listening stats, beside the tabs and opened from the main menu: how much was
//! listened to, and the albums, artists and genres played most, over a month, a year or all
//! time. The albums and artists open over them.

use std::rc::Rc;

use qobuz::{Client, models::Album};
use relm4::{
    Component, ComponentParts, ComponentSender,
    gtk::{self, glib, prelude::*},
};

use crate::{
    album_tile::{self, AlbumWidgets},
    choice_picker::ChoicePicker,
    covers::VisibleCovers,
    format,
    history::{Period, Summary},
    navigation::{Destination, Navigator, NavigatorMsg},
    playback::CoverActions,
    player::Player,
};

/// How many albums, artists and genres the stats rank.
const TOP_ALBUMS: usize = 12;
const TOP: usize = 10;
/// The height of the timeline's tallest bar.
const CHART_HEIGHT: i32 = 140;
/// Under this many bars, they're this wide rather than as wide as the page lets them.
const FEW_BARS: usize = 6;
const FEW_BARS_WIDTH: i32 = 72;
/// The widest the names in the rankings make them: longer ones are cut short.
const MAX_CHARS: i32 = 28;

pub struct StatsView {
    nav: Navigator,
    actions: CoverActions,
    covers: VisibleCovers,
    /// The stats, rebuilt when the period or the plays change.
    content: gtk::Box,
    /// The period shown, none until the stats first show.
    period: Option<Period>,
    /// The years with plays, the latest first, as the year picker offers them before
    /// all time.
    years: Vec<i32>,
    /// The months of the year shown with plays, as the month picker offers them after the
    /// whole year.
    months: Vec<i32>,
    year_picker: ChoicePicker,
    month_picker: ChoicePicker,
    /// The top albums, and their tiles.
    albums: Vec<Rc<Album>>,
    tiles: Vec<AlbumWidgets>,
    /// Whether the stats show, and so follow the plays.
    shown: bool,
}

#[derive(Debug)]
pub enum StatsViewMsg {
    /// Show the stats, with the plays from the logs, which syncing may have changed.
    Show,
    /// The stats no longer show.
    Hidden,
    /// Back to the stats, closing the pages opened over them.
    Home,
    /// Open pages as the user `client` signs in as, or close them as they signed out.
    SetClient(Option<Client>),
    Nav(NavigatorMsg),
    /// Open a page over the stats, as from Now Playing.
    OpenPage(Destination),
    /// The year at this position in the year picker was picked, all time after the years.
    Year(usize),
    /// The month at this position in the month picker was picked, the whole year first.
    Month(usize),
    /// Show this period, as a bar of the timeline was clicked.
    Period(Period),
    PlaysChanged,
    /// Open the top album at this position.
    OpenTopAlbum(usize),
    OpenArtist(u64, String),
}

#[derive(Debug)]
pub enum StatsViewOutput {
    /// Qobuz no longer accepts the session.
    SessionExpired,
}

#[relm4::component(pub)]
impl Component for StatsView {
    type Init = (Player, CoverActions);
    type Input = StatsViewMsg;
    type Output = StatsViewOutput;
    type CommandOutput = ();

    view! {
        gtk::Box {
            #[local_ref]
            nav_stack -> gtk::Stack {
                #[name = "scroller"]
                add_named[Some("home")] = &gtk::ScrolledWindow {
                    set_hscrollbar_policy: gtk::PolicyType::Never,
                    set_vexpand: true,

                    #[local_ref]
                    content -> gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 36,
                        set_margin_top: 18,
                        set_margin_bottom: 48,
                    },
                },
            },
        }
    }

    fn init(
        (player, actions): (Player, CoverActions),
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let year_picker = ChoicePicker::new("Year");
        let input = sender.input_sender().clone();
        year_picker.connect_picked(move |position| input.emit(StatsViewMsg::Year(position)));
        let month_picker = ChoicePicker::new("Month");
        let input = sender.input_sender().clone();
        month_picker.connect_picked(move |position| input.emit(StatsViewMsg::Month(position)));
        let input = sender.input_sender().clone();
        actions.plays.connect_changed(move || {
            input.emit(StatsViewMsg::PlaysChanged);
            glib::ControlFlow::Continue
        });

        let model = Self {
            nav: Navigator::new(player, actions.clone(), {
                let input = sender.input_sender().clone();
                move |msg| input.emit(StatsViewMsg::Nav(msg))
            }),
            actions,
            covers: VisibleCovers::default(),
            content: gtk::Box::default(),
            period: None,
            years: Vec::new(),
            months: Vec::new(),
            year_picker,
            month_picker,
            albums: Vec::new(),
            tiles: Vec::new(),
            shown: false,
        };
        let nav_stack = model.nav.widget();
        nav_stack.set_hexpand(true);
        let content = &model.content;
        let widgets = view_output!();
        model.covers.attach(&widgets.scroller);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: StatsViewMsg, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            StatsViewMsg::Show => {
                self.shown = true;
                // Shows the stats, as the plays change.
                self.actions.plays.reload();
            }
            StatsViewMsg::Hidden => self.shown = false,
            StatsViewMsg::Home => self.nav.home(),
            StatsViewMsg::SetClient(client) => self.nav.set_client(client),
            StatsViewMsg::OpenPage(destination) => self.nav.open(destination),
            StatsViewMsg::Nav(msg) => {
                if self.nav.update(msg) {
                    let _ = sender.output(StatsViewOutput::SessionExpired);
                }
            }
            StatsViewMsg::Year(position) => {
                let period = self
                    .years
                    .get(position)
                    .map_or(Period::All, |year| Period::Year(*year));
                self.show_period(period, &sender);
            }
            StatsViewMsg::Month(position) => {
                let Some(Period::Year(year) | Period::Month(year, _)) = self.period else {
                    return;
                };
                let period = position
                    .checked_sub(1)
                    .and_then(|index| self.months.get(index))
                    .map_or(Period::Year(year), |month| Period::Month(year, *month));
                self.show_period(period, &sender);
            }
            StatsViewMsg::Period(period) => self.show_period(period, &sender),
            StatsViewMsg::PlaysChanged => {
                if self.shown {
                    self.offer_periods();
                    self.fill(&sender);
                }
            }
            StatsViewMsg::OpenTopAlbum(index) => {
                if let Some(album) = self.albums.get(index) {
                    self.nav
                        .open(Destination::Album(Box::new((**album).clone())));
                }
            }
            StatsViewMsg::OpenArtist(id, name) => {
                self.nav.open(Destination::Artist { id, name });
            }
        }
    }
}

impl StatsView {
    fn show_period(&mut self, period: Period, sender: &ComponentSender<Self>) {
        if self.period != Some(period) {
            self.period = Some(period);
            self.offer_periods();
            self.fill(sender);
        }
    }

    /// Offer the years and months with plays, keeping the period shown if it still has
    /// some: the latest year at first.
    fn offer_periods(&mut self) {
        let plays = self.actions.plays.clone();
        let history = plays.history();
        self.years = history.years();
        let Some(&latest) = self.years.first() else {
            return;
        };
        let period = match self.period {
            Some(Period::Year(year)) if !self.years.contains(&year) => Period::Year(latest),
            Some(Period::Month(year, _)) if !self.years.contains(&year) => Period::Year(latest),
            Some(Period::Month(year, month)) if !history.months(year).contains(&month) => {
                Period::Year(year)
            }
            Some(period) => period,
            None => Period::Year(latest),
        };
        self.period = Some(period);

        let mut years: Vec<String> = self.years.iter().map(ToString::to_string).collect();
        years.push("All Time".to_owned());
        let year_position = match period {
            Period::Year(year) | Period::Month(year, _) => {
                self.years.iter().position(|y| *y == year).unwrap_or(0)
            }
            Period::All => self.years.len(),
        };
        self.year_picker.set_choices(&years, year_position);

        let year = match period {
            Period::Year(year) | Period::Month(year, _) => Some(year),
            Period::All => None,
        };
        self.months = year.map(|year| history.months(year)).unwrap_or_default();
        let mut months = vec!["Whole Year".to_owned()];
        months.extend(
            self.months
                .iter()
                .map(|month| format::month(*month).to_owned()),
        );
        let month_position = match period {
            Period::Month(_, month) => self
                .months
                .iter()
                .position(|m| *m == month)
                .map_or(0, |index| index + 1),
            _ => 0,
        };
        self.month_picker.set_choices(&months, month_position);
        // A month of all time means nothing.
        self.month_picker
            .widget()
            .set_sensitive(period != Period::All);
    }

    /// Show the stats of the period picked.
    fn fill(&mut self, sender: &ComponentSender<Self>) {
        for tile in self.tiles.drain(..) {
            tile.hide(&self.covers);
        }
        for picker in [&self.year_picker, &self.month_picker] {
            picker.widget().unparent();
        }
        while let Some(child) = self.content.first_child() {
            self.content.remove(&child);
        }

        let (summary, period) = {
            let history = self.actions.plays.history();
            let Some(period) = self.period.filter(|_| !history.is_empty()) else {
                self.content.append(&empty());
                return;
            };
            (history.summary(period), period)
        };

        let header = album_tile::row_header("Listening Stats");
        let pickers = gtk::Box::builder()
            .spacing(6)
            .margin_start(12)
            .valign(gtk::Align::Center)
            .build();
        pickers.append(self.year_picker.widget());
        pickers.append(self.month_picker.widget());
        header.append(&pickers);
        self.content.append(&header);
        self.content.append(&numbers(&summary));
        self.content.append(&timeline(&summary, period, sender));

        self.albums = summary
            .albums
            .iter()
            .take(TOP_ALBUMS)
            .filter_map(|(album, _, _)| album.to_album().map(Rc::new))
            .collect();
        let captions: Vec<String> = summary
            .albums
            .iter()
            .map(|(_, plays, time)| {
                format!(
                    "{} \u{b7} {}",
                    format::play_count(*plays),
                    format::listening_time(*time)
                )
            })
            .collect();
        if !self.albums.is_empty() {
            let open = sender.clone();
            let (row, tiles) = album_tile::album_row(
                "Top Albums",
                &self.albums,
                |album| {
                    let index = self.albums.iter().position(|a| a.id == album.id)?;
                    captions.get(index).cloned()
                },
                (&self.covers, &self.actions),
                album_tile::RowLayout::Grid,
                move |index| open.input(StatsViewMsg::OpenTopAlbum(index)),
                None,
            );
            self.tiles = tiles;
            self.content.append(&row);
        }

        // Each line as high as its own cards, which share its width.
        let cards = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .min_children_per_line(1)
            .max_children_per_line(2)
            .column_spacing(24)
            .row_spacing(24)
            .margin_start(36)
            .margin_end(36)
            .build();
        if !summary.artists.is_empty() {
            cards.append(&top_artists(&summary, sender));
        }
        if !summary.genres.is_empty() {
            cards.append(&top_genres(&summary));
        }
        self.content.append(&cards);
        self.covers.refresh();
    }
}

/// What shows before anything was played.
fn empty() -> gtk::Box {
    let empty = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .vexpand(true)
        .margin_top(96)
        .build();
    empty.append(
        &gtk::Label::builder()
            .label("Nothing Played Yet")
            .css_classes(["pochette-heading"])
            .build(),
    );
    empty.append(
        &gtk::Label::builder()
            .label(
                "Play some albums: how much you listen, your top albums, artists and \
                 genres show up here.",
            )
            .wrap(true)
            .max_width_chars(50)
            .justify(gtk::Justification::Center)
            .css_classes(["dim-label"])
            .build(),
    );
    empty
}

/// The period in a few big numbers.
fn numbers(summary: &Summary) -> gtk::FlowBox {
    let numbers = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .min_children_per_line(1)
        .max_children_per_line(3)
        .column_spacing(12)
        .row_spacing(12)
        .margin_start(36)
        .margin_end(36)
        .build();
    let stats = [
        (format::listening_time(summary.time), "Listened"),
        (summary.album_plays.to_string(), "Album Plays"),
        (summary.artists.len().to_string(), "Artists"),
    ];
    for (value, name) in stats {
        let stat = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .css_classes(["pochette-card"])
            .build();
        stat.append(
            &gtk::Label::builder()
                .label(&value)
                .xalign(0.0)
                .css_classes(["pochette-stat"])
                .build(),
        );
        stat.append(
            &gtk::Label::builder()
                .label(name)
                .xalign(0.0)
                .css_classes(["dim-label"])
                .build(),
        );
        numbers.append(&stat);
    }
    numbers
}

/// The time listened by year for all time, by month for a year, by day for a month, in
/// bars; clicking a year or a month with plays shows it.
fn timeline(summary: &Summary, period: Period, sender: &ComponentSender<StatsView>) -> gtk::Box {
    let title = match period {
        Period::All => "By Year",
        Period::Year(_) => "By Month",
        Period::Month(..) => "By Day",
    };
    let section = gtk::Box::new(gtk::Orientation::Vertical, 12);
    section.append(&album_tile::row_header(title));

    let bars = gtk::Box::builder()
        .homogeneous(true)
        .spacing(if matches!(period, Period::Month(..)) {
            3
        } else {
            6
        })
        .margin_start(36)
        .margin_end(36)
        .build();
    // A few years don't stretch across the page.
    let few = summary.timeline.len() < FEW_BARS;
    if few {
        bars.set_halign(gtk::Align::Start);
    }
    let most = summary.timeline.iter().map(|(_, time)| *time).max();
    let most = most.unwrap_or_default().max(1);
    for &(bucket, time) in &summary.timeline {
        let (name, label, opens) = match period {
            Period::All => (
                bucket.to_string(),
                bucket.to_string(),
                Some(Period::Year(bucket)),
            ),
            Period::Year(year) => (
                format::month(bucket).to_owned(),
                format::short_month(bucket).to_owned(),
                Some(Period::Month(year, bucket)),
            ),
            // Only some days named, or they wouldn't fit.
            Period::Month(_, month) => (
                format!("{} {bucket}", format::month(month)),
                if bucket == 1 || bucket % 5 == 0 {
                    bucket.to_string()
                } else {
                    String::new()
                },
                None::<Period>,
            ),
        };
        let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
        if few {
            column.set_width_request(FEW_BARS_WIDTH);
        }
        column.set_tooltip_text(Some(&format!("{name}: {}", format::listening_time(time))));
        if let Some(opens) = opens.filter(|_| time > 0) {
            column.add_css_class("pochette-bar-column");
            column.set_cursor_from_name(Some("pointer"));
            let click = gtk::GestureClick::new();
            let sender = sender.clone();
            click.connect_released(move |_, _, _, _| sender.input(StatsViewMsg::Period(opens)));
            column.add_controller(click);
        }
        let well = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .height_request(CHART_HEIGHT)
            .valign(gtk::Align::End)
            .build();
        if time > 0 {
            #[expect(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            let height = ((time as f64 / most as f64) * f64::from(CHART_HEIGHT)).round() as i32;
            well.append(
                &gtk::Box::builder()
                    .height_request(height.max(3))
                    .vexpand(true)
                    .valign(gtk::Align::End)
                    .css_classes(["pochette-bar"])
                    .build(),
            );
        }
        column.append(&well);
        column.append(
            &gtk::Label::builder()
                .label(&label)
                .css_classes(["dim-label", "pochette-caption"])
                .build(),
        );
        bars.append(&column);
    }
    section.append(&bars);
    section
}

/// A card with `title` and a list of rows.
fn card(title: &str) -> (gtk::Box, gtk::ListBox) {
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .valign(gtk::Align::Start)
        .hexpand(true)
        .width_request(300)
        .css_classes(["pochette-card"])
        .build();
    card.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(["pochette-subheading"])
            .build(),
    );
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["pochette-ranking"])
        .build();
    card.append(&list);
    (card, list)
}

fn top_artists(summary: &Summary, sender: &ComponentSender<StatsView>) -> gtk::Box {
    let (card, list) = card("Top Artists");
    let artists: Vec<_> = summary.artists.iter().take(TOP).cloned().collect();
    for (rank, (artist, time)) in artists.iter().enumerate() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.append(
            &gtk::Label::builder()
                .label((rank + 1).to_string())
                .width_chars(2)
                .xalign(1.0)
                .css_classes(["dim-label", "pochette-numeric"])
                .build(),
        );
        row.append(
            &gtk::Label::builder()
                .label(&artist.name)
                .xalign(0.0)
                .hexpand(true)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(MAX_CHARS)
                .build(),
        );
        row.append(
            &gtk::Label::builder()
                .label(format::listening_time(*time))
                .css_classes(["dim-label", "pochette-numeric"])
                .build(),
        );
        list.append(
            &gtk::ListBoxRow::builder()
                .child(&row)
                .cursor(&gtk::gdk::Cursor::from_name("pointer", None).expect("a pointer cursor"))
                .build(),
        );
    }
    let sender = sender.clone();
    list.connect_row_activated(move |_, row| {
        let artist = usize::try_from(row.index())
            .ok()
            .and_then(|index| artists.get(index));
        if let Some((artist, _)) = artist {
            sender.input(StatsViewMsg::OpenArtist(artist.id, artist.name.clone()));
        }
    });
    card
}

fn top_genres(summary: &Summary) -> gtk::Box {
    let (card, list) = card("Top Genres");
    let total: u64 = summary.genres.iter().map(|(_, time)| time).sum();
    for (genre, time) in summary.genres.iter().take(TOP) {
        #[expect(clippy::cast_precision_loss)]
        let share = *time as f64 / total.max(1) as f64;
        let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        line.append(
            &gtk::Label::builder()
                .label(genre)
                .xalign(0.0)
                .hexpand(true)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(MAX_CHARS)
                .build(),
        );
        line.append(
            &gtk::Label::builder()
                .label(format!("{:.0}%", share * 100.0))
                .css_classes(["dim-label", "pochette-numeric"])
                .build(),
        );
        row.append(&line);
        let meter = gtk::LevelBar::builder()
            .value(share)
            .css_classes(["pochette-meter"])
            .build();
        // Its offsets would color it by level, not in the accent color.
        for name in ["low", "high", "full"] {
            meter.remove_offset_value(Some(name));
        }
        row.append(&meter);
        list.append(
            &gtk::ListBoxRow::builder()
                .child(&row)
                .activatable(false)
                .build(),
        );
    }
    card
}
