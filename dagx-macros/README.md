# dagx-macros

Procedural macros for [dagx](https://crates.io/crates/dagx).

This crate provides the `#[task]` attribute macro. You typically don't need to depend on this directly—it's included automatically when you use `dagx`.

## Usage

See the [dagx documentation](https://docs.rs/dagx) for usage examples.

The `#[task]` macro automatically implements the `Task` trait by deriving input and output types from your `run()` method signature.

## License

MIT
