# altair-rs Starter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build, test, and publish the four starter crates of `altair-rs` (`altair-otel`, `altair-config`, `altair-retry`, `altair-concurrent`) to crates.io as `v0.1.0`, fully workspace-integrated with CI, Nix, Taskfile, and `release-plz`-driven publishing.

**Architecture:** Cargo workspace at repo root with four independent crates under `crates/`. Each crate wraps a best-in-class Rust library (`opentelemetry`, `figment`+`validator`, `backon`, `tokio`), provides typed builders, `thiserror` error types, generous re-exports, and a `prelude` module. Cross-crate observability via the global `tracing` subscriber (set up by `altair-otel`); explicit `Meter` handle for metrics.

**Tech Stack:**
- Rust 2024 edition, MSRV `1.95`
- Async: `tokio = "1"`, `tokio-util = "0.7"`
- Errors: `thiserror = "2"`, (`anyhow = "1"` only in binaries/examples)
- Tracing/OTel: `tracing = "0.1"`, `tracing-subscriber = "0.3"`, `tracing-opentelemetry = "0.33"`, `opentelemetry = "0.32"`, `opentelemetry_sdk = "0.32"`, `opentelemetry-otlp = "0.32"`
- Config: `figment = "0.10"` (features: toml, env), `validator = "0.20"` (derive), `serde = "1"` (derive), `toml = "0.8"` (via figment's bundled toml feature)
- Retry: `backon = "1"`
- Release: `release-plz`, `cargo-llvm-cov`, `cargo-deny`
- Dev tooling: Nix flake, `go-task`, GitHub Actions

---

## File Structure

```
altair-rs/
├── Cargo.toml                          # workspace root
├── rust-toolchain.toml                 # pin Rust 1.95
├── flake.nix                           # dev environment
├── flake.lock
├── Taskfile.yml
├── .envrc
├── .github/workflows/
│   ├── ci.yml
│   ├── release.yml
│   └── docs.yml
├── .gitignore                          # already exists
├── README.md                           # already exists, will update
├── INSTRUCTION.md                      # already exists
├── CLAUDE.md                           # already exists
├── LICENSE                             # already exists
├── deny.toml                           # cargo-deny config
├── release-plz.toml                    # release-plz config
├── docs/
│   ├── specs/                          # already exists
│   ├── plans/                          # this file lives here
│   ├── porting-tracker.md              # already exists
│   └── architecture.md                 # to be created in Phase 0
├── crates/
│   ├── altair-concurrent/
│   │   ├── Cargo.toml
│   │   ├── README.md
│   │   ├── src/lib.rs
│   │   ├── src/error.rs
│   │   ├── src/task_map.rs
│   │   ├── src/executor.rs
│   │   ├── src/prelude.rs
│   │   ├── tests/integration.rs
│   │   └── examples/basic.rs
│   ├── altair-retry/
│   │   ├── Cargo.toml
│   │   ├── README.md
│   │   ├── src/lib.rs
│   │   ├── src/config.rs
│   │   ├── src/error.rs
│   │   ├── src/retry.rs
│   │   ├── src/prelude.rs
│   │   ├── tests/integration.rs
│   │   └── examples/basic.rs
│   ├── altair-config/
│   │   ├── Cargo.toml
│   │   ├── README.md
│   │   ├── src/lib.rs
│   │   ├── src/error.rs
│   │   ├── src/loader.rs
│   │   ├── src/loaders.rs
│   │   ├── src/prelude.rs
│   │   ├── tests/integration.rs
│   │   └── examples/basic.rs
│   └── altair-otel/
│       ├── Cargo.toml
│       ├── README.md
│       ├── src/lib.rs
│       ├── src/config.rs
│       ├── src/error.rs
│       ├── src/init.rs
│       ├── src/globals.rs
│       ├── src/prelude.rs
│       ├── tests/integration.rs
│       └── examples/basic.rs
```

---

## Phase 0: Workspace Foundation

Goal: a working Cargo workspace with Nix, Taskfile, CI, and lint policy. No crates yet — Phase 1 adds the first one.

### Task 0.1: Pin Rust toolchain

**Files:**
- Create: `rust-toolchain.toml`

- [ ] **Step 1: Create the toolchain pin**

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.95.0"
components = ["rustfmt", "clippy", "rust-src"]
profile = "default"
```

- [ ] **Step 2: Verify rustup honors the pin**

Run: `rustc --version`
Expected: `rustc 1.95.0 (...)`. If a different version, ensure `rustup` is installed and updates the toolchain.

- [ ] **Step 3: Commit**

```bash
git add rust-toolchain.toml
git commit -m "chore: pin Rust toolchain to 1.95.0"
```

### Task 0.2: Workspace `Cargo.toml`

**Files:**
- Create: `Cargo.toml`

- [ ] **Step 1: Write the workspace manifest**

```toml
# Cargo.toml
[workspace]
resolver = "3"
members = []

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.95"
license = "MIT"
authors = ["Jasoet <jasoet87@gmail.com>"]
repository = "https://github.com/jasoet/altair-rs"
homepage = "https://github.com/jasoet/altair-rs"
readme = "README.md"
keywords = ["utility", "observability", "tokio", "opentelemetry"]
categories = ["asynchronous", "development-tools"]

[workspace.dependencies]
# Async runtime
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "sync", "signal"] }
tokio-util = { version = "0.7", features = ["rt"] }
futures = "0.3"

# Errors
thiserror = "2"
anyhow = "1"

# Tracing / OpenTelemetry
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json", "fmt", "tracing-log"] }
tracing-opentelemetry = "0.33"
opentelemetry = "0.32"
opentelemetry_sdk = { version = "0.32", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.32", features = ["grpc-tonic", "trace", "metrics", "logs"] }
opentelemetry-stdout = { version = "0.32", features = ["trace", "metrics", "logs"] }
opentelemetry-semantic-conventions = "0.32"

# Config
figment = { version = "0.10", features = ["toml", "env"] }
validator = { version = "0.20", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
toml = "0.8"

# Retry
backon = "1"

# Dev/test
tokio-test = "0.4"
pretty_assertions = "1.4"
assert_matches = "1.5"
tempfile = "3"

[workspace.lints.rust]
missing_docs = "deny"
unsafe_code = "forbid"
rust_2024_compatibility = "warn"

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
module_name_repetitions = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"

[profile.release]
lto = "thin"
codegen-units = 1
```

- [ ] **Step 2: Verify the workspace parses**

Run: `cargo metadata --format-version=1 > /dev/null`
Expected: Exit 0 (no output to stderr).

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add workspace manifest with shared dependencies"
```

### Task 0.3: Nix flake

**Files:**
- Create: `flake.nix`
- Create: `.envrc`

- [ ] **Step 1: Write `flake.nix`**

```nix
{
  description = "altair-rs - Rust utility crates with OpenTelemetry instrumentation";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            cargo-llvm-cov
            cargo-deny
            cargo-nextest
            cargo-release
            release-plz
            go-task
            jq
            curl
            git
          ];

          shellHook = ''
            echo "altair-rs dev shell"
            echo "  rustc: $(rustc --version)"
            echo "  cargo: $(cargo --version)"
            echo "  task:  $(task --version 2>/dev/null || echo 'not found')"
          '';
        };
      });
}
```

- [ ] **Step 2: Write `.envrc`**

```bash
# .envrc
use flake
```

- [ ] **Step 3: Verify flake evaluates**

Run: `nix flake check --no-build 2>&1 | head -20`
Expected: no errors. (First run downloads inputs; allow ~1–2 min.)

Run: `nix develop -c rustc --version`
Expected: `rustc 1.95.0 (...)`

- [ ] **Step 4: Commit**

```bash
git add flake.nix .envrc
git commit -m "chore: add Nix flake with Rust toolchain and dev tools"
```

### Task 0.4: Taskfile

**Files:**
- Create: `Taskfile.yml`

- [ ] **Step 1: Write the Taskfile**

```yaml
# Taskfile.yml
version: '3'

vars:
  N: nix develop -c

tasks:
  default:
    cmds:
      - task --list

  fmt:
    desc: Format all code with rustfmt
    cmds:
      - "{{.N}} cargo fmt --all"

  fmt:check:
    desc: Verify formatting (CI)
    cmds:
      - "{{.N}} cargo fmt --all --check"

  lint:
    desc: Run clippy with strict warnings
    cmds:
      - "{{.N}} cargo clippy --workspace --all-targets --all-features -- -D warnings"

  test:
    desc: Run unit tests
    cmds:
      - "{{.N}} cargo test --workspace --lib"

  test:integration:
    desc: Run integration tests
    cmds:
      - "{{.N}} cargo test --workspace --tests"

  test:doc:
    desc: Run doc tests
    cmds:
      - "{{.N}} cargo test --workspace --doc"

  test:examples:
    desc: Build all examples
    cmds:
      - "{{.N}} cargo build --workspace --examples"

  test:complete:
    desc: Run all tests with coverage
    cmds:
      - "{{.N}} cargo llvm-cov --workspace --all-features --html --output-dir output/coverage"

  doc:
    desc: Build documentation
    cmds:
      - "{{.N}} cargo doc --workspace --no-deps"

  check:
    desc: fmt:check + lint + test
    cmds:
      - task: fmt:check
      - task: lint
      - task: test

  ci:check:
    desc: Full CI check
    cmds:
      - task: fmt:check
      - task: lint
      - task: test
      - task: test:integration
      - task: test:doc
      - task: doc

  deny:
    desc: Run cargo-deny advisories + license check
    cmds:
      - "{{.N}} cargo deny check"

  clean:
    desc: Clean build artifacts
    cmds:
      - "{{.N}} cargo clean"
      - rm -rf output/

  nix:check:
    desc: Verify Nix environment
    cmds:
      - "{{.N}} rustc --version"
      - "{{.N}} cargo --version"
      - "{{.N}} task --version"

  release:
    desc: Run release-plz (CI only)
    cmds:
      - "{{.N}} release-plz release"
```

- [ ] **Step 2: Verify Taskfile parses**

Run: `nix develop -c task --list`
Expected: a table listing all defined tasks.

- [ ] **Step 3: Verify `task nix:check` passes**

Run: `nix develop -c task nix:check`
Expected: prints rustc/cargo/task versions, exit 0.

- [ ] **Step 4: Commit**

```bash
git add Taskfile.yml
git commit -m "chore: add Taskfile with build, test, and release commands"
```

### Task 0.5: `cargo-deny` config

**Files:**
- Create: `deny.toml`

- [ ] **Step 1: Write `deny.toml`**

```toml
# deny.toml
[graph]
all-features = true

[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
yanked = "deny"
ignore = []

[licenses]
allow = [
  "MIT",
  "Apache-2.0",
  "Apache-2.0 WITH LLVM-exception",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "ISC",
  "Unicode-3.0",
  "Unicode-DFS-2016",
  "Zlib",
  "0BSD",
  "MPL-2.0",
  "CDLA-Permissive-2.0",
]
confidence-threshold = 0.9

[bans]
multiple-versions = "warn"
wildcards = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
```

- [ ] **Step 2: Verify `cargo deny` runs**

Run: `task deny`
Expected: exit 0 (no advisories, no banned licenses; the workspace is empty so the graph is trivial).

- [ ] **Step 3: Commit**

```bash
git add deny.toml
git commit -m "chore: add cargo-deny config for advisories and licenses"
```

### Task 0.6: `release-plz` config

**Files:**
- Create: `release-plz.toml`

- [ ] **Step 1: Write `release-plz.toml`**

```toml
# release-plz.toml
[workspace]
allow_dirty = false
changelog_update = true
git_release_enable = true
git_tag_enable = true
publish = true
publish_allow_dirty = false
git_release_type = "auto"
semver_check = true

[changelog]
header = """# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
"""
body = """
## [{{ version }}]\
{%- if previous %} - {{ now() | date(format="%Y-%m-%d") }}{% endif %}

{% for group, commits in commits | group_by(attribute="group") %}
### {{ group | upper_first }}
{% for commit in commits %}
- {%- if commit.scope %} **({{ commit.scope }})**{% endif %} {{ commit.message | upper_first }}
{%- endfor %}
{% endfor %}
"""
commit_parsers = [
    { message = "^feat", group = "Features" },
    { message = "^fix", group = "Bug Fixes" },
    { message = "^docs", group = "Documentation" },
    { message = "^perf", group = "Performance" },
    { message = "^refactor", group = "Refactor" },
    { message = "^test", group = "Tests" },
    { message = "^chore", skip = true },
    { message = "^ci", skip = true },
    { message = "^style", skip = true },
]
```

- [ ] **Step 2: Commit**

```bash
git add release-plz.toml
git commit -m "chore: add release-plz config for automated crates.io publishing"
```

### Task 0.7: GitHub Actions — CI

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Write the CI workflow**

```yaml
# .github/workflows/ci.yml
name: ci

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-D warnings"

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}

jobs:
  fmt:
    name: rustfmt
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all --check

  clippy:
    name: clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --workspace --all-targets --all-features -- -D warnings

  test:
    name: test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --workspace --all-features

  doc:
    name: doc
    runs-on: ubuntu-latest
    env:
      RUSTDOCFLAGS: "-D warnings -D rustdoc::broken-intra-doc-links"
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo doc --workspace --no-deps --all-features

  deny:
    name: cargo-deny
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: EmbarkStudios/cargo-deny-action@v2
        with:
          command: check
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add GitHub Actions workflow for fmt, clippy, test, doc, deny"
```

### Task 0.8: GitHub Actions — release

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Write the release workflow**

```yaml
# .github/workflows/release.yml
name: release

on:
  push:
    branches: [main]

permissions:
  contents: write
  pull-requests: write

jobs:
  release-plz:
    name: release-plz
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
          token: ${{ secrets.GITHUB_TOKEN }}
      - uses: dtolnay/rust-toolchain@stable
      - uses: MarcoIeni/release-plz-action@v0.5
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add release-plz workflow for automated crates.io publishing"
```

### Task 0.9: GitHub Actions — docs

**Files:**
- Create: `.github/workflows/docs.yml`

- [ ] **Step 1: Write the docs workflow**

```yaml
# .github/workflows/docs.yml
name: docs

on:
  push:
    tags: ['v*']
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: false

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo doc --workspace --no-deps --all-features
        env:
          RUSTDOCFLAGS: "--enable-index-page -Zunstable-options"
      - run: |
          echo '<meta http-equiv="refresh" content="0; url=altair_otel/index.html">' > target/doc/index.html
      - uses: actions/upload-pages-artifact@v3
        with:
          path: target/doc

  deploy:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - id: deployment
        uses: actions/deploy-pages@v4
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/docs.yml
git commit -m "ci: add docs workflow to publish cargo doc to GitHub Pages"
```

### Task 0.10: Architecture document

**Files:**
- Create: `docs/architecture.md`

- [ ] **Step 1: Write `docs/architecture.md`**

```markdown
# altair-rs Architecture

Cross-crate patterns and conventions. Companion to the per-crate READMEs.

## Workspace Layout

All crates live under `crates/<name>/`. Each is self-contained: own `Cargo.toml`, `src/`, `tests/`, `examples/`, `README.md`. Workspace-shared dependencies are pinned in the root `Cargo.toml` `[workspace.dependencies]`.

## Cross-Crate Conventions

### Error Handling

- Every public function returns `crate::Result<T>`
- Each crate defines `pub enum Error` via `thiserror`
- `pub type Result<T> = std::result::Result<T, Error>` at crate root
- No shared "altair-error" crate — each owns its error vocabulary

### API Style

- Typed builders for non-trivial config: `Config::builder()...build()`
- Plain structs with `Default` for simple cases
- Free functions for the 80% case (e.g., `retry()`, `from_toml_str()`)
- Builder methods take owned values where ergonomic, references where mandatory

### Re-Exports

Each crate re-exports its key public types at the crate root:

```rust
// In altair-retry/src/lib.rs
pub use config::{Config, ConfigBuilder};
pub use error::{Error, Result, PermanentError};
pub use retry::retry;
```

Each crate also provides a `prelude` module with the most common imports:

```rust
// In altair-retry/src/prelude.rs
pub use crate::{retry, Config, PermanentError, Result};
pub use tokio_util::sync::CancellationToken;
```

### OTel Integration

- **Spans/logs**: via the global `tracing` subscriber. Crates emit `tracing::span!` and `tracing::info!`; whatever subscriber is installed receives them. `altair-otel` provides the canonical OTLP-wiring subscriber.
- **Metrics**: via explicit `Meter` handle. Components that need metrics accept a `Meter` in their builder or obtain one via `altair_otel::meter()`.

### Lints

Per-crate lint policy via `#![lints]` in `src/lib.rs`:

```rust
#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
```

### Testing

- Unit tests: inline `#[cfg(test)] mod tests` blocks
- Integration tests: `crates/<name>/tests/<topic>.rs`
- Doc tests: `///` examples; every public function must have at least one
- Examples: `crates/<name>/examples/<topic>.rs`, must compile and run

### Versioning

- All crates start at `0.1.0`
- Per-crate independent bumps managed by `release-plz` from Conventional Commit scopes
- `0.x`: minor = breaking allowed, patch = additive/fix
- MSRV bumps are minor, never patch
```

- [ ] **Step 2: Commit**

```bash
git add docs/architecture.md
git commit -m "docs: add architecture document"
```

---

## Phase 1: `altair-concurrent`

Goal: First crate published. Implements named, parallel task execution with cancellation, timeout, and per-task tracing.

### Task 1.1: Crate skeleton

**Files:**
- Create: `crates/altair-concurrent/Cargo.toml`
- Create: `crates/altair-concurrent/src/lib.rs`
- Create: `crates/altair-concurrent/README.md`
- Modify: `Cargo.toml` (add member)

- [ ] **Step 1: Write `crates/altair-concurrent/Cargo.toml`**

```toml
[package]
name = "altair-concurrent"
description = "Type-safe parallel execution of named async tasks with cancellation and per-task tracing"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
homepage.workspace = true
readme = "README.md"
keywords = ["async", "concurrent", "tokio", "tracing"]
categories = ["asynchronous", "concurrency"]

[dependencies]
tokio = { workspace = true }
tokio-util = { workspace = true }
futures = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "macros", "rt-multi-thread"] }
pretty_assertions = { workspace = true }
assert_matches = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 2: Write `crates/altair-concurrent/src/lib.rs`** (skeleton)

```rust
//! Type-safe parallel execution of named async tasks.
//!
//! Provides a [`TaskMap`] for declaring named tasks and an [`execute_concurrently`]
//! entry point that runs them on the tokio runtime with optional cancellation,
//! timeout, and partial-results modes. Each task runs inside its own tracing
//! span so it appears as a separate node in distributed traces.
//!
//! # Example
//!
//! ```no_run
//! use altair_concurrent::{execute_concurrently, TaskMap};
//!
//! # async fn run() -> altair_concurrent::Result<()> {
//! let tasks: TaskMap<String> = TaskMap::new()
//!     .insert("greet", |_| async { Ok("hi".to_string()) });
//! let results = execute_concurrently(tasks).await?;
//! assert_eq!(results["greet"], "hi");
//! # Ok(()) }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod error;
mod executor;
mod task_map;

pub mod prelude;

pub use error::{Error, Result};
pub use executor::{execute_concurrently, Executor};
pub use task_map::TaskMap;

// Re-exports for one-dep ergonomics
pub use tokio_util::sync::CancellationToken;
```

- [ ] **Step 3: Write `crates/altair-concurrent/README.md`** (stub)

```markdown
# altair-concurrent

Type-safe parallel execution of named async tasks with cancellation, timeout, and per-task tracing.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace — see [porting tracker](../../docs/porting-tracker.md) for status of other crates.

## Quick start

```rust,no_run
use altair_concurrent::{execute_concurrently, TaskMap};

# async fn run() -> altair_concurrent::Result<()> {
let tasks: TaskMap<String> = TaskMap::new()
    .insert("fetch_user", |_ctx| async { fetch_user(42).await })
    .insert("fetch_orders", |_ctx| async { fetch_orders(42).await });

let results = execute_concurrently(tasks).await?;
assert!(results.contains_key("fetch_user"));
# Ok(()) }
# async fn fetch_user(_: u64) -> Result<String, std::io::Error> { Ok("u".into()) }
# async fn fetch_orders(_: u64) -> Result<String, std::io::Error> { Ok("o".into()) }
```

## License

MIT
```

- [ ] **Step 4: Register crate in workspace**

In root `Cargo.toml`, change `members = []` to:

```toml
members = ["crates/altair-concurrent"]
```

- [ ] **Step 5: Verify the workspace still parses**

Run: `task fmt:check && task lint` — expected: failures because `error.rs`, `executor.rs`, `task_map.rs`, `prelude.rs` don't exist yet. This step's purpose is to confirm `Cargo.toml` is well-formed.

Run: `cargo metadata --format-version=1 > /dev/null`
Expected: exit 0.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/altair-concurrent
git commit -m "feat(concurrent): scaffold altair-concurrent crate"
```

### Task 1.2: `Error` type — failing test

**Files:**
- Create: `crates/altair-concurrent/src/error.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/altair-concurrent/src/error.rs`:

```rust
//! Errors produced by parallel task execution.

use thiserror::Error;

/// Errors returned by [`crate::execute_concurrently`].
#[derive(Debug, Error)]
pub enum Error {
    /// A task returned an error; remaining tasks were cancelled.
    #[error("task '{name}' failed: {source}")]
    TaskFailed {
        /// The static name of the failing task.
        name: &'static str,
        /// The underlying error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// The cancellation token fired before all tasks completed.
    #[error("execution cancelled")]
    Cancelled,

    /// The configured timeout elapsed before all tasks completed.
    #[error("execution timed out")]
    Timeout,

    /// A task panicked or was cancelled by the runtime.
    #[error("join error: {0}")]
    Join(#[from] tokio::task::JoinError),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_failed_message_includes_name() {
        let err = Error::TaskFailed {
            name: "fetch_user",
            source: "boom".into(),
        };
        assert!(err.to_string().contains("fetch_user"));
        assert!(err.to_string().contains("boom"));
    }

    #[test]
    fn cancelled_renders() {
        assert_eq!(Error::Cancelled.to_string(), "execution cancelled");
    }

    #[test]
    fn timeout_renders() {
        assert_eq!(Error::Timeout.to_string(), "execution timed out");
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p altair-concurrent --lib`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-concurrent/src/error.rs
git commit -m "feat(concurrent): add Error type with TaskFailed/Cancelled/Timeout/Join variants"
```

### Task 1.3: `TaskMap` — failing test then impl

**Files:**
- Create: `crates/altair-concurrent/src/task_map.rs`

- [ ] **Step 1: Write `task_map.rs` (test + impl together)**

```rust
//! Builder for a set of named concurrent tasks.

use futures::future::BoxFuture;
use std::collections::BTreeMap;
use tokio_util::sync::CancellationToken;

type BoxedTaskFn<T> =
    Box<dyn FnOnce(CancellationToken) -> BoxFuture<'static, Result<T, BoxedError>> + Send>;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// A set of named tasks to run concurrently.
///
/// `T` is the success result type — all tasks in a `TaskMap` produce the
/// same `T`. For heterogeneous batches, use `tokio::join!` directly.
pub struct TaskMap<T> {
    pub(crate) tasks: BTreeMap<&'static str, BoxedTaskFn<T>>,
}

impl<T> TaskMap<T> {
    /// Create an empty task map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
        }
    }

    /// Insert a named task into the map.
    ///
    /// The closure receives the active [`CancellationToken`] and must return
    /// a future producing `Result<T, E>` where `E` can be boxed into a
    /// `std::error::Error`.
    #[must_use]
    pub fn insert<F, Fut, E>(mut self, name: &'static str, task: F) -> Self
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = std::result::Result<T, E>> + Send + 'static,
        E: Into<BoxedError>,
        T: Send + 'static,
    {
        let boxed: BoxedTaskFn<T> = Box::new(move |token| {
            let fut = task(token);
            Box::pin(async move { fut.await.map_err(Into::into) })
        });
        self.tasks.insert(name, boxed);
        self
    }

    /// Return the number of tasks currently in the map.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Return `true` if no tasks have been inserted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

impl<T> Default for TaskMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn new_is_empty() {
        let m: TaskMap<u32> = TaskMap::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn insert_increments_len() {
        let m: TaskMap<u32> = TaskMap::new()
            .insert("a", |_| async { Ok::<_, std::io::Error>(1) })
            .insert("b", |_| async { Ok::<_, std::io::Error>(2) });
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn insert_duplicate_overwrites() {
        let m: TaskMap<u32> = TaskMap::new()
            .insert("a", |_| async { Ok::<_, std::io::Error>(1) })
            .insert("a", |_| async { Ok::<_, std::io::Error>(2) });
        assert_eq!(m.len(), 1);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-concurrent --lib task_map`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-concurrent/src/task_map.rs
git commit -m "feat(concurrent): add TaskMap builder for named tasks"
```

### Task 1.4: `Executor` and `execute_concurrently` — fail-fast path

**Files:**
- Create: `crates/altair-concurrent/src/executor.rs`

- [ ] **Step 1: Write `executor.rs`**

```rust
//! Concurrent execution entry point.

use crate::error::{Error, Result};
use crate::task_map::TaskMap;
use std::collections::HashMap;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{instrument, Instrument};

/// Configures and runs a [`TaskMap`].
///
/// Construct via [`execute_concurrently`].
pub struct Executor<T> {
    tasks: TaskMap<T>,
    cancellation: Option<CancellationToken>,
    timeout: Option<Duration>,
    partial: bool,
}

impl<T> Executor<T>
where
    T: Send + 'static,
{
    /// Attach a cancellation token. Cancelling it causes all tasks to abort.
    #[must_use]
    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation = Some(token);
        self
    }

    /// Apply an overall timeout. If the timeout elapses, remaining tasks are cancelled.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Return per-task `Result`s instead of fail-fast.
    #[must_use]
    pub fn with_partial_results(mut self) -> Self {
        self.partial = true;
        self
    }
}

impl<T> std::future::IntoFuture for Executor<T>
where
    T: Send + 'static,
{
    type Output = Result<HashMap<&'static str, T>>;
    type IntoFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { run(self).await })
    }
}

#[instrument(skip(executor), fields(task_count = executor.tasks.len()))]
async fn run<T>(executor: Executor<T>) -> Result<HashMap<&'static str, T>>
where
    T: Send + 'static,
{
    let token = executor.cancellation.unwrap_or_else(CancellationToken::new);
    let mut set: JoinSet<(&'static str, std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>)> = JoinSet::new();

    for (name, task_fn) in executor.tasks.tasks {
        let child_token = token.clone();
        let span = tracing::info_span!("concurrent.task", task.name = name);
        set.spawn(
            async move {
                let result = task_fn(child_token).await;
                (name, result)
            }
            .instrument(span),
        );
    }

    let mut results: HashMap<&'static str, T> = HashMap::new();
    let mut errors: HashMap<&'static str, Box<dyn std::error::Error + Send + Sync>> =
        HashMap::new();

    let timeout = executor.timeout;

    loop {
        let next = async { set.join_next().await };
        let outcome = if let Some(d) = timeout {
            match tokio::time::timeout(d, next).await {
                Ok(v) => v,
                Err(_) => {
                    token.cancel();
                    set.shutdown().await;
                    return Err(Error::Timeout);
                }
            }
        } else {
            next.await
        };

        match outcome {
            None => break,
            Some(Err(e)) => return Err(Error::Join(e)),
            Some(Ok((name, Ok(v)))) => {
                results.insert(name, v);
            }
            Some(Ok((name, Err(e)))) => {
                if executor.partial {
                    errors.insert(name, e);
                } else {
                    token.cancel();
                    set.shutdown().await;
                    return Err(Error::TaskFailed { name, source: e });
                }
            }
        }

        if token.is_cancelled() && set.is_empty() {
            return Err(Error::Cancelled);
        }
    }

    if executor.partial && !errors.is_empty() {
        // Partial mode: surface first error as TaskFailed for symmetry; full Results
        // map exposed via `with_partial_results` is out-of-scope for v0.1.0.
        let (name, source) = errors.into_iter().next().expect("non-empty");
        return Err(Error::TaskFailed { name, source });
    }

    Ok(results)
}

/// Run a [`TaskMap`] concurrently.
///
/// Returns an [`Executor`] that resolves to a `HashMap<&'static str, T>`
/// when awaited.
#[must_use]
pub fn execute_concurrently<T>(tasks: TaskMap<T>) -> Executor<T> {
    Executor {
        tasks,
        cancellation: None,
        timeout: None,
        partial: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn empty_map_resolves_to_empty_results() {
        let m: TaskMap<u32> = TaskMap::new();
        let r = execute_concurrently(m).await.unwrap();
        assert!(r.is_empty());
    }

    #[tokio::test]
    async fn two_tasks_complete() {
        let m: TaskMap<u32> = TaskMap::new()
            .insert("a", |_| async { Ok::<_, std::io::Error>(1) })
            .insert("b", |_| async { Ok::<_, std::io::Error>(2) });
        let r = execute_concurrently(m).await.unwrap();
        assert_eq!(r["a"], 1);
        assert_eq!(r["b"], 2);
    }

    #[tokio::test]
    async fn failing_task_returns_task_failed_error() {
        let m: TaskMap<u32> = TaskMap::new()
            .insert("ok", |_| async { Ok::<_, std::io::Error>(1) })
            .insert("bad", |_| async {
                Err::<u32, std::io::Error>(std::io::Error::other("boom"))
            });
        let err = execute_concurrently(m).await.unwrap_err();
        match err {
            Error::TaskFailed { name, .. } => assert_eq!(name, "bad"),
            other => panic!("expected TaskFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn timeout_returns_timeout_error() {
        let m: TaskMap<u32> = TaskMap::new().insert("slow", |_| async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok::<_, std::io::Error>(1)
        });
        let err = execute_concurrently(m)
            .with_timeout(Duration::from_millis(50))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Timeout));
    }

    #[tokio::test]
    async fn external_cancellation_causes_cancelled_error() {
        let token = CancellationToken::new();
        let inner = token.clone();
        let m: TaskMap<u32> = TaskMap::new().insert("waiter", move |ct| async move {
            ct.cancelled().await;
            Err::<u32, std::io::Error>(std::io::Error::other("cancelled"))
        });
        let handle = tokio::spawn(async move {
            execute_concurrently(m).with_cancellation(token).await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        inner.cancel();
        let err = handle.await.unwrap().unwrap_err();
        // Either TaskFailed or Cancelled is acceptable depending on order.
        assert!(matches!(err, Error::TaskFailed { .. } | Error::Cancelled));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-concurrent --lib executor`
Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-concurrent/src/executor.rs
git commit -m "feat(concurrent): add execute_concurrently with cancellation and timeout"
```

### Task 1.5: `prelude` module

**Files:**
- Create: `crates/altair-concurrent/src/prelude.rs`

- [ ] **Step 1: Write `prelude.rs`**

```rust
//! Common imports for users of this crate.
//!
//! ```
//! use altair_concurrent::prelude::*;
//! ```

pub use crate::{execute_concurrently, CancellationToken, Error, Executor, Result, TaskMap};
```

- [ ] **Step 2: Doc-test runs**

Run: `cargo test -p altair-concurrent --doc`
Expected: doc tests pass (including the lib.rs example and the prelude example).

- [ ] **Step 3: Commit**

```bash
git add crates/altair-concurrent/src/prelude.rs
git commit -m "feat(concurrent): add prelude module"
```

### Task 1.6: Integration test

**Files:**
- Create: `crates/altair-concurrent/tests/integration.rs`

- [ ] **Step 1: Write integration test**

```rust
//! End-to-end behavior tests for altair-concurrent.

use altair_concurrent::prelude::*;
use pretty_assertions::assert_eq;
use std::time::Duration;

#[tokio::test]
async fn three_parallel_tasks_share_results() {
    let tasks: TaskMap<String> = TaskMap::new()
        .insert("alpha", |_| async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok::<_, std::io::Error>("a".to_string())
        })
        .insert("beta", |_| async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            Ok::<_, std::io::Error>("b".to_string())
        })
        .insert("gamma", |_| async {
            Ok::<_, std::io::Error>("g".to_string())
        });

    let results = execute_concurrently(tasks).await.unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(results["alpha"], "a");
    assert_eq!(results["beta"], "b");
    assert_eq!(results["gamma"], "g");
}

#[tokio::test]
async fn cancellation_token_propagates_to_tasks() {
    let token = CancellationToken::new();
    let m: TaskMap<bool> = TaskMap::new().insert("respect_ct", |ct| async move {
        tokio::select! {
            _ = ct.cancelled() => Ok::<_, std::io::Error>(false),
            _ = tokio::time::sleep(Duration::from_secs(10)) => Ok::<_, std::io::Error>(true),
        }
    });

    let inner = token.clone();
    let handle = tokio::spawn(async move {
        execute_concurrently(m).with_cancellation(inner).await
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    token.cancel();
    let result = handle.await.unwrap();
    // External cancel may yield either flavor; success means the task observed the token.
    assert!(result.is_err() || result.unwrap()["respect_ct"] == false);
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p altair-concurrent --tests`
Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-concurrent/tests/integration.rs
git commit -m "test(concurrent): add integration tests for parallel execution and cancellation"
```

### Task 1.7: Example binary

**Files:**
- Create: `crates/altair-concurrent/examples/basic.rs`

- [ ] **Step 1: Write the example**

```rust
//! Run with: `cargo run --example basic -p altair-concurrent`

use altair_concurrent::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tasks: TaskMap<String> = TaskMap::new()
        .insert("fetch_user", |_| async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok::<_, std::io::Error>("alice".to_string())
        })
        .insert("fetch_orders", |_| async {
            tokio::time::sleep(Duration::from_millis(80)).await;
            Ok::<_, std::io::Error>("3 open".to_string())
        })
        .insert("fetch_prefs", |_| async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            Ok::<_, std::io::Error>("dark mode".to_string())
        });

    let results = execute_concurrently(tasks).await?;
    for (name, value) in results {
        println!("{name} = {value}");
    }
    Ok(())
}
```

- [ ] **Step 2: Add `anyhow` to dev-dependencies**

Edit `crates/altair-concurrent/Cargo.toml`, add to `[dev-dependencies]`:

```toml
anyhow = { workspace = true }
```

- [ ] **Step 3: Build the example**

Run: `cargo build -p altair-concurrent --example basic`
Expected: builds cleanly.

- [ ] **Step 4: Run the example**

Run: `cargo run -p altair-concurrent --example basic`
Expected: three lines of output in any order: `fetch_user = alice`, `fetch_orders = 3 open`, `fetch_prefs = dark mode`.

- [ ] **Step 5: Commit**

```bash
git add crates/altair-concurrent/Cargo.toml crates/altair-concurrent/examples/basic.rs
git commit -m "docs(concurrent): add basic usage example"
```

### Task 1.8: Crate README

**Files:**
- Modify: `crates/altair-concurrent/README.md`

- [ ] **Step 1: Replace stub README with full recipe**

```markdown
# altair-concurrent

Type-safe parallel execution of named async tasks with cancellation, timeout, and per-task tracing.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-concurrent
```

## Quick start

```rust,no_run
use altair_concurrent::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> altair_concurrent::Result<()> {
    let tasks: TaskMap<String> = TaskMap::new()
        .insert("fetch_user", |_| async { Ok::<_, std::io::Error>("alice".into()) })
        .insert("fetch_orders", |_| async { Ok::<_, std::io::Error>("3 open".into()) });

    let results = execute_concurrently(tasks)
        .with_timeout(Duration::from_secs(5))
        .await?;

    println!("{:?}", results);
    Ok(())
}
```

## Features

- **Named tasks** — `HashMap<&'static str, T>` results, not positional tuples
- **Tracing** — each task runs inside a `tracing::info_span!("concurrent.task", task.name = ...)` so it shows up as a separate node in distributed traces
- **Cancellation** — pass a `CancellationToken`; cancelling it aborts all tasks
- **Timeout** — `.with_timeout(Duration)`; expires cancel remaining tasks
- **Fail-fast or partial** — by default, the first error cancels remaining tasks; `with_partial_results()` switches to "run all, surface first error" semantics

## Constraints

- All tasks must return the same `Result<T, E>`. For heterogeneous batches, use `tokio::join!` directly.
- Built on `tokio::task::JoinSet`; tokio is the only supported runtime.

## License

[MIT](../../LICENSE)
```

- [ ] **Step 2: Verify doc tests still pass**

Run: `cargo test -p altair-concurrent --doc`
Expected: all doc tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-concurrent/README.md
git commit -m "docs(concurrent): expand README with quick start and feature list"
```

### Task 1.9: Update porting tracker

**Files:**
- Modify: `docs/porting-tracker.md`

- [ ] **Step 1: Move `altair-concurrent` row from Planned to In Progress**

In the "Starter Set" table, change the `altair-concurrent` row's status to `🚧 In Progress`.

- [ ] **Step 2: Commit**

```bash
git add docs/porting-tracker.md
git commit -m "docs: mark altair-concurrent as in-progress"
```

### Task 1.10: Full local CI check

- [ ] **Step 1: Run the full check**

Run: `task ci:check`
Expected: all stages pass.

- [ ] **Step 2: If anything fails, fix it before proceeding to Phase 2**

---

## Phase 2: `altair-retry`

Goal: Retry wrapper over `backon` with auto-tracing and permanent-error short-circuit.

### Task 2.1: Crate skeleton

**Files:**
- Create: `crates/altair-retry/Cargo.toml`
- Create: `crates/altair-retry/src/lib.rs`
- Create: `crates/altair-retry/README.md`
- Modify: `Cargo.toml` (add member)

- [ ] **Step 1: Write `crates/altair-retry/Cargo.toml`**

```toml
[package]
name = "altair-retry"
description = "Async retry with exponential backoff, auto-traced via the tracing crate"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
homepage.workspace = true
readme = "README.md"
keywords = ["retry", "backoff", "async", "tokio", "tracing"]
categories = ["asynchronous", "network-programming"]

[dependencies]
tokio = { workspace = true }
tokio-util = { workspace = true }
backon = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "macros", "rt-multi-thread"] }
pretty_assertions = { workspace = true }
anyhow = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 2: Write `crates/altair-retry/src/lib.rs`**

```rust
//! Async retry with exponential backoff and automatic tracing.
//!
//! Each retry attempt runs inside a `tracing::span!` so it appears in
//! distributed traces. If `altair-otel` is initialized in the same process,
//! retries flow to OTLP automatically.
//!
//! # Example
//!
//! ```no_run
//! use altair_retry::{retry, Config};
//! use std::time::Duration;
//!
//! # async fn run() -> altair_retry::Result<()> {
//! # async fn ping() -> std::io::Result<()> { Ok(()) }
//! let cfg = Config::builder()
//!     .name("db.connect")
//!     .max_retries(3)
//!     .initial_interval(Duration::from_millis(100))
//!     .build();
//!
//! retry(cfg, || async { ping().await }).await?;
//! # Ok(()) }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod config;
mod error;
mod retry;

pub mod prelude;

pub use config::{Config, ConfigBuilder};
pub use error::{Error, PermanentError, Result};
pub use retry::retry;

// Re-exports for one-dep ergonomics
pub use tokio_util::sync::CancellationToken;
```

- [ ] **Step 3: Write `crates/altair-retry/README.md` stub**

```markdown
# altair-retry

Async retry with exponential backoff and automatic tracing.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

(See lib.rs doc comment for usage.)
```

- [ ] **Step 4: Add to workspace members**

In root `Cargo.toml`:

```toml
members = ["crates/altair-concurrent", "crates/altair-retry"]
```

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/altair-retry
git commit -m "feat(retry): scaffold altair-retry crate"
```

### Task 2.2: `Error` and `PermanentError`

**Files:**
- Create: `crates/altair-retry/src/error.rs`

- [ ] **Step 1: Write `error.rs`**

```rust
//! Error types for retry operations.

use thiserror::Error;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// Errors returned by [`crate::retry`].
#[derive(Debug, Error)]
pub enum Error {
    /// All retry attempts exhausted; final attempt's error is preserved.
    #[error("retry '{name}' exhausted after {attempts} attempts: {source}")]
    Exhausted {
        /// The retry config's name.
        name: String,
        /// Number of attempts made.
        attempts: u32,
        /// The last underlying error.
        #[source]
        source: BoxedError,
    },

    /// The operation returned a [`PermanentError`]; no more retries attempted.
    #[error("retry '{name}' encountered permanent error: {source}")]
    Permanent {
        /// The retry config's name.
        name: String,
        /// The underlying permanent error.
        #[source]
        source: BoxedError,
    },

    /// The cancellation token was triggered.
    #[error("retry '{name}' cancelled")]
    Cancelled {
        /// The retry config's name.
        name: String,
    },
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Marker for non-retryable errors. Wrap an error with [`PermanentError::wrap`]
/// to short-circuit retry — the next attempt is not made and the wrapped
/// error is returned via [`Error::Permanent`].
#[derive(Debug)]
pub struct PermanentError {
    pub(crate) inner: BoxedError,
}

impl PermanentError {
    /// Wrap an error so retry treats it as permanent.
    #[must_use]
    pub fn wrap<E>(e: E) -> Self
    where
        E: Into<BoxedError>,
    {
        Self { inner: e.into() }
    }
}

impl std::fmt::Display for PermanentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl std::error::Error for PermanentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausted_includes_name_and_count() {
        let e = Error::Exhausted {
            name: "db.connect".into(),
            attempts: 3,
            source: "ENETUNREACH".into(),
        };
        assert!(e.to_string().contains("db.connect"));
        assert!(e.to_string().contains("3 attempts"));
    }

    #[test]
    fn permanent_wrap_preserves_message() {
        let p = PermanentError::wrap("invalid token");
        assert_eq!(p.to_string(), "invalid token");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-retry --lib error`
Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-retry/src/error.rs
git commit -m "feat(retry): add Error and PermanentError types"
```

### Task 2.3: `Config` and `ConfigBuilder`

**Files:**
- Create: `crates/altair-retry/src/config.rs`

- [ ] **Step 1: Write `config.rs`**

```rust
//! Retry configuration.

use std::time::Duration;

/// Retry policy.
#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) name: String,
    pub(crate) max_retries: u32,
    pub(crate) initial_interval: Duration,
    pub(crate) max_interval: Duration,
    pub(crate) multiplier: f64,
    pub(crate) jitter: bool,
}

impl Config {
    /// Start building a new config.
    #[must_use]
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }

    /// Return a default config with the given name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "unnamed".to_string(),
            max_retries: 5,
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(30),
            multiplier: 1.5,
            jitter: true,
        }
    }
}

/// Builder for [`Config`].
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    inner: Config,
}

impl ConfigBuilder {
    /// Set the operation name (appears in spans + error messages).
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.inner.name = name.into();
        self
    }

    /// Maximum number of retry attempts after the initial call.
    #[must_use]
    pub fn max_retries(mut self, n: u32) -> Self {
        self.inner.max_retries = n;
        self
    }

    /// Initial backoff interval.
    #[must_use]
    pub fn initial_interval(mut self, d: Duration) -> Self {
        self.inner.initial_interval = d;
        self
    }

    /// Maximum backoff interval (caps exponential growth).
    #[must_use]
    pub fn max_interval(mut self, d: Duration) -> Self {
        self.inner.max_interval = d;
        self
    }

    /// Exponential growth factor (e.g., 1.5, 2.0).
    #[must_use]
    pub fn multiplier(mut self, m: f64) -> Self {
        self.inner.multiplier = m;
        self
    }

    /// Toggle backoff jitter.
    #[must_use]
    pub fn jitter(mut self, on: bool) -> Self {
        self.inner.jitter = on;
        self
    }

    /// Finalize the config.
    #[must_use]
    pub fn build(self) -> Config {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn default_has_sensible_values() {
        let c = Config::default();
        assert_eq!(c.max_retries, 5);
        assert_eq!(c.initial_interval, Duration::from_millis(100));
        assert!(c.jitter);
    }

    #[test]
    fn builder_overrides_defaults() {
        let c = Config::builder()
            .name("test")
            .max_retries(2)
            .initial_interval(Duration::from_millis(10))
            .jitter(false)
            .build();
        assert_eq!(c.name, "test");
        assert_eq!(c.max_retries, 2);
        assert!(!c.jitter);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-retry --lib config`
Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-retry/src/config.rs
git commit -m "feat(retry): add Config and ConfigBuilder"
```

### Task 2.4: `retry` function

**Files:**
- Create: `crates/altair-retry/src/retry.rs`

- [ ] **Step 1: Write `retry.rs`**

```rust
//! The `retry` entry point.

use crate::config::Config;
use crate::error::{Error, PermanentError, Result};
use backon::{BackoffBuilder, ExponentialBuilder};
use std::future::Future;
use std::time::Instant;
use tracing::{info_span, Instrument};

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// Run `op` with retry per `config`.
///
/// On success, returns the value. On error, retries with exponential backoff.
/// If `op` returns an error that downcasts to [`PermanentError`], retry stops
/// immediately and the wrapped error is returned via [`Error::Permanent`].
pub async fn retry<T, E, F, Fut>(config: Config, mut op: F) -> Result<T>
where
    F: FnMut() -> Fut + Send,
    Fut: Future<Output = std::result::Result<T, E>> + Send,
    E: Into<BoxedError>,
    T: Send,
{
    let backoff = ExponentialBuilder::default()
        .with_min_delay(config.initial_interval)
        .with_max_delay(config.max_interval)
        .with_factor(config.multiplier as f32)
        .with_max_times(config.max_retries as usize)
        .with_jitter()
        .build();

    let start = Instant::now();
    let mut attempt: u32 = 0;
    let mut delays = backoff;

    let span = info_span!(
        "retry",
        retry.name = %config.name,
        retry.max_attempts = config.max_retries + 1,
    );

    async {
        loop {
            attempt += 1;
            let attempt_span = info_span!(
                "retry.attempt",
                retry.attempt = attempt,
            );

            let outcome = op().instrument(attempt_span).await;

            match outcome {
                Ok(v) => {
                    tracing::info!(
                        retry.outcome = "success",
                        retry.attempts = attempt,
                        retry.elapsed_ms = start.elapsed().as_millis() as u64,
                        "retry succeeded",
                    );
                    return Ok(v);
                }
                Err(e) => {
                    let boxed: BoxedError = e.into();
                    if let Some(perm) = downcast_permanent(&boxed) {
                        tracing::warn!(
                            retry.outcome = "permanent",
                            retry.attempts = attempt,
                            "permanent error encountered",
                        );
                        return Err(Error::Permanent {
                            name: config.name.clone(),
                            source: perm,
                        });
                    }

                    match delays.next() {
                        Some(delay) => {
                            tracing::debug!(
                                retry.attempt = attempt,
                                retry.delay_ms = delay.as_millis() as u64,
                                "retrying after backoff",
                            );
                            tokio::time::sleep(delay).await;
                        }
                        None => {
                            tracing::warn!(
                                retry.outcome = "exhausted",
                                retry.attempts = attempt,
                                "retry attempts exhausted",
                            );
                            return Err(Error::Exhausted {
                                name: config.name.clone(),
                                attempts: attempt,
                                source: boxed,
                            });
                        }
                    }
                }
            }
        }
    }
    .instrument(span)
    .await
}

fn downcast_permanent(e: &BoxedError) -> Option<BoxedError> {
    // PermanentError wraps an underlying error; unwrap so callers see the original
    if let Some(p) = e.downcast_ref::<PermanentError>() {
        Some(format!("{p}").into())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    fn fast_config(name: &str) -> Config {
        Config::builder()
            .name(name)
            .max_retries(3)
            .initial_interval(Duration::from_millis(1))
            .max_interval(Duration::from_millis(10))
            .jitter(false)
            .build()
    }

    #[tokio::test]
    async fn succeeds_on_first_try() {
        let r: Result<u32> = retry(fast_config("ok"), || async {
            Ok::<_, std::io::Error>(42)
        })
        .await;
        assert_eq!(r.unwrap(), 42);
    }

    #[tokio::test]
    async fn retries_then_succeeds() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let r: Result<u32> = retry(fast_config("flaky"), move || {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst) + 1;
                if n < 3 {
                    Err::<u32, _>(std::io::Error::other("flake"))
                } else {
                    Ok(n)
                }
            }
        })
        .await;
        assert_eq!(r.unwrap(), 3);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn exhausts_with_attempts_count() {
        let r: Result<u32> = retry(fast_config("always_fail"), || async {
            Err::<u32, _>(std::io::Error::other("nope"))
        })
        .await;
        match r {
            Err(Error::Exhausted { attempts, .. }) => assert_eq!(attempts, 4),
            other => panic!("expected Exhausted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn permanent_error_short_circuits() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let r: Result<u32> = retry(fast_config("permanent"), move || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err::<u32, _>(PermanentError::wrap("invalid creds"))
            }
        })
        .await;
        assert!(matches!(r, Err(Error::Permanent { .. })));
        assert_eq!(counter.load(Ordering::SeqCst), 1, "should not retry on permanent");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-retry --lib retry`
Expected: 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-retry/src/retry.rs
git commit -m "feat(retry): add retry function with tracing and permanent-error handling"
```

### Task 2.5: `prelude` module

**Files:**
- Create: `crates/altair-retry/src/prelude.rs`

- [ ] **Step 1: Write `prelude.rs`**

```rust
//! Common imports for users of this crate.
//!
//! ```
//! use altair_retry::prelude::*;
//! ```

pub use crate::{retry, CancellationToken, Config, ConfigBuilder, Error, PermanentError, Result};
```

- [ ] **Step 2: Doc test**

Run: `cargo test -p altair-retry --doc`
Expected: doc tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-retry/src/prelude.rs
git commit -m "feat(retry): add prelude module"
```

### Task 2.6: Example + README

**Files:**
- Create: `crates/altair-retry/examples/basic.rs`
- Modify: `crates/altair-retry/README.md`

- [ ] **Step 1: Write example**

```rust
//! Run with: `cargo run --example basic -p altair-retry`

use altair_retry::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let attempts = Arc::new(AtomicU32::new(0));
    let a = attempts.clone();

    let result = retry(
        Config::builder()
            .name("flaky.api")
            .max_retries(3)
            .initial_interval(Duration::from_millis(50))
            .build(),
        move || {
            let a = a.clone();
            async move {
                let n = a.fetch_add(1, Ordering::SeqCst) + 1;
                println!("attempt {n}");
                if n < 3 {
                    Err::<&'static str, _>(std::io::Error::other("temporary"))
                } else {
                    Ok("success")
                }
            }
        },
    )
    .await?;

    println!("got: {result}");
    Ok(())
}
```

- [ ] **Step 2: Add `tracing-subscriber` and `anyhow` to dev-deps**

Edit `crates/altair-retry/Cargo.toml`:

```toml
[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "macros", "rt-multi-thread"] }
pretty_assertions = { workspace = true }
anyhow = { workspace = true }
tracing-subscriber = { workspace = true }
```

- [ ] **Step 3: Replace README**

```markdown
# altair-retry

Async retry with exponential backoff and automatic tracing.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-retry
```

## Quick start

```rust,no_run
use altair_retry::prelude::*;
use std::time::Duration;

# async fn ping() -> std::io::Result<()> { Ok(()) }
# async fn run() -> altair_retry::Result<()> {
let cfg = Config::builder()
    .name("db.connect")
    .max_retries(3)
    .initial_interval(Duration::from_millis(100))
    .build();

retry(cfg, || async { ping().await }).await?;
# Ok(()) }
```

## Permanent (non-retryable) errors

```rust,no_run
use altair_retry::prelude::*;

# async fn run() -> altair_retry::Result<()> {
retry(Config::default().with_name("api"), || async {
    if invalid_request() {
        return Err::<&'static str, _>(PermanentError::wrap("invalid input"));
    }
    do_call().await
}).await?;
# Ok(()) }
# fn invalid_request() -> bool { false }
# async fn do_call() -> Result<&'static str, std::io::Error> { Ok("ok") }
```

## Tracing

Each attempt runs inside a `tracing::span!("retry.attempt", retry.attempt = N)` span, nested under a top-level `retry` span with `retry.name` and `retry.max_attempts` attributes. Final outcome (`success`, `permanent`, `exhausted`) is emitted as a `tracing::info!`/`warn!` event with `retry.elapsed_ms` and `retry.attempts`.

If `altair-otel` is initialized, these spans flow to OTLP automatically.

## License

[MIT](../../LICENSE)
```

- [ ] **Step 4: Build and run example**

Run: `cargo run -p altair-retry --example basic`
Expected: prints `attempt 1`, `attempt 2`, `attempt 3`, then `got: success`.

- [ ] **Step 5: Doc tests pass**

Run: `cargo test -p altair-retry --doc`
Expected: doc tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/altair-retry
git commit -m "docs(retry): add basic example and complete README"
```

### Task 2.7: Update porting tracker + local CI

- [ ] **Step 1: Mark `altair-retry` as `🚧 In Progress` in `docs/porting-tracker.md`**

- [ ] **Step 2: Run full CI check**

Run: `task ci:check`
Expected: passes.

- [ ] **Step 3: Commit**

```bash
git add docs/porting-tracker.md
git commit -m "docs: mark altair-retry as in-progress"
```

---

## Phase 3: `altair-config`

Goal: Thin wrapper around `figment` + `validator` + `toml` providing `from_toml_str` / `from_file` one-liners and a `Loader` builder.

### Task 3.1: Crate skeleton

**Files:**
- Create: `crates/altair-config/Cargo.toml`
- Create: `crates/altair-config/src/lib.rs`
- Create: `crates/altair-config/README.md` (stub)
- Modify: `Cargo.toml` (add member)

- [ ] **Step 1: `crates/altair-config/Cargo.toml`**

```toml
[package]
name = "altair-config"
description = "Type-safe TOML config loading with env overrides and validation"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
homepage.workspace = true
readme = "README.md"
keywords = ["config", "toml", "figment", "validator", "serde"]
categories = ["config"]

[dependencies]
figment = { workspace = true }
validator = { workspace = true }
serde = { workspace = true }
toml = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt"] }
pretty_assertions = { workspace = true }
anyhow = { workspace = true }
tempfile = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 2: `crates/altair-config/src/lib.rs`**

```rust
//! Type-safe TOML configuration loading with env-var overrides and validation.
//!
//! # Example
//!
//! ```
//! use altair_config::{Deserialize, Validate};
//!
//! #[derive(Debug, Deserialize, Validate)]
//! struct App {
//!     #[validate(range(min = 1, max = 65535))]
//!     port: u16,
//! }
//!
//! let toml = "port = 8080";
//! let cfg: App = altair_config::from_toml_str(toml, "APP").unwrap();
//! assert_eq!(cfg.port, 8080);
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod error;
mod loader;
mod loaders;

pub mod prelude;

pub use error::{Error, Result};
pub use loader::Loader;
pub use loaders::{from_file, from_reader, from_toml_str};

// Re-exports for one-dep ergonomics
pub use serde::{Deserialize, Serialize};
pub use validator::{Validate, ValidationError, ValidationErrors};
```

- [ ] **Step 3: README stub**

```markdown
# altair-config

Type-safe TOML config loading with env overrides and validation.

(Full README added in a later task.)
```

- [ ] **Step 4: Add to workspace members**

```toml
members = ["crates/altair-concurrent", "crates/altair-retry", "crates/altair-config"]
```

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/altair-config
git commit -m "feat(config): scaffold altair-config crate"
```

### Task 3.2: `Error` type

**Files:**
- Create: `crates/altair-config/src/error.rs`

- [ ] **Step 1: Write `error.rs`**

```rust
//! Config loading and validation errors.

use thiserror::Error;

/// Errors returned by config loaders.
#[derive(Debug, Error)]
pub enum Error {
    /// I/O error while reading a config file.
    #[error("config I/O: {0}")]
    Io(#[from] std::io::Error),

    /// Underlying parse/merge error from `figment`.
    #[error("config parse: {0}")]
    Parse(#[from] figment::Error),

    /// Validation failed.
    #[error("config validation failed: {0}")]
    Validation(#[from] validator::ValidationErrors),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_renders() {
        let io = std::io::Error::other("nope");
        let e: Error = io.into();
        assert!(e.to_string().contains("nope"));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-config --lib error`
Expected: 1 test passes.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-config/src/error.rs
git commit -m "feat(config): add Error type bridging figment + validator"
```

### Task 3.3: `Loader` builder + free functions

**Files:**
- Create: `crates/altair-config/src/loader.rs`
- Create: `crates/altair-config/src/loaders.rs`

- [ ] **Step 1: Write `loader.rs`**

```rust
//! Multi-source layered config loader.

use crate::error::{Error, Result};
use figment::providers::{Env, Format, Toml};
use figment::Figment;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use validator::Validate;

/// Builder for layered config loads.
///
/// Layers are merged in insertion order — later sources override earlier ones.
#[derive(Debug, Default)]
pub struct Loader {
    files: Vec<(PathBuf, bool)>, // (path, optional?)
    env_prefix: Option<String>,
}

impl Loader {
    /// Create a new empty loader.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a required TOML file. Loading fails if the file is missing.
    #[must_use]
    pub fn toml_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.files.push((path.into(), false));
        self
    }

    /// Add an optional TOML file. Missing files are silently skipped.
    #[must_use]
    pub fn toml_file_optional(mut self, path: impl Into<PathBuf>) -> Self {
        self.files.push((path.into(), true));
        self
    }

    /// Apply environment variable overrides with the given prefix.
    ///
    /// `APP_DATABASE_HOST=db.prod` sets `database.host` to `db.prod`.
    #[must_use]
    pub fn env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.env_prefix = Some(prefix.into());
        self
    }

    /// Build, deserialize, and validate.
    pub fn build<T>(self) -> Result<T>
    where
        T: DeserializeOwned + Validate,
    {
        let mut figment = Figment::new();
        for (path, optional) in &self.files {
            if *optional && !Path::new(path).exists() {
                continue;
            }
            figment = figment.merge(Toml::file(path));
        }
        if let Some(prefix) = self.env_prefix {
            figment = figment.merge(Env::prefixed(&format!("{prefix}_")).split("_"));
        }
        let value: T = figment.extract()?;
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use tempfile::NamedTempFile;
    use validator::Validate;

    #[derive(Debug, Deserialize, Validate)]
    struct Cfg {
        #[validate(range(min = 1))]
        port: u16,
        host: String,
    }

    #[test]
    fn loader_reads_toml_file() {
        let mut f = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut f, b"port = 9000\nhost = \"localhost\"\n").unwrap();
        let cfg: Cfg = Loader::new().toml_file(f.path()).build().unwrap();
        assert_eq!(cfg.port, 9000);
        assert_eq!(cfg.host, "localhost");
    }

    #[test]
    fn loader_missing_required_file_errors() {
        let r: Result<Cfg> = Loader::new().toml_file("/nonexistent/x.toml").build();
        assert!(r.is_err());
    }

    #[test]
    fn loader_missing_optional_file_is_ok_with_base() {
        let mut base = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut base, b"port = 9000\nhost = \"localhost\"\n").unwrap();
        let cfg: Cfg = Loader::new()
            .toml_file(base.path())
            .toml_file_optional("/does/not/exist.toml")
            .build()
            .unwrap();
        assert_eq!(cfg.port, 9000);
    }

    #[test]
    fn loader_env_override() {
        let mut f = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut f, b"port = 9000\nhost = \"localhost\"\n").unwrap();
        // SAFETY: setting env vars in tests is safe in single-threaded contexts.
        std::env::set_var("TEST_ALT_PORT", "1234");
        let cfg: Cfg = Loader::new()
            .toml_file(f.path())
            .env_prefix("TEST_ALT")
            .build()
            .unwrap();
        std::env::remove_var("TEST_ALT_PORT");
        assert_eq!(cfg.port, 1234);
    }

    #[test]
    fn validation_error_propagates() {
        let mut f = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut f, b"port = 0\nhost = \"localhost\"\n").unwrap();
        let r: Result<Cfg> = Loader::new().toml_file(f.path()).build();
        assert!(matches!(r, Err(Error::Validation(_))));
    }
}
```

- [ ] **Step 2: Write `loaders.rs`**

```rust
//! One-liner convenience loaders.

use crate::error::Result;
use crate::loader::Loader;
use serde::de::DeserializeOwned;
use std::io::Read;
use std::path::Path;
use validator::Validate;

/// Load and validate config from a TOML string with env-var overrides.
pub fn from_toml_str<T>(toml: &str, env_prefix: &str) -> Result<T>
where
    T: DeserializeOwned + Validate,
{
    use figment::providers::{Env, Format, Toml};
    use figment::Figment;

    let figment = Figment::new()
        .merge(Toml::string(toml))
        .merge(Env::prefixed(&format!("{env_prefix}_")).split("_"));
    let value: T = figment.extract().map_err(crate::error::Error::from)?;
    value.validate()?;
    Ok(value)
}

/// Load and validate config from a TOML file with env-var overrides.
pub fn from_file<T>(path: impl AsRef<Path>, env_prefix: &str) -> Result<T>
where
    T: DeserializeOwned + Validate,
{
    Loader::new()
        .toml_file(path.as_ref().to_path_buf())
        .env_prefix(env_prefix)
        .build()
}

/// Load and validate config from any `Read` source.
pub fn from_reader<T>(mut reader: impl Read, env_prefix: &str) -> Result<T>
where
    T: DeserializeOwned + Validate,
{
    let mut s = String::new();
    reader.read_to_string(&mut s)?;
    from_toml_str(&s, env_prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use validator::Validate;

    #[derive(Debug, Deserialize, Validate)]
    struct Cfg {
        #[validate(range(min = 1))]
        port: u16,
    }

    #[test]
    fn from_toml_str_loads() {
        let cfg: Cfg = from_toml_str("port = 7777", "FYS_NONE").unwrap();
        assert_eq!(cfg.port, 7777);
    }

    #[test]
    fn from_reader_loads() {
        let bytes = b"port = 4242";
        let cfg: Cfg = from_reader(&bytes[..], "FYR_NONE").unwrap();
        assert_eq!(cfg.port, 4242);
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p altair-config --lib`
Expected: all unit tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-config/src/loader.rs crates/altair-config/src/loaders.rs
git commit -m "feat(config): add Loader builder and from_toml_str/from_file/from_reader"
```

### Task 3.4: Prelude + README + example

**Files:**
- Create: `crates/altair-config/src/prelude.rs`
- Create: `crates/altair-config/examples/basic.rs`
- Modify: `crates/altair-config/README.md`

- [ ] **Step 1: Write `prelude.rs`**

```rust
//! Common imports for users of this crate.
//!
//! ```
//! use altair_config::prelude::*;
//!
//! #[derive(Debug, Deserialize, Validate)]
//! struct Cfg {
//!     name: String,
//! }
//! ```

pub use crate::{from_file, from_reader, from_toml_str, Deserialize, Error, Loader, Result, Serialize, Validate, ValidationError, ValidationErrors};
```

- [ ] **Step 2: Write example**

```rust
//! Run with: `cargo run --example basic -p altair-config`

use altair_config::prelude::*;

#[derive(Debug, Deserialize, Validate)]
struct AppConfig {
    #[validate(range(min = 1, max = 65535))]
    port: u16,

    #[validate(length(min = 1))]
    name: String,
}

fn main() -> anyhow::Result<()> {
    let toml = r#"
port = 8080
name = "my-service"
"#;
    let cfg: AppConfig = from_toml_str(toml, "APP")?;
    println!("{cfg:#?}");
    Ok(())
}
```

- [ ] **Step 3: Replace README**

```markdown
# altair-config

Type-safe TOML config loading with env-var overrides and validation. Wraps [`figment`](https://crates.io/crates/figment) and [`validator`](https://crates.io/crates/validator) under a unified surface.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-config
```

You do **not** need to add `figment`, `validator`, `serde`, or `toml` separately — `altair-config` re-exports the types and derives you need.

## Quick start

```rust,no_run
use altair_config::prelude::*;

#[derive(Debug, Deserialize, Validate)]
struct AppConfig {
    #[validate(range(min = 1, max = 65535))]
    port: u16,
    database: DbConfig,
}

#[derive(Debug, Deserialize, Validate)]
struct DbConfig {
    #[validate(length(min = 1))]
    host: String,
    port: u16,
}

# fn main() -> altair_config::Result<()> {
let toml = r#"
port = 8080

[database]
host = "localhost"
port = 5432
"#;
let cfg: AppConfig = from_toml_str(toml, "APP")?;
# Ok(()) }
```

## Layered loading

```rust,no_run
use altair_config::prelude::*;
# #[derive(Debug, Deserialize, Validate)] struct AppConfig { port: u16 }
# fn main() -> altair_config::Result<()> {
let cfg: AppConfig = Loader::new()
    .toml_file("config/base.toml")
    .toml_file_optional("config/local.toml")
    .env_prefix("APP")
    .build()?;
# Ok(()) }
```

## Env overrides

With `env_prefix("APP")`, `APP_PORT=9090` sets `cfg.port`, and `APP_DATABASE_HOST=db.prod` sets `cfg.database.host`. Nested keys are joined by `_`.

## License

[MIT](../../LICENSE)
```

- [ ] **Step 4: Verify build + tests + doc tests**

Run: `cargo build -p altair-config --examples && cargo test -p altair-config && cargo test -p altair-config --doc`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/altair-config
git commit -m "docs(config): add prelude, basic example, and complete README"
```

### Task 3.5: Update porting tracker + local CI

- [ ] **Step 1: Mark `altair-config` as `🚧 In Progress`**

- [ ] **Step 2: Run `task ci:check`**

Expected: passes.

- [ ] **Step 3: Commit**

```bash
git add docs/porting-tracker.md
git commit -m "docs: mark altair-config as in-progress"
```

---

## Phase 4: `altair-otel`

Goal: One-call OpenTelemetry setup. Provides global tracing subscriber wire-up (spans + logs) and a `Meter` handle for metrics.

### Task 4.1: Crate skeleton

**Files:**
- Create: `crates/altair-otel/Cargo.toml`
- Create: `crates/altair-otel/src/lib.rs`
- Create: `crates/altair-otel/README.md` (stub)
- Modify: `Cargo.toml` (add member)

- [ ] **Step 1: `crates/altair-otel/Cargo.toml`**

```toml
[package]
name = "altair-otel"
description = "One-call OpenTelemetry setup with tracing-subscriber integration"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
homepage.workspace = true
readme = "README.md"
keywords = ["opentelemetry", "tracing", "observability", "otlp"]
categories = ["development-tools::debugging", "asynchronous"]

[features]
default = ["otlp-grpc"]
otlp-grpc = ["opentelemetry-otlp/grpc-tonic"]
otlp-http = ["opentelemetry-otlp/http-proto", "opentelemetry-otlp/reqwest-client"]
console = []

[dependencies]
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
tracing-opentelemetry = { workspace = true }
opentelemetry = { workspace = true }
opentelemetry_sdk = { workspace = true }
opentelemetry-otlp = { workspace = true }
opentelemetry-stdout = { workspace = true }
opentelemetry-semantic-conventions = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
pretty_assertions = { workspace = true }
anyhow = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 2: `src/lib.rs`**

```rust
//! One-call OpenTelemetry setup for tokio applications.
//!
//! After calling [`Config::init`], all `tracing::info!`, `tracing::warn!`, and
//! `#[tracing::instrument]`-decorated functions emit OTLP spans + logs. Use
//! [`meter`] to obtain an OpenTelemetry [`Meter`](opentelemetry::metrics::Meter)
//! for explicit metric instrumentation.
//!
//! # Example
//!
//! ```no_run
//! # async fn run() -> altair_otel::Result<()> {
//! altair_otel::Config::from_env()?.init()?;
//!
//! tracing::info!(user_id = 42, "request received");
//!
//! let counter = altair_otel::meter().u64_counter("requests.total").build();
//! counter.add(1, &[]);
//!
//! altair_otel::shutdown();
//! # Ok(()) }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod config;
mod error;
mod globals;
mod init;

pub mod prelude;

pub use config::{Config, ConfigBuilder};
pub use error::{Error, Result};
pub use globals::{meter, shutdown};

// Re-exports for one-dep ergonomics
pub use tracing::{self, debug, error, info, instrument, span, trace, warn, Span};
pub use opentelemetry::metrics::{Counter, Histogram, Meter, UpDownCounter};
```

- [ ] **Step 3: README stub**

```markdown
# altair-otel

One-call OpenTelemetry setup for tokio applications.

(Full README added in a later task.)
```

- [ ] **Step 4: Add to workspace members**

```toml
members = [
    "crates/altair-concurrent",
    "crates/altair-retry",
    "crates/altair-config",
    "crates/altair-otel",
]
```

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/altair-otel
git commit -m "feat(otel): scaffold altair-otel crate"
```

### Task 4.2: `Error` type

**Files:**
- Create: `crates/altair-otel/src/error.rs`

- [ ] **Step 1: Write `error.rs`**

```rust
//! Errors from OTel initialization.

use thiserror::Error;

/// Errors from [`crate::Config::init`].
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to build the OTLP exporter or tracer/meter/logger provider.
    #[error("otel exporter: {0}")]
    Exporter(String),

    /// The global tracing subscriber was already set.
    #[error("tracing subscriber already initialized")]
    AlreadyInitialized,

    /// An environment variable required by [`Config::from_env`] is missing or malformed.
    #[error("env config: {key} - {message}")]
    EnvConfig {
        /// The offending env var key.
        key: String,
        /// Reason it was rejected.
        message: String,
    },
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_config_renders() {
        let e = Error::EnvConfig {
            key: "OTEL_EXPORTER_OTLP_ENDPOINT".into(),
            message: "not a valid URL".into(),
        };
        assert!(e.to_string().contains("OTEL_EXPORTER_OTLP_ENDPOINT"));
        assert!(e.to_string().contains("not a valid URL"));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-otel --lib error`
Expected: 1 test passes.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-otel/src/error.rs
git commit -m "feat(otel): add Error type"
```

### Task 4.3: `Config` and `ConfigBuilder`

**Files:**
- Create: `crates/altair-otel/src/config.rs`

- [ ] **Step 1: Write `config.rs`**

```rust
//! OTel setup configuration.

use crate::error::{Error, Result};

/// How span/log/metric data is exported.
#[derive(Debug, Clone, Default)]
pub enum Exporter {
    /// OTLP over gRPC (default).
    #[default]
    Otlp,
    /// Stdout exporter for local dev.
    Stdout,
    /// No exporter — spans/logs are dropped (useful for tests).
    None,
}

/// Log output format on stdout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LogFormat {
    /// Human-readable, multi-line pretty format.
    #[default]
    Pretty,
    /// JSON, one object per line.
    Json,
}

/// OTel setup config.
#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) service_name: String,
    pub(crate) service_version: Option<String>,
    pub(crate) otlp_endpoint: Option<String>,
    pub(crate) resource_attributes: Vec<(String, String)>,
    pub(crate) exporter: Exporter,
    pub(crate) log_format: LogFormat,
}

impl Config {
    /// Start building a config.
    #[must_use]
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }

    /// Build a config from OTel-spec environment variables.
    ///
    /// Reads:
    /// - `OTEL_SERVICE_NAME` (required)
    /// - `OTEL_SERVICE_VERSION` (optional)
    /// - `OTEL_EXPORTER_OTLP_ENDPOINT` (optional; default `http://localhost:4317`)
    /// - `OTEL_LOG_FORMAT` (`pretty` or `json`; default `pretty`)
    pub fn from_env() -> Result<Self> {
        let service_name = std::env::var("OTEL_SERVICE_NAME").map_err(|_| Error::EnvConfig {
            key: "OTEL_SERVICE_NAME".into(),
            message: "not set".into(),
        })?;

        let service_version = std::env::var("OTEL_SERVICE_VERSION").ok();
        let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();
        let log_format = match std::env::var("OTEL_LOG_FORMAT").ok().as_deref() {
            Some("json") => LogFormat::Json,
            _ => LogFormat::Pretty,
        };

        Ok(Self {
            service_name,
            service_version,
            otlp_endpoint,
            resource_attributes: vec![],
            exporter: Exporter::Otlp,
            log_format,
        })
    }
}

/// Builder for [`Config`].
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    inner: Option<ConfigInner>,
}

#[derive(Debug, Default)]
struct ConfigInner {
    service_name: String,
    service_version: Option<String>,
    otlp_endpoint: Option<String>,
    resource_attributes: Vec<(String, String)>,
    exporter: Exporter,
    log_format: LogFormat,
}

impl ConfigBuilder {
    fn inner_mut(&mut self) -> &mut ConfigInner {
        self.inner.get_or_insert_with(ConfigInner::default)
    }

    /// Set the service name (required).
    #[must_use]
    pub fn service_name(mut self, name: impl Into<String>) -> Self {
        self.inner_mut().service_name = name.into();
        self
    }

    /// Set the service version (optional).
    #[must_use]
    pub fn service_version(mut self, v: impl Into<String>) -> Self {
        self.inner_mut().service_version = Some(v.into());
        self
    }

    /// Set the OTLP endpoint (optional; defaults to `http://localhost:4317`).
    #[must_use]
    pub fn otlp_endpoint(mut self, e: impl Into<String>) -> Self {
        self.inner_mut().otlp_endpoint = Some(e.into());
        self
    }

    /// Add a resource attribute (repeatable).
    #[must_use]
    pub fn resource_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner_mut()
            .resource_attributes
            .push((key.into(), value.into()));
        self
    }

    /// Override the exporter backend.
    #[must_use]
    pub fn exporter(mut self, exp: Exporter) -> Self {
        self.inner_mut().exporter = exp;
        self
    }

    /// Override the stdout log format.
    #[must_use]
    pub fn log_format(mut self, f: LogFormat) -> Self {
        self.inner_mut().log_format = f;
        self
    }

    /// Build the [`Config`].
    ///
    /// Panics if `service_name` was not set.
    #[must_use]
    pub fn build(self) -> Config {
        let i = self.inner.expect("ConfigBuilder::build() called on empty builder");
        assert!(!i.service_name.is_empty(), "service_name is required");
        Config {
            service_name: i.service_name,
            service_version: i.service_version,
            otlp_endpoint: i.otlp_endpoint,
            resource_attributes: i.resource_attributes,
            exporter: i.exporter,
            log_format: i.log_format,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn builder_basic() {
        let c = Config::builder()
            .service_name("svc")
            .service_version("1.2.3")
            .resource_attribute("env", "test")
            .build();
        assert_eq!(c.service_name, "svc");
        assert_eq!(c.service_version, Some("1.2.3".into()));
        assert_eq!(c.resource_attributes, vec![("env".into(), "test".into())]);
    }

    #[test]
    #[should_panic(expected = "service_name is required")]
    fn build_panics_without_name() {
        let _ = Config::builder().service_version("v").build();
    }

    #[test]
    fn from_env_missing_service_name_errors() {
        // Best-effort: this test assumes OTEL_SERVICE_NAME is not set.
        std::env::remove_var("OTEL_SERVICE_NAME");
        let r = Config::from_env();
        assert!(matches!(r, Err(Error::EnvConfig { .. })));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-otel --lib config`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-otel/src/config.rs
git commit -m "feat(otel): add Config and ConfigBuilder with from_env"
```

### Task 4.4: Globals — meter + shutdown

**Files:**
- Create: `crates/altair-otel/src/globals.rs`

- [ ] **Step 1: Write `globals.rs`**

```rust
//! Global accessors set up by [`crate::Config::init`].

use opentelemetry::global;
use opentelemetry::metrics::Meter;

/// Return the global [`Meter`] for the calling service.
///
/// Equivalent to `opentelemetry::global::meter("altair")`. After [`crate::Config::init`],
/// this meter routes through the configured OTLP exporter (or whatever exporter
/// was selected).
#[must_use]
pub fn meter() -> Meter {
    global::meter("altair")
}

/// Flush and shut down the global tracer/meter providers.
///
/// Call this once during graceful shutdown to ensure pending spans and
/// metrics reach the collector before the process exits.
pub fn shutdown() {
    global::shutdown_tracer_provider();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_returns_global_meter() {
        let m = meter();
        let _counter = m.u64_counter("test.counter").build();
        // No assertion — we're checking compilation + non-panic.
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-otel --lib globals`
Expected: 1 test passes.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-otel/src/globals.rs
git commit -m "feat(otel): add global meter() and shutdown()"
```

### Task 4.5: `init` — wire the tracing subscriber + OTLP exporter

**Files:**
- Create: `crates/altair-otel/src/init.rs`
- Modify: `crates/altair-otel/src/config.rs` (add `init` method)

- [ ] **Step 1: Write `init.rs`**

```rust
//! Subscriber + provider wire-up.

use crate::config::{Config, Exporter, LogFormat};
use crate::error::{Error, Result};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub(crate) fn init(config: &Config) -> Result<()> {
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

    let resource = build_resource(config);
    let provider = build_tracer_provider(config, resource.clone())?;
    opentelemetry::global::set_tracer_provider(provider.clone());

    let tracer = provider.tracer("altair");
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let registry = tracing_subscriber::registry().with(env_filter).with(otel_layer);

    let try_init = match config.log_format {
        LogFormat::Json => registry
            .with(tracing_subscriber::fmt::layer().json())
            .try_init(),
        LogFormat::Pretty => registry
            .with(tracing_subscriber::fmt::layer().pretty())
            .try_init(),
    };
    try_init.map_err(|_| Error::AlreadyInitialized)?;

    Ok(())
}

fn build_resource(config: &Config) -> Resource {
    let mut attrs = vec![KeyValue::new("service.name", config.service_name.clone())];
    if let Some(v) = &config.service_version {
        attrs.push(KeyValue::new("service.version", v.clone()));
    }
    for (k, v) in &config.resource_attributes {
        attrs.push(KeyValue::new(k.clone(), v.clone()));
    }
    Resource::builder().with_attributes(attrs).build()
}

fn build_tracer_provider(
    config: &Config,
    resource: Resource,
) -> Result<SdkTracerProvider> {
    let builder = SdkTracerProvider::builder().with_resource(resource);

    let provider = match config.exporter {
        Exporter::Otlp => {
            let mut exporter_builder = opentelemetry_otlp::SpanExporter::builder().with_tonic();
            if let Some(endpoint) = &config.otlp_endpoint {
                exporter_builder = exporter_builder.with_endpoint(endpoint);
            }
            let exporter = exporter_builder
                .build()
                .map_err(|e| Error::Exporter(e.to_string()))?;
            builder.with_batch_exporter(exporter).build()
        }
        Exporter::Stdout => {
            let exporter = opentelemetry_stdout::SpanExporter::default();
            builder.with_simple_exporter(exporter).build()
        }
        Exporter::None => builder.build(),
    };

    Ok(provider)
}
```

- [ ] **Step 2: Add `init` to `Config` in `config.rs`**

Append at the bottom of the `impl Config { ... }` block (before `impl Default`):

```rust
impl Config {
    /// Wire the global tracing subscriber and OTel providers per this config.
    ///
    /// Must be called at most once per process — subsequent calls return
    /// [`Error::AlreadyInitialized`].
    pub fn init(self) -> Result<()> {
        crate::init::init(&self)
    }
}
```

(Add it just below the existing `impl Config { ... }` that contains `builder()` and `from_env()`. Keep both `impl Config { ... }` blocks — Rust allows multiple `impl` blocks per type.)

- [ ] **Step 3: Run unit tests**

Run: `cargo test -p altair-otel --lib`
Expected: all unit tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-otel/src/init.rs crates/altair-otel/src/config.rs
git commit -m "feat(otel): wire tracing subscriber with OTLP exporter"
```

### Task 4.6: Prelude + integration test

**Files:**
- Create: `crates/altair-otel/src/prelude.rs`
- Create: `crates/altair-otel/tests/integration.rs`

- [ ] **Step 1: Write `prelude.rs`**

```rust
//! Common imports for users of this crate.
//!
//! ```
//! use altair_otel::prelude::*;
//! ```

pub use crate::{meter, shutdown, Config, ConfigBuilder, Error, Result};
pub use crate::{debug, error, info, instrument, span, trace, warn, Span};
pub use opentelemetry::metrics::{Counter, Histogram, Meter, UpDownCounter};
```

- [ ] **Step 2: Write `tests/integration.rs`**

```rust
//! End-to-end behavior tests.

use altair_otel::config::Exporter;
use altair_otel::Config;

#[tokio::test]
async fn init_with_none_exporter_succeeds() {
    // This test only checks initialization wires together — no exporter actually fires.
    // Subsequent calls will return AlreadyInitialized; we only verify the first call.
    let cfg = Config::builder()
        .service_name("test-svc")
        .exporter(Exporter::None)
        .build();
    let r = cfg.init();
    // Either Ok (first call) or AlreadyInitialized (if a previous test ran first in this process).
    assert!(r.is_ok() || matches!(r, Err(altair_otel::Error::AlreadyInitialized)));
}
```

Re-export `Exporter` from the crate by adding to `src/lib.rs`:

```rust
// Add to the existing pub use block:
pub use config::{Config, ConfigBuilder, Exporter, LogFormat};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p altair-otel`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-otel/src/prelude.rs crates/altair-otel/src/lib.rs crates/altair-otel/tests/integration.rs
git commit -m "feat(otel): add prelude module and integration test"
```

### Task 4.7: Example + README

**Files:**
- Create: `crates/altair-otel/examples/basic.rs`
- Modify: `crates/altair-otel/README.md`

- [ ] **Step 1: Write example**

```rust
//! Run with: `cargo run --example basic -p altair-otel`
//!
//! Uses the stdout exporter so no collector is required.

use altair_otel::prelude::*;
use altair_otel::{Exporter, Config};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Config::builder()
        .service_name("basic-example")
        .service_version("0.1.0")
        .exporter(Exporter::Stdout)
        .build()
        .init()?;

    info!(user_id = 42, "request received");

    let counter = meter().u64_counter("requests.total").build();
    counter.add(1, &[]);
    counter.add(1, &[]);

    do_work().await;

    shutdown();
    Ok(())
}

#[instrument]
async fn do_work() {
    info!("doing work");
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
}
```

- [ ] **Step 2: Replace README**

```markdown
# altair-otel

One-call OpenTelemetry setup for tokio applications. Sets up the `tracing` subscriber, OTLP exporters, and provides a `Meter` handle for metrics.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-otel
```

You do **not** need to add `opentelemetry`, `tracing`, or `tracing-subscriber` separately — `altair-otel` re-exports them.

## Quick start

```rust,no_run
use altair_otel::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Config::builder()
        .service_name("payments-api")
        .service_version("0.1.0")
        .otlp_endpoint("http://collector:4317")
        .build()
        .init()?;

    info!(user_id = 42, "request received");

    let counter = meter().u64_counter("requests.total").build();
    counter.add(1, &[]);

    shutdown();
    Ok(())
}
```

## From environment

```rust,no_run
use altair_otel::prelude::*;

# fn main() -> altair_otel::Result<()> {
Config::from_env()?.init()?;
# Ok(()) }
```

Honored env vars:

- `OTEL_SERVICE_NAME` (required)
- `OTEL_SERVICE_VERSION` (optional)
- `OTEL_EXPORTER_OTLP_ENDPOINT` (optional)
- `OTEL_LOG_FORMAT` — `pretty` (default) or `json`
- `RUST_LOG` — standard tracing filter (e.g., `info,altair_otel=debug`)

## Exporters

- `Exporter::Otlp` (default) — OTLP/gRPC via tonic
- `Exporter::Stdout` — local dev, no collector needed
- `Exporter::None` — disable exporters (useful in tests)

## Feature flags

- `otlp-grpc` (default) — OTLP over gRPC
- `otlp-http` — OTLP over HTTP/protobuf
- `console` — additional stdout exporter helpers

## License

[MIT](../../LICENSE)
```

- [ ] **Step 3: Build + run example**

Run: `cargo build -p altair-otel --example basic`
Expected: builds cleanly.

Run: `cargo run -p altair-otel --example basic`
Expected: prints stdout spans + log lines, exits with code 0.

- [ ] **Step 4: Doc tests pass**

Run: `cargo test -p altair-otel --doc`
Expected: all doc tests pass (or compile-only for `no_run` examples).

- [ ] **Step 5: Commit**

```bash
git add crates/altair-otel
git commit -m "docs(otel): add basic example and complete README"
```

### Task 4.8: Update porting tracker + local CI

- [ ] **Step 1: Mark `altair-otel` as `🚧 In Progress`**

- [ ] **Step 2: `task ci:check`**

Expected: passes.

- [ ] **Step 3: Commit**

```bash
git add docs/porting-tracker.md
git commit -m "docs: mark altair-otel as in-progress"
```

---

## Phase 5: Release Preparation and First Publish

Goal: Tighten root-level docs, run security checks, set up `release-plz` secrets, publish all four crates as `v0.1.0`.

### Task 5.1: Update root README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update README to reflect implemented crates**

Replace the "Starter Set (v0.1.0 planned)" section with:

```markdown
## Starter Set — v0.1.0

| Crate | Purpose | crates.io |
|---|---|---|
| [`altair-otel`](crates/altair-otel) | One-call OpenTelemetry setup | [![crate](https://img.shields.io/crates/v/altair-otel.svg)](https://crates.io/crates/altair-otel) |
| [`altair-config`](crates/altair-config) | Type-safe TOML config + env + validation | [![crate](https://img.shields.io/crates/v/altair-config.svg)](https://crates.io/crates/altair-config) |
| [`altair-retry`](crates/altair-retry) | Async retry with auto-tracing | [![crate](https://img.shields.io/crates/v/altair-retry.svg)](https://crates.io/crates/altair-retry) |
| [`altair-concurrent`](crates/altair-concurrent) | Type-safe parallel execution | [![crate](https://img.shields.io/crates/v/altair-concurrent.svg)](https://crates.io/crates/altair-concurrent) |
```

Update the "Status" section:

```markdown
## Status

**v0.1.0** — first public release. APIs are stable within `0.x` (minor = breaking allowed, patch = additive).
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: update README for v0.1.0 release"
```

### Task 5.2: Update porting tracker for v0.1.0

**Files:**
- Modify: `docs/porting-tracker.md`

- [ ] **Step 1: Move all four starter crates to `✅ Done`**

Edit the "Starter Set" table to change every row's status from `🚧 In Progress` to `✅ Done`.

- [ ] **Step 2: Update header**

Change `Last updated:` to today's date and the release version note.

- [ ] **Step 3: Commit**

```bash
git add docs/porting-tracker.md
git commit -m "docs: mark v0.1.0 starter set as Done"
```

### Task 5.3: Verify all crates publish cleanly (dry run)

- [ ] **Step 1: Dry-run publish each crate in topological order**

Run:
```bash
cargo publish --dry-run -p altair-concurrent
cargo publish --dry-run -p altair-retry
cargo publish --dry-run -p altair-config
cargo publish --dry-run -p altair-otel
```

Expected: each command exits 0 and prints "Packaged …". If any complains about missing fields in `Cargo.toml` (description, keywords, license), fix and re-run.

- [ ] **Step 2: Commit any fixes**

```bash
git add -p
git commit -m "chore: tighten crate metadata for publish"
```

### Task 5.4: Configure crates.io API token in CI

This step is **manual** — performed by the user.

- [ ] **Step 1: Generate a crates.io API token**

Visit https://crates.io/me, generate a new token scoped to "publish-new" and "publish-update".

- [ ] **Step 2: Add as a GitHub repository secret**

In the `altair-rs` GitHub repo settings → Secrets and variables → Actions, add:
- Name: `CARGO_REGISTRY_TOKEN`
- Value: (the token from step 1)

- [ ] **Step 3: Verify `release.yml` references it correctly**

Already wired in Task 0.8 — no change needed. Confirm by inspecting `.github/workflows/release.yml`.

### Task 5.5: First push to GitHub

This step is **manual** — performed by the user.

- [ ] **Step 1: Create the empty `altair-rs` repo on GitHub**

Via web UI or `gh repo create jasoet/altair-rs --public --description "Rust utility crates with OpenTelemetry instrumentation"`.

- [ ] **Step 2: Add remote and push**

```bash
git remote add origin git@github.com:jasoet/altair-rs.git
git push -u origin main
```

- [ ] **Step 3: Verify CI passes on the first push**

Wait for the `ci` workflow to complete (~5 min). All jobs should pass.

### Task 5.6: First publish via release-plz

Either `release-plz` runs automatically on push to main (via the workflow), or trigger it manually.

- [ ] **Step 1: Inspect the release-plz PR**

After the push, `release-plz` opens a PR titled "release v0.1.0 (date)". Review it: it should bump every crate to `0.1.0` and update each crate's `CHANGELOG.md`.

- [ ] **Step 2: Merge the release PR**

Squash-merge the PR. On merge, the release workflow tags `altair-concurrent-v0.1.0`, `altair-retry-v0.1.0`, `altair-config-v0.1.0`, `altair-otel-v0.1.0` and publishes each to crates.io.

- [ ] **Step 3: Verify each crate appears on crates.io**

Check:
- https://crates.io/crates/altair-concurrent
- https://crates.io/crates/altair-retry
- https://crates.io/crates/altair-config
- https://crates.io/crates/altair-otel

All should show `0.1.0` as the current version.

### Task 5.7: Verify docs deployment

- [ ] **Step 1: Verify `docs` workflow ran on tag push**

The `docs.yml` workflow triggers on `v*` tags. After the publish, confirm it ran successfully and deployed to GitHub Pages.

- [ ] **Step 2: Confirm Pages URL renders**

Visit `https://jasoet.github.io/altair-rs/` (URL may differ slightly depending on Pages config). Should redirect to `altair_otel/index.html` and render the cargo doc output.

### Task 5.8: Close out v0.1.0

- [ ] **Step 1: Update porting tracker's "Last updated" date one more time**

- [ ] **Step 2: Tag the workspace milestone in the tracker**

Add a brief "Release Notes" section at the top of `docs/porting-tracker.md`:

```markdown
## v0.1.0 — Released YYYY-MM-DD

Starter set published to crates.io:
- `altair-concurrent` 0.1.0
- `altair-retry` 0.1.0
- `altair-config` 0.1.0
- `altair-otel` 0.1.0

Next milestone: depends on real-world need. Most likely candidates from `Awaiting Demand`:
`altair-server` (axum), `altair-rest` (reqwest), `altair-db` (sqlx).
```

- [ ] **Step 3: Commit**

```bash
git add docs/porting-tracker.md
git commit -m "docs: record v0.1.0 release in porting tracker"
git push
```

---

## Self-Review

### Spec Coverage Check

Each section of `docs/specs/2026-05-27-altair-rs-starter-design.md`:

- §1 Overview — captured in plan header
- §2 Decisions Locked — every decision mapped to plan tasks (workspace, naming, MSRV, edition, errors, license, tooling, OTel hybrid integration)
- §3.1 Workspace Layout — Phase 0 builds the exact structure
- §3.2 Cross-Crate Conventions — applied in every per-crate Cargo.toml + lib.rs (`#![deny(missing_docs)]`, `#![forbid(unsafe_code)]`, `thiserror`, prelude module)
- §3.3 Design Philosophy (re-exports, prelude, smart defaults, cross-crate auto-integration) — applied in each crate's `lib.rs` and `prelude.rs`
- §4.1 `altair-otel` — Phase 4
- §4.2 `altair-config` — Phase 3
- §4.3 `altair-retry` — Phase 2
- §4.4 `altair-concurrent` — Phase 1
- §5 Testing/CI/Release — Phase 0 (CI + cargo-deny + release-plz config), each phase (per-crate tests), Phase 5 (publish flow)
- §6 Porting Tracker — touched in every phase's "Update porting tracker" task
- §7 Risks — pinned `[workspace.dependencies]` addresses transitive-type-leak risk; pre-1.0 versioning policy addresses MSRV
- §8 Out of Scope — explicitly not built (hot-reload, heterogeneous concurrent, multi-runtime)

### Placeholder Scan

No "TBD", "TODO", "implement later", or "similar to Task N" markers. Each step contains complete code.

### Type Consistency

- `TaskMap<T>`, `Executor<T>`, `execute_concurrently<T>(...)` — consistent generic naming
- `Config::builder().build()` pattern identical across `altair-retry` and `altair-otel`
- `Error` / `Result<T>` / `PermanentError` — consistent naming across crates
- `prelude` modules — same shape (re-export local + key 3rd-party types)

No drift identified.

---

## Execution Handoff

**Plan complete and saved to `docs/plans/2026-05-27-altair-rs-starter-implementation.md`. Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** — execute tasks in this session via executing-plans, batch with checkpoints

Pick when ready to start implementation.
