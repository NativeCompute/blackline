# Contributing to Blackline

Thanks for helping. Blackline is a screenwriting app first, so the bar for any
change is simple: **a writer's scripts are never lost, changed or rewritten
behind their back.** Everything below follows from that.

## Before you start

- **Bugs:** open an issue with the *Bug report* template. Say what you did,
  what you expected and what happened, and attach a small script that shows
  it if you can (`⋯` → *Export* → Fountain is the easiest to share).
- **Features:** open an issue with the *Feature request* template first, so we
  can agree on the shape before you spend time on code.
- **Security problems:** do not open a public issue; see
  [SECURITY.md](SECURITY.md).

## Building

```bash
cargo build --release                                # the desktop app
cargo test --release                                 # every test, headless
cargo clippy --release --all-targets -- -D warnings  # must be clean
cargo build --release --lib --target wasm32-unknown-unknown   # the library also builds for WebAssembly
```

On Linux, `./install.sh` builds and installs it for your user.

## Rules for a change

1. **Scripts are sacred.** A change must not alter how an existing script file
   is read or written unless that is the point of the change, and then it
   needs a test that an old file still opens and saves byte for byte where it
   did not change. `an_existing_library_is_never_rewritten_by_the_app` in
   `src/tests.rs` guards this; never loosen it to make a change pass.
2. **Tests come with the change.** Behaviour goes in `src/tests.rs`, anything
   you click in `src/uitests.rs` (a real `App`, driven headless), the shared
   library screens in `src/teamtests.rs`.
3. **No new warnings.** CI runs clippy with `-D warnings` on Linux, macOS and Windows.
4. **Linux, Windows and WebAssembly all build.** Code that only works on one
   goes behind `#[cfg(...)]`, and the others still build.
5. **The design language is NativeCompute's.** Colours, type and controls come
   from [NCF](https://github.com/NativeCompute/ncf) through `src/theme.rs` and
   `src/ui.rs`; do not hard-code a colour or draw a one-off control. A control
   Blackline needs that NCF lacks belongs in NCF. Void black, chrome, one signal blue, hairlines, 2px
   corners, no glows or shadows.
6. **Keep it small.** One change per pull request, described in plain words:
   what it does for a writer, and how you checked it.

## Pull requests

Fill in the template. CI must be green: tests, clippy and the WebAssembly
build on Linux, and the Windows build. A maintainer reviews every change before it is
merged.

By contributing you agree that your contribution is licensed under the
project's [MIT licence](LICENSE).
