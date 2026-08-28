# cargo-boards: Cargo subcommand to run cargo with multiple feature configurations

The use is mostly tailored towards embedded projects
where the same firmware can be used for multiple
boards with only minor differences.

## Installation

The tool can then be installed using:

```
cargo install cargo-boards
```

or

```
git clone https://github.com/MarSik/cargo-boards.git cargo-boards
cd cargo-boards/
cargo install --path=.
```

## Configuration

The tool loads the Boards.toml file and also all boards/<id>.toml files.

Each board in Boards.toml has the following fields:

```
[boards.<id>]
name = "human readable name"
description = "some extra content"
features = ["features", "to", "enable"]
default-features = optional true|false

[boards.<id>.configs]
target = "armv6e-none-eabi"
```

File based boards use the same fields, but `id` matches the basename of the file:

```
# boards/my_board.toml
name = "human readable name"
description = "some extra content"
features = ["features", "to", "enable"]
default-features = optional true|false

[configs]
target = "armv6e-none-eabi"
```

## List

Use `cargo boards --list` to see all configured boards.

## Execution

Just prefix your cargo command with `cargo boards`:

```
# cargo boards build
Building board_1
....
Building board_2
....
```

You will then find the artifacts in the usual target directory, but under `boards/<board id>` subdirectory:

```
target/
  boards/
    board_1/
      debug/
      release/
    board_2/
      debug/
      release/
```

### cargo run and cargo embed

Cargo run works too, but requires a specific board variant to be selected first. All usual arguments work.

```
cargo boards -b board_1 run --release -- my_custom_arguments
```

## Contributing

First install a stable Rust toolchain and add https://docs.rs/cargo-audit/latest/cargo_audit/

All code changes must pass the following checks with no warnings:

- cargo audit
- cargo check
- cargo fmt
- cargo clippy

