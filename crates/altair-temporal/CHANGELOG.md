# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-06-11


### Bug Fixes

- **(temporal)** Production readiness — tuner, graceful shutdown, identity, retry validation ([#60](https://github.com/jasoet/altair-rs/pull/60))

### Documentation

- **(examples)** Scheduled examples + 16 wf variants + serde polish ([#63](https://github.com/jasoet/altair-rs/pull/63))

### Refactor

- **(otel)** Return Result from ConfigBuilder::build ([#66](https://github.com/jasoet/altair-rs/pull/66))

## [0.1.5] - 2026-05-31


### Bug Fixes

- **(temporal)** Retry validation, signal handler, workflow_id guard, prelude macros ([#32](https://github.com/jasoet/altair-rs/pull/32))

### Features

- **(temporal)** TemporalContainer fixture + 10 production-grade integration tests ([#34](https://github.com/jasoet/altair-rs/pull/34))

## [0.1.3]


### Features

- **(temporal)** Add altair-temporal crate (temporalio-sdk facade) ([#28](https://github.com/jasoet/altair-rs/pull/28))
