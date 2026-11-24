{
  description = "Pochette: a native Linux player for Qobuz, built around albums";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  # Rust with the Android targets' standard library, which nixpkgs' Rust lacks: for the
  # Android dev shell only.
  inputs.rust-overlay = {
    url = "github:oxalica/rust-overlay";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    { nixpkgs, rust-overlay, ... }:
    let
      forAllSystems =
        f:
        nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" ] (
          system: f system nixpkgs.legacyPackages.${system}
        );
    in
    {
      packages = forAllSystems (
        _: pkgs: {
          default = pkgs.callPackage ./package.nix { };
        }
      );

      devShells = forAllSystems (
        system: pkgs: {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              rustc
              clippy
              rustfmt
              rust-analyzer
              gtk4
              # Playback: playbin, FLAC and HTTP sources, PipeWire/PulseAudio output
              gst_all_1.gstreamer
              gst_all_1.gst-plugins-base
              gst_all_1.gst-plugins-good
              glib-networking
              # aws-lc-rs (reqwest's TLS crypto) builds C sources
              cmake
              pkg-config
            ];
            # TLS for GStreamer's HTTP source
            GIO_EXTRA_MODULES = "${pkgs.glib-networking}/lib/gio/modules";
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          };

          # The Android app: `nix develop .#android`. The Android SDK isn't free software:
          # its license is accepted here, for this shell only.
          android =
            let
              androidPkgs = import nixpkgs {
                inherit system;
                config = {
                  android_sdk.accept_license = true;
                  allowUnfree = true;
                };
                overlays = [ rust-overlay.overlays.default ];
              };
              buildTools = "36.1.0";
              ndk = "29.0.14206865";
              sdk =
                (androidPkgs.androidenv.composeAndroidPackages {
                  platformVersions = [ "36" ];
                  buildToolsVersions = [ buildTools ];
                  includeNDK = true;
                  ndkVersions = [ ndk ];
                  # A headless emulator, for tests and screenshots.
                  includeEmulator = true;
                  includeSystemImages = true;
                  systemImageTypes = [ "default" ];
                  abiVersions = [ "x86_64" ];
                }).androidsdk;
              androidHome = "${sdk}/libexec/android-sdk";
              # The same Rust as the desktop build's, pinned for reproducible builds.
              rust = androidPkgs.rust-bin.stable."1.99.1".default.override {
                extensions = [ "rust-src" ];
                targets = [
                  "aarch64-linux-android"
                  "armv7-linux-androideabi"
                  "x86_64-linux-android"
                ];
              };
            in
            androidPkgs.mkShell {
              packages = [
                rust
                sdk
                androidPkgs.cargo-ndk
                androidPkgs.jdk21
                androidPkgs.gradle
                # aws-lc-rs builds C sources, for each Android target too
                androidPkgs.cmake
                androidPkgs.pkg-config
              ];
              ANDROID_HOME = androidHome;
              ANDROID_SDK_ROOT = androidHome;
              ANDROID_NDK_HOME = "${androidHome}/ndk/${ndk}";
              JAVA_HOME = androidPkgs.jdk21.home;
              # Gradle's own aapt2 is built for other Linux systems: NixOS can't run it.
              GRADLE_OPTS = "-Dorg.gradle.project.android.aapt2FromMavenOverride=${androidHome}/build-tools/${buildTools}/aapt2";
            };
        }
      );
    };
}
