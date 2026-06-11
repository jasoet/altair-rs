# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.3.0] - 2026-06-11


### Bug Fixes

- **(wf)** Production readiness — Error::Activity downcast + tracing + helpers ([#61](https://github.com/jasoet/altair-rs/pull/61))
- **(temporal)** Production readiness — tuner, graceful shutdown, identity, retry validation ([#60](https://github.com/jasoet/altair-rs/pull/60))
- **(wf)** Crate-wide review fixes for altair-wf ([#57](https://github.com/jasoet/altair-rs/pull/57))
- **(wf)** Phase 3 R2 review fixes for datasync module ([#56](https://github.com/jasoet/altair-rs/pull/56))
- **(wf)** Phase 3 R1 review fixes for datasync module ([#55](https://github.com/jasoet/altair-rs/pull/55))
- **(wf)** Phase 2 R2 review fixes for function module ([#53](https://github.com/jasoet/altair-rs/pull/53))
- **(wf)** Phase 2 R1 review fixes for function module ([#52](https://github.com/jasoet/altair-rs/pull/52))
- **(wf)** Round 2 deep-review fixes — timing bug, facade, helper, integration coverage ([#50](https://github.com/jasoet/altair-rs/pull/50))
- **(wf)** Round 1 deep-review fixes — determinism, deep DAGs, facade, docs ([#49](https://github.com/jasoet/altair-rs/pull/49))

### Documentation

- **(examples)** Scheduled examples + 16 wf variants + serde polish ([#63](https://github.com/jasoet/altair-rs/pull/63))
- **(wf)** Add 9 runnable examples covering every wf capability ([#58](https://github.com/jasoet/altair-rs/pull/58))

### Features

- **(wf)** Parallel concurrency cap + function-activity heartbeat ticker ([#62](https://github.com/jasoet/altair-rs/pull/62))
- **(wf)** Track failed-task positions in pipeline / parallel / loop outputs ([#59](https://github.com/jasoet/altair-rs/pull/59))
- **(wf)** Phase 3 — datasync core + chunk submodule ([#54](https://github.com/jasoet/altair-rs/pull/54))
- **(wf)** Phase 2 — function module (registry + named-handler activity) ([#51](https://github.com/jasoet/altair-rs/pull/51))

### Refactor

- **(wf)** Scope missing_docs allow to activity module ([#65](https://github.com/jasoet/altair-rs/pull/65))
