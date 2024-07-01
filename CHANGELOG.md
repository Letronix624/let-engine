# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- A new async asset system for managing and loading resources from the disk with ease.
- Scale function for camera scaling.
- `Model` now has a `Data` function allowing you to get the model data.
- Error handling for removed objects.
- Exclusive fullscreen.
- Monitor representations and video modes.
- Cleaning functions that clean the built up memory usage of the engine.
- `is_initialized` function for object.
- New networking system feature to communicate between clients.

### Changed

- `CoefficientCombineRule's` location to `objects::physics`
- Layer's `size_to_world` function does not require dimensions anymore.
- The auto_scale function of appearance now takes a pixel per unit value.
- The `init_with_parent` function does not require a layer anymore.
- Setting an object to invisible makes all children invisible too. For the old effect use `None` as model.
- Switched from dynamic to fixed viewports, hopefully making games faster when not resizing windows.
- Split crate features into multiple crates including `asset-system`, `let-engine-core`, `let-engine-audio`, `let-engine-widgets` and `let-engine`
- Playing spatial sounds now requires a `Listener` to be existant.
- Updated Rapier
- Log crate macros instead of println are getting used for the vulkan validation layer feature.
- Updated winit, therefore updated `keycode` to `key`and added most of the key parts to the `KeyboardEvent` struct.
- `Game` functions are now all async, except for `exit`.
- `Engine` now requires a generic `Game`, being the game struct.

### Fixed

- Deadlock when running the object `sync`
- Default of 0 waiting time in tick settings.
- Move functions of layer swapping instead of moving.
- Crash when syncing a label and removing it afterwards.
- Cursor visible function just being the cursor grab function.

### Removed

- `InputEvent::ReceivedCharacter` in favour of Key::Chararcter
- Labels from the game engine. To access them import the let-engine-widgets library.

## [0.10.0] - 2024-2-10

### Added

- A brand new changelog.
- Added a server mode that does not contain client features like resources, rendering and client specific things.
- A `Sound` struct with `SoundData`, able to play sounds with the help of the kira library.
- Directional sound by binding an object to the new `Sound` struct
- Basic rebound sound to the Pong example.
- Graphics settings with settable display mode and framerate limit

### Changed

- Layer and scene to not contain a lot of Arc's but have a single outer one. Layers are now accessible as `Arc<Layer>`.
- Objects are now in the typestate pattern under the names of `NewObject` and `Object`.
- With it also the Labels holding the object as a generic now.
- Made the game engine more modular by making physics, audio, labels and client their own separate features.
- Settings struct to be more modular
- Switched `rusttype` with `glyph_brush`.
- Cardinal directions are now an enum.
- Rapier version 0.17 -> 0.18

### Removed

- Public visibility of the layer new function.
- A big amount of unwraps inside the engine code giving the object init function a result containing unexpected errors.
- `get_` from most function names.

## [0.9.0] - 2023-12-21

### Added

- Builder struct for the `Object` struct.
- Descriptive error message when running the `egui` example without the `egui` feature activated.
- Engine struct replacing the macro system.
- A game trait that contains methods that the engine calls.
- A default tick system that runs the `tick` method of the `Game` trait.
- Time scale for the `Time` struct, influencing delta time and optionally the tick rate.
- Static variables that make usage simpler including `SCENE`, `TIME`, `INPUT`, `SETTINGS`.

### Changed

- Engine does not get initialized using macros anymore but using the `Engine` struct and it's `start` method.

### Removed

- `let_engine!` and `start_engine!` macros. Instead launch the engine with the new `Engine` struct.
- Visibility of the `Resources` struct, also removing the boilerplate code you had to include to load any resource.

### Fixed

- Window appearing before the scene is ready, making it flash on start.
- Docs.rs works again, removing the need to host the docs on my own website.

[unreleased]: https://github.com/Letronix624/let-engine/compare/0.10.0...main
[0.10.0]: https://github.com/Letronix624/let-engine/compare/0.9.0...0.10.0
[0.9.0]: https://github.com/Letronix624/let-engine/releases/tag/0.9.0
