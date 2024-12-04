# yaml2pac

Extracted command line tool from [chiptool].

Generate register definitions from YAML files.

[chiptool]: https://github.com/embassy-rs/chiptool

## Usage

```shell

cargo run -- -i uart_v0.yaml -o src/uart.rs --common
```
