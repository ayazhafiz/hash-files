## Building + running

These instructions come from the perspective of a macOS user. They will likely
be reversed or require additional steps if you are running on a linux system
(or anything else).

### macos

```
cargo b --release

target/release/hash-files
```

### linux x86

First up, make sure to install cross-compiling tools.

```
brew install FiloSottile/musl-cross/musl-cross
```

Building:

```
TARGET_CC=x86_64-linux-musl-gcc cargo build --release --target x86_64-unknown-linux-musl
```

The target binary is then found in

```
target/x86_64-unknown-linux-musl/release/hash-files
```

## Auto-completion when developing for linux-only strategies

Add the following to your `.cargo/config` to indicate to rust-analyzer that it
should analyze with the x86_64-linux target.

```
[build]
target = "x86_64-unknown-linux-musl"
```
