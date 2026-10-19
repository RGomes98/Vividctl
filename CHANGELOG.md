# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Repo:** Cargo workspace scaffold, CI, lint and dependency policy, contribution docs and issue templates.
- **CLI:** `vividctl list`, `get`, `set` and `reset` for outputs driven by an NVIDIA GPU, naming a monitor by its identity or by connector.
- **GNOME:** Shell extension (46 to 50) that boosts saturation per monitor from inside the compositor, with a D-Bus interface and a containerised test in CI.
- **GNOME backend:** `vividctl list`, `get`, `set` and `reset` also drive the outputs the Shell extension controls, such as the internal panel of a hybrid laptop. The vibrance is converted to a saturation with a provisional curve (SP-08). A monitor an NVIDIA GPU drives is listed once and handled by NVKMS. Building needs `libdbus-1-dev` and `pkg-config`, or `--no-default-features` for the NVKMS backend only.
- **GNOME:** `vividctl extension install`, `status` and `remove`. The extension is embedded in the binary, so it always matches it; installing adds it to the enabled extensions without disturbing the ones already there, and one log in is enough for GNOME Shell on Wayland to load it.
