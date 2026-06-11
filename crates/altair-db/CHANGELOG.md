# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-06-11


### Bug Fixes

- **(db)** Make slim feature builds compile and lint clean ([#67](https://github.com/jasoet/altair-rs/pull/67))

### Refactor

- **(otel)** Return Result from ConfigBuilder::build ([#66](https://github.com/jasoet/altair-rs/pull/66))
- **(db)** Feature-gate database drivers ([#64](https://github.com/jasoet/altair-rs/pull/64))

## [0.1.6] - 2026-05-31


### Tests

- Cross-crate integration tests — db macOS, config, retry, otel collector fixture ([#35](https://github.com/jasoet/altair-rs/pull/35))

## [0.1.3] - 2026-05-29


### Features

- **(db)** Add altair-db crate (sea-orm + sqlx) ([#26](https://github.com/jasoet/altair-rs/pull/26))
