# Contribution

A game engine can only be great when people are using it.
Contributing to the project would be a big help and everyone is welcome to do so.

Please fix my code.

Firstly..
## Discussion

Want to contact someone about this game engine or help in general? You can visit my [Discord server](https://discord.gg/x7ZknsvDdN) and chat in the game engine channels.

There is also a forum work in progress. I will link it here when it is functional.

## Issues

Have a bug report or a feature request?
Filing it in issues would help everyone using the game engine in a way.

Just make sure that
- there is not a similar issue
and
- provide a way to reproduce it.

## Pull requests

If you want to make a pull request then just do that for small things like formatting, fixing things, cleaning and others.

If you are planning something bigger like a new feature, then please announce your plans before you open a pull request, so there are not several people working on the same thing.
If you are trying to solve one of the issues you can announce that you will be working on it.

Before submitting a pull request, make sure there are no problems when runnning the `test.sh` script in the root directory and format it using `cargo fmt`.
Also make sure that all the examples work as they did before or better.

## Code

Right now my game engine is focusing on making it work first. Not everything is tested yet and it is not ready for use.

There are a few things that should change and should be practiced on my game engine right now.

The code is a bit messy, there are many unwraps to be removed, documentation to be added (and do not look at the use parts under `game/mod.rs` (actually do and help me)).

I am planning on cleaning all that up, but I will only clean it when I absolutely need to or all my features on my todo list are completed.

It would be nice if you could handle some of that.

Even if the current code does not follow some of those points, please

- Write clean code
- Have a look at the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Try to avoid unsafe code
- Try to dodge everything that could panic the code like `unwrap`. (and please remove all current unwraps.)
- Have a good look at the code before you make your change
- Add docstrings `///` to every public structs, methods, objects or things like that
- Also add examples in the docs
- Use the `use` statements locally in the functions if it is the only place where it is used. (not like it is right now)
- When making `// TODO:` comments add your GitHub name like `// TODO(Letronix624):`

Correcting some of the current mistakes would be so nice.

Everything is work in progress right now.
