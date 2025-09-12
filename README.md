[![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/Letronix624/let-engine/rust.yml?style=for-the-badge&logo=github&label=GitHub&color=9376e0)](https://github.com/Letronix624/let-engine) [![Crates.io](https://img.shields.io/crates/d/let-engine?style=for-the-badge&logo=rust&label=Crates.io&color=e893cf)](https://crates.io/crates/let-engine) [![Static Badge](https://img.shields.io/badge/Docs-passing?style=for-the-badge&logo=docsdotrs&color=f3bcc8&link=docs.rs%2Flet_engine)](https://docs.rs/let-engine) [![Website](https://img.shields.io/website?up_message=Up&up_color=f6ffa6&down_message=Down&down_color=lightgrey&url=https%3A%2F%2Flet.software%2F&style=for-the-badge&logo=apache&color=f6ffa6&link=https%3A%2F%2Flet.software%2F)](https://let.software/) [![Static Badge](https://img.shields.io/badge/radicle-up-brightgreen?style=for-the-badge&logo=git&label=Radicle&link=https%3A%2F%2Fapp.radicle.xyz%2Fnodes%2Fseed.let.software%2Frad%3Az35VMD8yfGYcrvb7k2eyxiUL4VUko)](https://app.radicle.xyz/nodes/seed.let.software/rad:z35VMD8yfGYcrvb7k2eyxiUL4VUko)

# let-engine (pre-alpha)

_batteries included game engine_

- Heavily under development.
- Not all core features are ready.
If you for some reason at all are interested in using this game engine, I recommend cloning it and when experiencing problems fix them and submit a pull request. Not everything is tested and the focus right now is to develop all features in first.
Please contact me, let from [let.software](https://let.software/), if you require personal support.

## Features

- Modular backend system

- Layer based object system

- Labels and text

- Built in default networking protocol

- Built in default graphics backend

- Built in Rapier physics

- Tick System

# Installation

To add `let-engine` to your project, run this in your project directory:
```bash
cargo add let_engine
```
## Dependencies
### Debian based dependencies

```bash
sudo apt install -y libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev build-essential cmake libvulkan-dev libasound2-dev libfontconfig1-dev
```
### Arch based dependencies

```bash
sudo pacman -Sy vulkan-devel
```
### Zypper based dependencies

```
sudo zypper install alsa-devel cmake
sudo zypper install --type pattern devel_basis devel_C_C++ devel_vulkan mingw64-cross-gcc-c++ mingw64-cross-pkgconf
```
## Radicle

To clone this repository on [Radicle](https://radicle.xyz), simply run:

```
rad clone rad:z35VMD8yfGYcrvb7k2eyxiUL4VUko
```
# Examples

run by doing
```bash
cargo run --example 
```
## Tip
For best performance compile to `release` with **this** in the `Cargo.toml`

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
```

## Minimum requirements (Client)
A device with Vulkan 1.2 support.
# Contribution
feel free to contribute. Go resolve some of the issues I made or take a look at this:
[![dependency status](https://deps.rs/repo/github/Letronix624/let-engine/status.svg)](https://deps.rs/repo/github/Letronix624/let-engine)
also read [the contribution guidelines](CONTRIBUTING.md).

# Plan
After the first stage of my game engine is completed, this is where I will start advertising the game engine.
Only together this game engine can be made great.

https://crates.io/crates/let-engine
