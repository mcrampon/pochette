//! Checks against the real API, as a real user: run them with a session from Albuz's
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
