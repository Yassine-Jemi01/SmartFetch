# SmartFetch

A fast, lightweight system information tool with an animated 3D terminal view.

SmartFetch is moving to a Rust-based core and terminal renderer. Rust is not inherently faster than C; both compile to native code. The Rust transition is intended to make the cross-platform terminal engine safer to maintain while keeping the hot rendering path allocation-conscious.

## Highlights

- Fast static system summary.
- sfetch --live: animated 3D distro-logo relief with live CPU, RAM, swap, and uptime.
- sfetch --live --donut: rotating 3D donut with the same live system panel.
- sfetch --donut: shortcut for the live donut mode.
- sfetch --logo NAME: choose a built-in logo.
- Adaptive layout: the artwork and hardware information have a clear gap; narrow terminals switch to a stacked layout instead of squeezing the logo against the text.
- The live renderer uses a single buffered terminal frame and refreshes system statistics once per second.

## Quick start

Requirements: Rust 1.95 or newer and Cargo.

~~~sh
cargo build --release
./target/release/sfetch
~~~

On Unix-like systems, install under /usr/local:

~~~sh
sudo make install
~~~

Or install under a user-owned prefix:

~~~sh
make install PREFIX="$HOME/.local"
~~~

## Commands

~~~sh
sfetch
sfetch --live
sfetch --live --donut
sfetch --donut
sfetch --logo fedora
sfetch --help
sfetch --version
~~~

### Live controls

- Q or Esc: quit and restore the terminal.
- Space: pause or resume rotation.
- R: refresh system information immediately.
- Ctrl+C: exit; the terminal guard restores raw mode and the normal screen.

The live screen uses the terminal's alternate buffer, so it does not overwrite the shell history.

## Platform support

The Rust system-information layer uses sysinfo, which currently supports Linux, macOS, Windows, FreeBSD, and NetBSD. The terminal interface uses crossterm, which supports Unix and Windows terminals.

- Linux: reads the distribution name and ID from /etc/os-release, so detection is not limited to a fixed distro allowlist. Built-in artwork is provided for common distributions; unknown distributions use the generic fallback logo.
- macOS and Windows: OS and host information are detected through the system-information layer.
- BSD: FreeBSD and NetBSD are supported by the system-information dependency. BSD builds are included in cross-target checks where the toolchain permits them.

Hardware sensor availability varies by operating system, permissions, drivers, and virtual-machine environment. Missing sensor values should be reported as unavailable rather than guessed.

## Performance approach

- Keep one System sampler alive instead of rebuilding it every frame.
- Avoid process enumeration in the fetch display path.
- Refresh changing system statistics at a one-second interval.
- Precompute 3D scene points; each frame performs only rotation, projection, depth testing, and one buffered terminal write.
- Release builds use LTO, one codegen unit, and stripping.

## Development

~~~sh
cargo fmt --all -- --check
cargo test --all-targets
cargo build --release
~~~

The legacy C implementation is retained in C_code/ during the Rust migration. The Rust binary is the primary build target.

## License

MIT. See LICENSE.
