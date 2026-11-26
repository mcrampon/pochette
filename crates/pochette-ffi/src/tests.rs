//! What Kotlin will call, called from Rust, without the network: one test, as the core's
//! platform is set once per process.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use qobuz::{Client, auth::Session, models::Album};

use crate::{
    Config, LibraryView, Observer, Period, Pochette, Stream, SyncStatus,
    library::Sort,
    lock,
    playback::{LyricLine, current_line},
    session::SignedIn,
};

fn album(id: &str, title: &str, artist: &str, genre: &str, year: &str, added: i64) -> Album {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "title": title,
        "artist": {"id": 7, "name": artist},
        "image": {"small": format!("https://covers/{id}_230.jpg")},
        "genre": {"id": 1, "name": genre},
        "release_date_original": format!("{year}-05-01"),
        "favorited_at": added,
        "streamable": true,
        "tracks_count": 2,
        "duration": 400,
        "tracks": {"offset": 0, "limit": 2, "total": 2, "items": [
            {"id": 11, "title": "One", "track_number": 1, "duration": 200, "streamable": true},
            {"id": 12, "title": "Two", "track_number": 2, "duration": 200, "streamable": true}
        ]}
    }))
    .unwrap()
}

#[derive(Default)]
struct Counting {
    plays: AtomicUsize,
    syncs: AtomicUsize,
}

impl Observer for Counting {
    fn sync_changed(&self, _status: SyncStatus) {
        self.syncs.fetch_add(1, Ordering::SeqCst);
    }

    fn plays_changed(&self) {
        self.plays.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
// One story, as the platform is set once per process.
#[allow(clippy::too_many_lines)]
fn works_offline() {
    let dir = std::env::temp_dir().join(format!("pochette-ffi-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let pochette = Pochette::new(Config {
        data_dir: dir.join("data").to_string_lossy().into_owned(),
        state_dir: dir.join("state").to_string_lossy().into_owned(),
        device_name: "Test Phone".to_owned(),
        app_version: "1.0.0".to_owned(),
    })
    .unwrap();

    // Signed out: the calls that need Qobuz say so.
    assert!(pochette.account().is_none() && pochette.session().is_none());
    let stream = Stream {
        track_id: 11,
        url: "https://stream/11".to_owned(),
        format_id: 27,
        mime_type: None,
        bit_depth: Some(24),
        sampling_rate: Some(96.0),
        // No blob: Qobuz hears nothing of the test.
        blob: None,
        sample: false,
        quality: None,
    };
    assert!(pochette.start_listening("b", &stream).is_err());

    // Signed in, with a session that only exists here.
    let session = Session {
        app_id: "1".to_owned(),
        user_id: 2,
        user_auth_token: "token".to_owned(),
    };
    *lock(&pochette.inner.account) = Some(SignedIn {
        client: Client::new(pochette.inner.http.clone(), session),
        account: crate::Account {
            id: 2,
            name: "Test".to_owned(),
            avatar: None,
            plan: None,
            can_stream: true,
        },
    });
    let kept: serde_json::Value = serde_json::from_str(&pochette.session().unwrap()).unwrap();
    assert_eq!(kept["user_auth_token"], "token");

    // The library, sorted and filtered.
    *lock(&pochette.inner.favorites) = vec![
        album("a", "Zebra", "Alpha", "Rock", "1997", 100),
        album("b", "Apple", "Beta", "Jazz", "2004", 300),
        album("c", "Mango", "Gamma", "Rock", "2021", 200),
    ];
    let ids = |view: LibraryView| -> Vec<String> {
        pochette
            .library(view)
            .into_iter()
            .map(|card| card.id)
            .collect()
    };
    assert_eq!(ids(LibraryView::default()), ["b", "c", "a"]);
    let by_title = LibraryView {
        sort: Sort::Title,
        ..LibraryView::default()
    };
    assert_eq!(ids(by_title.clone()), ["b", "c", "a"]);
    assert_eq!(
        ids(LibraryView {
            reversed: true,
            ..by_title
        }),
        ["a", "c", "b"]
    );
    assert_eq!(
        ids(LibraryView {
            genres: vec!["Rock".to_owned()],
            decades: vec![1990],
            ..LibraryView::default()
        }),
        ["a"]
    );
    assert_eq!(
        ids(LibraryView {
            query: "gamma".to_owned(),
            ..LibraryView::default()
        }),
        ["c"]
    );
    assert_eq!(pochette.library_genres(), ["Jazz", "Rock"]);
    assert_eq!(pochette.library_decades(), [2020, 2000, 1990]);
    assert!(pochette.is_favorite("a") && !pochette.is_favorite("z"));

    // A track heard to its end: into the log, the plays and the stats.
    let observer = Arc::new(Counting::default());
    pochette.observe(Some(observer.clone()));
    lock(&pochette.inner.albums).insert(
        "b".to_owned(),
        album("b", "Apple", "Beta", "Jazz", "2004", 300),
    );
    let listening = pochette.start_listening("b", &stream).unwrap();
    listening.stretch_ended(60_000);
    listening.stretch_started(150_000);
    listening.finished();
    assert_eq!(listening.heard_seconds(), 110);
    assert!(listening.end().unwrap());
    assert!(!listening.end().unwrap(), "a listening ends once");
    assert_eq!(observer.plays.load(Ordering::SeqCst), 1);
    let log = std::fs::read_dir(dir.join("data"))
        .unwrap()
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("plays-Test_Phone-")
        })
        .expect("this device's log");
    assert_eq!(
        std::fs::read_to_string(log.path()).unwrap().lines().count(),
        2
    );
    // 110 seconds of a 400-second album: no play yet.
    assert_eq!(pochette.album_plays("b").count, 0);
    let stats = pochette.stats(Period::All);
    assert_eq!((stats.time, stats.albums.len()), (110, 1));
    assert_eq!(stats.albums[0].album.title, "Apple");
    assert_eq!(stats.artists[0].name, "Beta");

    // Another device's log arrives, as by syncing: the plays are read again.
    let now = pochette_core::time::now();
    std::fs::write(
        dir.join("data/plays-laptop-0123abcd.jsonl"),
        format!(
            "{}\n{}\n",
            serde_json::json!({"album": {"id": "b", "title": "Apple", "artist": {"id": 7, "name": "Beta"}, "length": 400}}),
            serde_json::json!({"listen": {"at": now - 1000, "album": "b", "heard": 300}}),
        ),
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(2500));
    assert_eq!(observer.plays.load(Ordering::SeqCst), 2);
    assert!(observer.syncs.load(Ordering::SeqCst) >= 1);
    assert_eq!(pochette.album_plays("b").count, 1);
    assert_eq!(pochette.stats(Period::All).time, 410);
    assert_eq!(pochette.sync_status().name, "Test Phone");

    // Kotlin calls the engine from its own threads, outside the runtime its tasks start on.
    pochette.set_pairable(true);
    pochette.pair("127.0.0.1:9".to_owned());
    // The engine takes it up in a task of its own.
    let mut pairing = None;
    for _ in 0..40 {
        pairing = pochette.sync_status().pairing;
        if pairing.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let pairing = pairing.expect("a pairing under way");
    assert_eq!(pairing.name, "127.0.0.1:9");
    pochette.answer_pairing(false);
    pochette.dismiss_pairing();
    pochette.forget("nobody");
    pochette.sync_now();
    pochette.set_pairable(false);

    pochette.sign_out();
    assert!(pochette.session().is_none() && pochette.library(LibraryView::default()).is_empty());

    let lines = vec![
        LyricLine {
            at_ms: 1000,
            text: "a".to_owned(),
        },
        LyricLine {
            at_ms: 5000,
            text: "b".to_owned(),
        },
    ];
    assert_eq!(current_line(lines.clone(), 500), None);
    // Lit a little before it's sung.
    assert_eq!(current_line(lines.clone(), 900), Some(0));
    assert_eq!(current_line(lines, 6000), Some(1));

    pochette.observe(None);
    let _ = std::fs::remove_dir_all(&dir);
}
