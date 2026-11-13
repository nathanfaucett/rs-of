# Owner local validation

Run from the repository root:

```sh
cp Cargo.lock .scratch/backend-simplification/owner-validation/Cargo.lock
cargo test --manifest-path .scratch/backend-simplification/owner-validation/Cargo.toml --lib
```

This isolated test crate compiles the actual IdP seed, Storage database catalog, database runtime, and local-copy modules. It uses the real model migrations, SQL engines, IdP service, Management service, and private-key files. It does not replace their behavior with mocks.

The small `RouterState` fixture contains only the real service and client fields used by the seed. It is not the server RouterState. This check does **not** compile or qualify the four HTTP constructors, Device replication, filesystem runtime, or independent serving. Normal constructor tests remain in each owner's `tests/local.rs`.

This separation is needed because the base revision has Device replication API errors and offs `KvSnapshot` serde errors. Those dependencies prevent the normal owner tests from reaching the changed modules. Remove this isolated crate when normal owner tests can run, and move its three tests to the corresponding owner/service suites. Existing catalog and database runtime unit tests also run here through their production modules.

Ticket 04 also includes the actual unified config/TLS modules. Its TLS test sends an HTTPS request through the real TLS listener with certificate verification, tests certificate/key reuse and external loading, and rejects partial retained material. The IdP seed test now also obtains an administrator password-grant token through the real OAuth service. These are not four-owner HTTP tests.
