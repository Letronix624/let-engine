# Let Engine
*A 2d Rust game engine*

- Heavily under construction. Not recommended for use right now.

## Facts and features

- Derive object and camera

- Layer based object system

- Labels and text

- Custom shader support

- Egui support as a feature

# Progress

## To do

- Sounds

- Physics

- 3D layers

- Winit independency

- Post processing

## 3 stages of Rust repository building.

1. [ ] Make it work first
2. [ ] Make it right
3. [ ] Make it fast

## Installation

Command line:

```bash
cargo add let_engine
```

or for your ``Cargo.toml``:

```
let_engine = "0.5.0"
```

### Debian based dependencies

```bash
sudo apt install -y libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev build-essential cmake libvulkan-dev libasound2-dev libfontconfig1-dev
```

### Arch based dependencies

```bash
sudo pacman -Sy vulkan-devel 
```
 
https://crates.io/crates/let-engine
