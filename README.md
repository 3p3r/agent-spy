# agent-spy

WinSpy inspired tool for Agents - The last desktop automation tool your Agents need

![agent-spy](./demo.png)

## Build

Use the package scripts to drive Cargo builds:

- `npm run build` builds the native target for the current machine.
- `npm run build:release` builds a native release binary.
- `npm run build:linux` builds a Linux release binary.
- `npm run build:windows` builds a Windows release binary.
- `npm run build:macos` builds an Apple Silicon macOS release binary.
- `npm run build:macos:intel` builds an Intel macOS release binary.
- `npm run verify` runs `cargo check` and `cargo test`.

Cross-target scripts require the corresponding Rust target toolchain to be installed.
