{
  description = "Pochette: a native Linux player for Qobuz, built around albums";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { nixpkgs, ... }:
    let
      forAllSystems = f: nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" ]
        (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAllSystems (pkgs: {
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
      });
    };
}
