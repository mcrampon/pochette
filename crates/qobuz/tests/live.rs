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
