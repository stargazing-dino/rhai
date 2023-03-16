Source Structure
================

Root Sources
------------

| Source file    | Description                                                                     |
| -------------- | ------------------------------------------------------------------------------- |
| `lib.rs`       | Crate root                                                                      |
| `engine.rs`    | The scripting engine, defines the `Engine` type                                 |
| `tokenizer.rs` | Script tokenizer/lexer                                                          |
| `parser.rs`    | Script parser                                                                   |
| `optimizer.rs` | Script optimizer                                                                |
| `reify.rs`     | Utilities for making generic types concrete                                     |
| `tests.rs`     | Unit tests (not integration tests, which are in the `rhai/tests` sub-directory) |


Sub-Directories
---------------

| Sub-directory | Description                                           |
| ------------- | ----------------------------------------------------- |
| `types`       | Common data types (e.g. `Dynamic`, errors)            |
| `api`         | Public API for the scripting engine                   |
| `ast`         | AST definition                                        |
| `module`      | Support for modules                                   |
| `packages`    | Pre-defined packages                                  |
| `func`        | Support for function calls                            |
| `eval`        | Evaluation engine                                     |
| `serde`       | Support for [`serde`](https://crates.io/crates/serde) |
| `tools`       | External tools needed for building                    |
| `bin`         | Pre-built CLI binaries (e.g. `rhai-run`, `rhai-repl`) |
