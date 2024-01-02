# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- A brand new changelog.
- Added a server mode that does not contain client features like resources, rendering and client specific things.
- A `Sound` struct with `SoundData`, able to play sounds with the help of the kira library.
- Directional sound by binding an object to the new `Sound` struct
- Basic rebound sound to the Pong example.

### Changed

- Layer and scene to not contain a lot of Arc's but have a single outer one. Layers are now accessible as `Arc<Layer>`.
- Objects are now in the typestate pattern under the names of `NewObject` and `Object`.
- With it also the Labels holding the object as a generic now.

### Removed

- Public visibility of the layer new function.

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

[unreleased]: https://github.com/Letronix624/let-engine/compare/0.9.0...main
[0.9.0]: https://github.com/Letronix624/let-engine/releases/tag/0.9.0
