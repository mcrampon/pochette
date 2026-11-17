# Pochette, built from this repository: `nix build`, or `nix run`.
{
  lib,
  rustPlatform,
  pkg-config,
  wrapGAppsHook4,
  desktop-file-utils,
  appstream,
  aws-lc,
  glib,
  gtk4,
  glib-networking,
  gst_all_1,
  versionCheckHook,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "pochette";
  version = (lib.importTOML ./Cargo.toml).workspace.package.version;

  __structuredAttrs = true;

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./Cargo.toml
      ./Cargo.lock
      ./crates
      ./data
    ];
  };

  cargoLock.lockFile = ./Cargo.lock;

  # The application; the API client is built as its dependency.
  cargoBuildFlags = [
    "--package"
    "pochette"
  ];

  nativeBuildInputs = [
    pkg-config
    wrapGAppsHook4
    desktop-file-utils
    appstream
  ];

  buildInputs = [
    # reqwest's TLS crypto, which nixpkgs links rather than builds.
    aws-lc
    glib
    gtk4
    # TLS for GStreamer's HTTP source.
    glib-networking
    gst_all_1.gstreamer
    # playbin, and the FLAC, HTTP and PulseAudio/PipeWire elements.
    gst_all_1.gst-plugins-base
    gst_all_1.gst-plugins-good
  ];

  postInstall = ''
    install -Dm644 data/io.github.mcrampon.Pochette.desktop -t $out/share/applications
    install -Dm644 data/io.github.mcrampon.Pochette.metainfo.xml -t $out/share/metainfo
    install -Dm644 data/icons/hicolor/scalable/apps/io.github.mcrampon.Pochette.svg \
      -t $out/share/icons/hicolor/scalable/apps
    install -Dm644 data/icons/hicolor/symbolic/apps/io.github.mcrampon.Pochette-symbolic.svg \
      -t $out/share/icons/hicolor/symbolic/apps
  '';

  doInstallCheck = true;
  nativeInstallCheckInputs = [ versionCheckHook ];
  installCheckPhase = ''
    runHook preInstallCheck
    desktop-file-validate $out/share/applications/io.github.mcrampon.Pochette.desktop
    appstreamcli validate --no-net $out/share/metainfo/io.github.mcrampon.Pochette.metainfo.xml
    runHook postInstallCheck
  '';

  meta = {
    description = "Native Linux player for Qobuz, built around albums";
    homepage = "https://github.com/mcrampon/pochette";
    license = lib.licenses.gpl3Plus;
    mainProgram = "pochette";
    platforms = lib.platforms.linux;
  };
})
