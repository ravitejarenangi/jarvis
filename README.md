# JARVIS

A real-life JARVIS: a voice-first AI assistant written in Rust, with a full-screen
holographic HUD that collapses into an arc-reactor orb.

**Status:** Phase 1 in progress (talking JARVIS + HUD on macOS). See the
[design](docs/superpowers/specs/2026-11-03-jarvis-design.md) and the
[build plan](docs/superpowers/plans/2026-11-03-jarvis-phase-0-1.md).

## Build (macOS, Apple Silicon)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
brew install cmake meson ninja pkg-config
scripts/fetch-models.sh
cargo run -p jarvis-app
```

API keys are read from the macOS Keychain (service `jarvis`) or from
`JARVIS_<PROVIDER>_API_KEY` environment variables. Never commit keys.

## Credits

- [God's Eye View](https://github.com/bilawalsidhu/gods-eye-view) (MIT), the
  reference for the planned Rust globe port.
- [VoiceStudio](https://github.com/debpalash/VoiceStudio) (AGPL-3.0), an optional
  local voice engine JARVIS talks to over its API. It is not bundled or linked.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
