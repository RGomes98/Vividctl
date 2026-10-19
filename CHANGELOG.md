# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Repo:** Cargo workspace scaffold, CI, lint and dependency policy, contribution docs and issue templates.
- **CLI:** `vividctl list`, `get`, `set` and `reset` for outputs driven by an NVIDIA GPU, naming a monitor by its identity or by connector.
- **GNOME:** Shell extension (46 to 50) that boosts saturation per monitor from inside the compositor, with a D-Bus interface and a containerised test in CI. Not yet wired into `vividctl`.
