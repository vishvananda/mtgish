# MTGish IR compatibility crate

This package re-exports Oracle Grammar's `oracle-ir` crate. The AST definitions,
vocabulary, frozen wire schemas and generated visitors are owned there.

The immutable provider revision is in `Cargo.toml` and the workspace lockfile.
Do not add a local schema or validator here. MTGish Source lowering and Cantrip
generation consume these exact shared Rust types.

See [the migration and component boundaries](../../docs/oracle-owned-model-consumers-20261113.md).
