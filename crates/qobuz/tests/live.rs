//! Checks against the real API, as a real user: run them with a session from Pochette's
//! keyring item in `QOBUZ_SESSION`, as JSON, and `cargo test -- --ignored`.

use qobuz::{Client, Quality, auth::Session};

fn client() -> Client {
    let session: Session = serde_json::from_str(
        &std::env::var("QOBUZ_SESSION").expect("QOBUZ_SESSION holds a session"),
    )
    .expect("QOBUZ_SESSION is a session in JSON");
    Client::new(reqwest::Client::new(), session)
}

/// "Hares on The Mountain", by Radie Peat.
const TRACK_ID: u64 = 446_392_172;

#[tokio::test]
#[ignore = "needs a Qobuz session"]
async fn finds_where_to_stream_a_track() {
    let client = client();
    let file = client.file_url(TRACK_ID, Quality::HiRes192).await.unwrap();
    println!(
        "format {} · {:?} · {:?}-bit / {:?} kHz · sample: {} · blob: {}",
        file.format_id,
        file.mime_type,
        file.bit_depth,
        file.sampling_rate,
        file.sample,
        file.blob.is_some(),
    );
    assert!(file.url.starts_with("https://"));

    // The secret found by the first call signs the next ones.
    client.file_url(TRACK_ID, Quality::Cd).await.unwrap();
}

#[tokio::test]
#[ignore = "needs a Qobuz session"]
async fn reads_the_discover_sections() {
    use qobuz::discover::Section;

    let client = client();
    let genres = client.genres().await.unwrap();
    // Jazz and classical.
    let picked: Vec<u64> = genres
        .iter()
        .map(|genre| genre.id)
        .filter(|id| [80, 10].contains(id))
        .collect();
    assert_eq!(picked.len(), 2);
    println!("{} genres", genres.len());

    let discover = client.discover(&picked).await.unwrap();
    for section in [
        Section::NewReleases,
        Section::PressAwards,
        Section::IdealDiscography,
    ] {
        let albums = discover.section(section);
        println!(
            "{section:?}: {} albums, more: {}",
            albums.albums.len(),
            albums.has_more
        );
        assert!(!albums.albums.is_empty());

        let page = client
            .discover_section(section, &picked, 0, 50)
            .await
            .unwrap();
        let next = client
            .discover_section(section, &picked, 50, 50)
            .await
            .unwrap();
        println!(
            "  page of {}, then {}",
            page.albums.len(),
            next.albums.len()
        );
        assert_ne!(page.albums[0].id, next.albums[0].id);
    }
}

#[tokio::test]
#[ignore = "needs a Qobuz session"]
async fn reads_an_artist_page_and_their_releases() {
    use qobuz::artist::ReleaseKind;

    /// Radiohead.
    const ARTIST_ID: u64 = 43840;

    let client = client();
    let artist = client.artist(ARTIST_ID).await.unwrap();
    println!(
        "{}: portrait {}, biography {} chars, {} similar",
        artist.name,
        artist.portrait.is_some(),
        artist.biography.as_deref().map_or(0, str::len),
        artist.similar.len()
    );
    for (kind, albums) in &artist.releases {
        println!(
            "  {kind:?}: {} albums, more: {}",
            albums.albums.len(),
            albums.has_more
        );
    }
    assert_eq!(artist.releases[0].0, ReleaseKind::Album);

    let first = client
        .artist_releases(ARTIST_ID, ReleaseKind::EpSingle, 0, 10)
        .await
        .unwrap();
    let next = client
        .artist_releases(ARTIST_ID, ReleaseKind::EpSingle, 10, 10)
        .await
        .unwrap();
    println!(
        "  EPs and singles: {}, then {}",
        first.albums.len(),
        next.albums.len()
    );
    assert!(first.has_more);
    assert_ne!(first.albums[0].id, next.albums[0].id);
}

#[tokio::test]
#[ignore = "needs a Qobuz session"]
async fn searches_albums_and_artists() {
    let client = client();
    let artists = client.search_artists("gorillaz", 5).await.unwrap();
    println!(
        "artists: {:?}",
        artists.iter().map(|a| &a.name).collect::<Vec<_>>()
    );
    assert_eq!(artists[0].name, "Gorillaz");

    let first = client.search_albums("gorillaz", 0, 5).await.unwrap();
    let next = client.search_albums("gorillaz", 5, 5).await.unwrap();
    println!(
        "albums: {:?}, then {:?}",
        first.albums.iter().map(|a| &a.title).collect::<Vec<_>>(),
        next.albums.iter().map(|a| &a.title).collect::<Vec<_>>()
    );
    assert!(first.has_more);
    assert_ne!(first.albums[0].id, next.albums[0].id);
}
