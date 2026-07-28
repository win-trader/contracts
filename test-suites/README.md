# Fee-vault clean-room black-box tests

These tests derive their expectations only from:

- `docs/design/2026-07-fee-mechanics-theory-ste100.md`
- `docs/design/2026-07-fee-vault-contract-mechanics-ste100.md`
- the public contract specifications embedded in compiled WASM files

The suite does not depend on a production Rust crate. It invokes deployed WASM
contracts through generated public clients.

Build the contract WASM artifacts, then run:

```sh
make build
cargo test -p fee-vault-black-box-tests
```
