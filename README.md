# gothite wn

A simple Rust Window Manager.

> gothite: a red or yellow or brown mineral; an oxide of iron that is a common constituent of rust.

*This is a personal project created in an attempt to learn Rust and xlib. Not actually intended for usage and is missing a lot of features at the moment.*

## Installation

Requires Rust, X11 development libraries.

```
cargo build
```

## Usage

Currently only for development purposes, so it is recommended that you use Xephir:

First start the server:

```
Xephyr :3
```

Then start the WM:

```
DISPLAY=:3 RUST_LOG=debug ./target/debug/gothite-wm
```

And any other application you might want, ex:

```
DISPLAY=:3 xclock
```

## Controls

* `ALT` + `Button0` = Move window
* `ALT` + `Button3` = Resize window
* `ALT` + `F4` = Close window

## License

MIT
