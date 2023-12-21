[![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/Letronix624/let-engine/rust.yml?style=for-the-badge&logo=github&label=GitHub&color=9376e0)](https://github.com/Letronix624/let-engine) [![Crates.io](https://img.shields.io/crates/d/let-engine?style=for-the-badge&logo=rust&label=Crates.io&color=e893cf)](https://crates.io/crates/let-engine) [![Static Badge](https://img.shields.io/badge/Docs-passing?style=for-the-badge&logo=docsdotrs&color=f3bcc8&link=docs.rs%2Flet_engine)](https://docs.rs/let-engine) [![Website](https://img.shields.io/website?up_message=Up&up_color=f6ffa6&down_message=Down&down_color=lightgrey&url=https%3A%2F%2Flet-server.net%2F&style=for-the-badge&logo=apache&color=f6ffa6&link=https%3A%2F%2Flet-server.net%2F)](https://let-server.net/)

# Let Engine
*Right now a simple 2d Rust game engine*

- Heavily under development and not recommended for use right now.

## Facts and features

- Better than Unity

- Layer based object system

- Labels and text

- Custom shader support (limited)

- Egui support as a feature

- Rapier Physics

# Progress
To do:

- Sounds

- 3D layers

- Post processing

- Tick System

- Serialisation, Deserialisation with Serde

- Resource packing system

- Server mode

- Better labels with text edit and caret

## 3 stages of Rust repository building.

1. [ ] Make it work first
2. [ ] Make it right
3. [ ] Make it fast

## Installation

Command line:

```bash
cargo add let_engine
```

### Debian based dependencies

```bash
sudo apt install -y libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev build-essential cmake libvulkan-dev libasound2-dev libfontconfig1-dev
```

### Arch based dependencies

```bash
sudo pacman -Sy vulkan-devel 
```
## Examples

run by doing
```bash
cargo run --example pong / circle / egui
```

## Tips

For best performance compile to `release` with **this** in the `Cargo.toml`
```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
```

## Minimum requirements (Client)

A graphics driver with Vulkan 1.2 support.

# Contribution

feel free to contribute. Go resolve some of the issues I made or take a look at this:
[![dependency status](https://deps.rs/repo/github/Letronix624/let-engine/status.svg)](https://deps.rs/repo/github/Letronix624/let-engine)

I would be grateful.

https://crates.io/crates/let-engine
