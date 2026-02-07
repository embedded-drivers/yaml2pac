# yaml2pac

Multi-mode register code generator based on [chiptool] IR (YAML format).

Supports three generation modes from the same YAML register description:

| Mode | Description | Register Access |
|------|-------------|-----------------|
| **pac** | Standard MMIO PAC | `read_volatile`/`write_volatile` pointer ops |
| **rvcsr** | RISC-V CSR registers | `csrrs`/`csrrw`/`csrrc` inline asm |
| **i2cdev** | I2C device registers | Typed `u8` addresses (bring your own transport) |

All modes share the same fieldset/enum type system. Only the register access layer differs.

[chiptool]: https://github.com/embassy-rs/chiptool

## Installation

```shell
cargo install yaml2pac
```

Or from source:

```shell
cargo install --path .
```

## Usage

```shell
# PAC mode (default) - standard MMIO peripheral access
yaml2pac --mode pac -i registers.yaml -o pac.rs --builtin-common

# RISC-V CSR mode - inline asm for CSR access
yaml2pac --mode rvcsr -i csr.yaml -o register.rs --builtin-common

# I2C device mode - typed register addresses
yaml2pac --mode i2cdev -i sensor.yaml -o regs.rs --builtin-common
```

### Options

| Option | Description |
|--------|-------------|
| `-i, --input <FILE>...` | Input YAML file(s). Multiple files are merged. |
| `-o, --output <FILE>` | Output `.rs` file (default: `./out/out.rs`) |
| `--mode <MODE>` | Generation mode: `pac`, `rvcsr`, `i2cdev` (default: `pac`) |
| `--builtin-common` | Embed the common module into the generated output |
| `--common-module-path <PATH>` | Rust path to common module (default: `self::common` with `--builtin-common`, `crate::common` otherwise) |

### Common module path

The `--common-module-path` option controls where generated code looks for the common module (`Reg`, access traits, etc.):

```shell
# Embedded as submodule (default with --builtin-common)
yaml2pac --mode rvcsr -i csr.yaml -o register.rs --builtin-common
# Generated code uses: self::common::Reg

# External module at crate root (default without --builtin-common)
yaml2pac --mode rvcsr -i csr.yaml -o register.rs
# Generated code uses: crate::common::Reg

# Custom path
yaml2pac --mode rvcsr -i csr.yaml -o register.rs --builtin-common --common-module-path "crate::register::common"
# Generated code uses: crate::register::common::Reg
```

## YAML format

Uses the [chiptool IR](https://github.com/embassy-rs/chiptool) YAML format. The `byte_offset` field semantics vary by mode:

- **pac**: Memory offset from peripheral base address
- **rvcsr**: 12-bit CSR address (e.g. `0x300` for `mstatus`)
- **i2cdev**: I2C register address (`u8`)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
