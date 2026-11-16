# Albuz

A native Linux player for [Qobuz](https://www.qobuz.com), built around albums.

Requires a paid Qobuz subscription. Albuz uses the Qobuz API but is not certified by Qobuz
and is not affiliated with Qobuz.

## Status

Sign-in only, for now:

```sh
cargo run            # sign in (or check the stored session)
cargo run -- logout  # forget the session
```

Signing in happens in your default browser, on Qobuz's own page, so your saved password or
SSO provider just works. Albuz catches the redirect on a loopback port and keeps the session
in your keyring through the Secret Service API (GNOME Keyring, KWallet, KeePassXC…), or in
`$XDG_DATA_HOME/albuz/session.json` (readable only by you) when no keyring runs.

## Layout

- `crates/qobuz`: Qobuz API client, with no UI code (app id discovery, browser sign-in, API calls)
- `crates/albuz`: the application

## Development

```sh
nix develop          # or any Rust ≥ 1.95 toolchain
cargo test           # add `-- --include-ignored` to also check against the live web player
```
