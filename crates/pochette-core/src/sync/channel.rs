//! An encrypted connection between two devices, over TCP, with the Noise protocol: each
//! proves it holds its key, and nobody else on the network can read or change what they
//! say. Messages are JSON, cut into Noise's frames of at most 64 KiB.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::{Serialize, de::DeserializeOwned};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    sync::mpsc,
    task::JoinHandle,
};

use super::device::Identity;

/// Both devices prove their static key, which neither knows of the other beforehand.
pub const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

/// The longest a Noise frame can be, and its authentication tag.
const MAX_FRAME: usize = 65535;
const TAG: usize = 16;

/// The longest message taken, so a device can't have another hold all it sends.
const MAX_MESSAGE: usize = 32 << 20;

/// How long the handshake may take.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("encryption failed: {0}")]
    Noise(#[from] snow::Error),
    #[error("unexpected message: {0}")]
    Json(#[from] serde_json::Error),
    #[error("the other device took too long")]
    Timeout,
    #[error("the other device hung up")]
    Closed,
    #[error("{0}")]
    Protocol(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct Channel<In> {
    writer: OwnedWriteHalf,
    noise: Arc<Mutex<snow::TransportState>>,
    incoming: mpsc::Receiver<Result<In>>,
    reader: JoinHandle<()>,
    /// The other device's public key.
    pub remote: Vec<u8>,
    /// What both devices worked out in the handshake, the same on both only if nobody
    /// stood between them.
    pub handshake_hash: Vec<u8>,
}

impl<In: DeserializeOwned + Send + 'static> Channel<In> {
    /// Shake hands over `stream`, as the device that called if `initiator`.
    pub async fn open(stream: TcpStream, identity: &Identity, initiator: bool) -> Result<Self> {
        let _ = stream.set_nodelay(true);
        tokio::time::timeout(
            HANDSHAKE_TIMEOUT,
            Self::handshake(stream, identity, initiator),
        )
        .await
        .map_err(|_| Error::Timeout)?
    }

    async fn handshake(
        mut stream: TcpStream,
        identity: &Identity,
        initiator: bool,
    ) -> Result<Self> {
        let builder =
            snow::Builder::new(NOISE_PATTERN.parse()?).local_private_key(&identity.private)?;
        let mut handshake = if initiator {
            builder.build_initiator()?
        } else {
            builder.build_responder()?
        };
        let mut buffer = vec![0; MAX_FRAME];
        while !handshake.is_handshake_finished() {
            if handshake.is_my_turn() {
                let length = handshake.write_message(&[], &mut buffer)?;
                write_frame(&mut stream, &buffer[..length]).await?;
            } else {
                let frame = read_frame(&mut stream).await?;
                handshake.read_message(&frame, &mut buffer)?;
            }
        }
        let remote = handshake
            .get_remote_static()
            .ok_or_else(|| Error::Protocol("no key from the other device".to_owned()))?
            .to_vec();
        let handshake_hash = handshake.get_handshake_hash().to_vec();
        let noise = Arc::new(Mutex::new(handshake.into_transport_mode()?));
        let (reader, writer) = stream.into_split();
        let (sender, incoming) = mpsc::channel(4);
        let reader = tokio::spawn(read_messages(reader, Arc::clone(&noise), sender));
        Ok(Self {
            writer,
            noise,
            incoming,
            reader,
            remote,
            handshake_hash,
        })
    }

    pub async fn send(&mut self, message: &impl Serialize) -> Result<()> {
        let json = serde_json::to_vec(message)?;
        let mut plain = u32::try_from(json.len())
            .map_err(|_| Error::Protocol("message too long".to_owned()))?
            .to_be_bytes()
            .to_vec();
        plain.extend(json);
        let mut frames = Vec::new();
        {
            let mut noise = self.noise.lock().expect("the encryption state is sound");
            let mut buffer = vec![0; MAX_FRAME];
            for chunk in plain.chunks(MAX_FRAME - TAG) {
                let length = noise.write_message(chunk, &mut buffer)?;
                frames.push(buffer[..length].to_vec());
            }
        }
        for frame in frames {
            write_frame(&mut self.writer, &frame).await?;
        }
        Ok(())
    }

    /// The next message, waiting for it no longer than `within`. Cancelling it loses
    /// nothing: the message waits for the next call.
    pub async fn recv(&mut self, within: Duration) -> Result<In> {
        match tokio::time::timeout(within, self.incoming.recv()).await {
            Err(_) => Err(Error::Timeout),
            Ok(None) => Err(Error::Closed),
            Ok(Some(message)) => message,
        }
    }
}

impl<In> Drop for Channel<In> {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

async fn read_messages<In: DeserializeOwned>(
    mut reader: OwnedReadHalf,
    noise: Arc<Mutex<snow::TransportState>>,
    sender: mpsc::Sender<Result<In>>,
) {
    let mut plain = Vec::new();
    let mut buffer = vec![0; MAX_FRAME];
    loop {
        // A whole message waiting?
        if plain.len() >= 4 {
            let length = u32::from_be_bytes([plain[0], plain[1], plain[2], plain[3]]) as usize;
            if length > MAX_MESSAGE {
                let _ = sender
                    .send(Err(Error::Protocol("message too long".to_owned())))
                    .await;
                return;
            }
            if plain.len() >= 4 + length {
                let message = serde_json::from_slice(&plain[4..4 + length]).map_err(Error::from);
                plain.drain(..4 + length);
                let failed = message.is_err();
                if sender.send(message).await.is_err() || failed {
                    return;
                }
                continue;
            }
        }
        let frame = match read_frame(&mut reader).await {
            Ok(frame) => frame,
            Err(err) => {
                let closed =
                    matches!(&err, Error::Io(io) if io.kind() == std::io::ErrorKind::UnexpectedEof);
                let _ = sender
                    .send(Err(if closed { Error::Closed } else { err }))
                    .await;
                return;
            }
        };
        let read = noise
            .lock()
            .expect("the encryption state is sound")
            .read_message(&frame, &mut buffer);
        match read {
            Ok(length) => plain.extend_from_slice(&buffer[..length]),
            Err(err) => {
                let _ = sender.send(Err(err.into())).await;
                return;
            }
        }
    }
}

async fn write_frame(writer: &mut (impl AsyncWriteExt + Unpin), frame: &[u8]) -> Result<()> {
    let length =
        u16::try_from(frame.len()).map_err(|_| Error::Protocol("frame too long".to_owned()))?;
    writer.write_all(&length.to_be_bytes()).await?;
    writer.write_all(frame).await?;
    Ok(())
}

async fn read_frame(reader: &mut (impl AsyncReadExt + Unpin)) -> Result<Vec<u8>> {
    let mut length = [0; 2];
    reader.read_exact(&mut length).await?;
    let mut frame = vec![0; usize::from(u16::from_be_bytes(length))];
    reader.read_exact(&mut frame).await?;
    Ok(frame)
}

/// The code both devices show while pairing, from their handshake: the same on both only
/// if they're talking to each other.
#[must_use]
pub fn pairing_code(handshake_hash: &[u8]) -> String {
    let bytes: [u8; 4] = handshake_hash
        .get(..4)
        .and_then(|bytes| bytes.try_into().ok())
        .unwrap_or_default();
    let number = u32::from_be_bytes(bytes) % 1_000_000;
    format!("{:03} {:03}", number / 1000, number % 1000)
}
