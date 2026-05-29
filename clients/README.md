# Editor clients

Thin editor hosts for the local autocomplete broker. Both clients collect editor context, call `POST /v1/autocomplete`, render inline ghost text, and leave provider selection/prompting/postprocessing inside the Rust broker.

## Shared protocol artifact

`clients/protocol/src/generated/` is generated from the Rust protocol crate:

```sh
cargo run -p autocomplete-protocol --bin export_client_artifacts -- clients/protocol/src/generated
```

## Test

```sh
npm test --prefix clients
```
