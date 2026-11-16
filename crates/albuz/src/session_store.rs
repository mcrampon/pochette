//! Keeps the Qobuz session between runs.
//!
//! The session goes to the Secret Service, the freedesktop keyring API that GNOME Keyring,
//! `KWallet`, `KeePassXC` and `oo7-daemon` all provide. Inside Flatpak, `oo7` uses the secret
//! portal instead. When no Secret Service runs, the session goes to a file only the user
//! can read.

use std::{
    collections::HashMap,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
};

use qobuz::auth::Session;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("keyring error: {0}")]
    Keyring(#[from] oo7::Error),
    #[error("cannot access {path}: {source}")]
    File {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("the stored session is unreadable: {0}")]
    Corrupt(#[from] serde_json::Error),
}

type Result<T> = std::result::Result<T, Error>;

const LABEL: &str = "Albuz Qobuz session";

/// Where the session is kept.
pub enum SessionStore {
    Keyring(oo7::Keyring),
    File(PathBuf),
}

impl std::fmt::Debug for SessionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Keyring(_) => f.write_str("SessionStore::Keyring"),
            Self::File(path) => write!(f, "SessionStore::File({})", path.display()),
        }
    }
}

impl SessionStore {
    pub async fn open() -> Self {
        match keyring().await {
            Ok(keyring) => Self::Keyring(keyring),
            Err(err) => {
                let path = session_file();
                tracing::warn!(
                    "no Secret Service available ({err}), keeping the session in {}",
                    path.display()
                );
                Self::File(path)
            }
        }
    }

    pub async fn load(&self) -> Result<Option<Session>> {
        let bytes = match self {
            Self::Keyring(keyring) => {
                let Some(item) = keyring
                    .search_items(&attributes())
                    .await?
                    .into_iter()
                    .next()
                else {
                    return Ok(None);
                };
                item.unlock().await?;
                item.secret().await?.as_bytes().to_vec()
            }
            Self::File(path) => match std::fs::read(path) {
                Ok(bytes) => bytes,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(source) => {
                    return Err(Error::File {
                        path: path.clone(),
                        source,
                    });
                }
            },
        };
        Ok(Some(serde_json::from_slice(&bytes)?))
    }

    pub async fn save(&self, session: &Session) -> Result<()> {
        let json = serde_json::to_vec(session)?;
        match self {
            Self::Keyring(keyring) => {
                keyring
                    .create_item(LABEL, &attributes(), json, true)
                    .await?;
            }
            Self::File(path) => write_private(path, &json).map_err(|source| Error::File {
                path: path.clone(),
                source,
            })?,
        }
        Ok(())
    }

    pub async fn clear(&self) -> Result<()> {
        match self {
            Self::Keyring(keyring) => keyring.delete(&attributes()).await?,
            Self::File(path) => match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(Error::File {
                        path: path.clone(),
                        source,
                    });
                }
            },
        }
        Ok(())
    }
}

/// The keyring, if a Secret Service answers.
async fn keyring() -> Result<oo7::Keyring> {
    let keyring = oo7::Keyring::new().await?;
    // Connecting succeeds even when nothing provides the service; a call doesn't.
    keyring.search_items(&attributes()).await?;
    Ok(keyring)
}

fn attributes() -> HashMap<&'static str, &'static str> {
    HashMap::from([("application", "albuz"), ("kind", "qobuz-session")])
}

fn session_file() -> PathBuf {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .filter(|dir| !dir.is_empty())
        .map_or_else(
            || PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"),
            PathBuf::from,
        );
    data_home.join("albuz").join("session.json")
}

/// Write a file that only the user can read, in a directory only they can enter.
fn write_private(path: &PathBuf, contents: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents)
}
