# Installation

## Requirements

- Apple Silicon (M1+)
- macOS 26
- Rust 2024 edition

## From crates.io

```
$ cargo install microvm
```

## From source

```
$ git clone https://github.com/windsornguyen/microvm.git
$ cd microvm
$ cargo build --release
```

## Codesigning

Virtualization.framework requires the `com.apple.security.virtualization` entitlement.
You must codesign the binary before it can run:

```
$ codesign --sign - --entitlements entitlements.plist --force target/release/microvm
$ cp target/release/microvm ~/.local/bin/microvm
```

Without this step, macOS will reject the binary at runtime.
