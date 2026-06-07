# tira

A Rust terminal UI for working with Jira.

## Development commands

Run the app:

```bash
cargo run
```

Run the app with hot restart on code changes:

```bash
cargo watch -x run
```

Useful hot restart variants:

```bash
cargo watch -q -x run   # less noisy output
cargo watch -c -x run   # clear the terminal before each restart
```

Run tests:

```bash
cargo test
```

Fast compile/type check:

```bash
cargo check
```

Build the binary:

```bash
cargo build
```

Format code:

```bash
cargo fmt
```

Run lints:

```bash
cargo clippy
```

## Installing cargo-watch

If `cargo watch` is not available, install it once:

```bash
cargo install cargo-watch
```
