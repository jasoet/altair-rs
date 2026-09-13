# altair-temporal: native input for scheduled workflows

**Status:** Approved (2026-09-13)
**Scope:** `altair-temporal` `schedule` module
**Type:** Feature (additive; no breaking changes)

## Problem

`altair-temporal`'s `ScheduleBuilder` schedules workflows by type name and
always sets the SDK action's `input` to `None`. Before the temporalio 1.0
upgrade, attaching input to a scheduled workflow was not possible, so
projects packed payloads into the workflow ID (`workflow_id::encode`).

SDK 1.0's `ScheduleAction::StartWorkflow` carries input directly. Its only
public constructor is the typed `ScheduleAction::start_workflow<W>(workflow,
input, task_queue, workflow_id)` where `W: HasWorkflowDefinition`
(`ScheduleWorkflowInput`'s raw constructor is private). Two workflow types
satisfy `HasWorkflowDefinition`:

- a Rust `#[workflow]` struct (macro-generated impl), input = its declared
  input type; serialized by the client's `DataConverter`.
- `UntypedWorkflow::new(name)` — schedule by string name, input =
  `RawValue` (pre-encoded `Vec<Payload>`).

## Goals

- Let schedules carry native input on **both** a typed path (Rust-native
  workflows) and the existing string-named path.
- Keep the current no-input `start_workflow(...)` API working unchanged.
- Do not require consumers to touch raw payloads for the common serde case.

## Non-goals (v1)

- `update()` does not change a schedule's input/action — it keeps its
  current spec/paused/note-only behavior. Input is set at create time.
- Multi-argument workflow inputs (single-arg only, matching the SDK's
  common case).
- Custom `DataConverter` support on the string path (it assumes the
  default `json/plain` encoding).
- Removing `workflow_id::encode/decode` — it remains a convenience utility.

## API

### String-named path — add `.input()` to `ScheduleBuilder`

```rust
impl ScheduleBuilder {
    /// Attach input to the scheduled workflow, serialized as a
    /// `json/plain` payload (Temporal's default converter). Fallible:
    /// serialization can fail.
    pub fn input(self, value: &impl serde::Serialize) -> Result<Self>;
}
```

With input set, `create` builds
`ScheduleAction::start_workflow(UntypedWorkflow::new(workflow_type),
RawValue::new(payloads), task_queue, workflow_id)`. Without it, the current
`input: None` path is used.

### Typed path — `TypedSchedule<W>` / `TypedScheduleBuilder<W>`

```rust
TypedSchedule::<MyWorkflow>::builder()          // W: HasWorkflowDefinition + Default
    .cron("0 9 * * *").interval(..).timezone("Asia/Jakarta").note(..).paused(..)
    .input(MyInput { .. })                       // W::Input, by value, lazy (not fallible)
    .task_queue("tq").workflow_id("wid")
    .create(&client, "sched-id").await?;
```

Uses `ScheduleAction::start_workflow(W::default(), input, task_queue,
workflow_id)`; the client's `DataConverter` serializes lazily, so custom
converters are respected. `W: Default` lets the type-only `::builder()`
entry construct the instance needed for `WorkflowDefinition::name(&self)`.

## Internal design

- **`SpecParts`** (private): `cron_strings`, `intervals`, `timezone`,
  `note`, `paused` + `to_spec() -> ScheduleSpec`. Shared by both builders
  so the cron/interval/timezone/note/paused setters live in one place.
- **`Schedule`** gains `input_payloads: Option<Vec<Payload>>`
  (`Clone`+`Debug` preserved).
- **`encode_input_payloads(&impl Serialize) -> Result<Vec<Payload>>`**:
  `serde_json::to_vec` into a `Payload { metadata: {encoding: json/plain},
  data }`, wrapped as one payload.
- **`apply_spec_update(client, id, &SpecParts)`**: the `handle.update`
  (spec/paused/note) call, shared by both builders' `update`.
- **`create_or_update`** clones the (cheap, `Clone`) `SpecParts` before
  `create` consumes the input, so **`W::Input` need not be `Clone`**.

## Code layout

Split `schedule.rs` into a `schedule/` module:

- `schedule/mod.rs` — `SpecParts`, `Schedule`, `ScheduleBuilder` (with
  `.input()`), `encode_input_payloads`, `apply_spec_update`, `delete` /
  `delete_if_exists`, validation.
- `schedule/typed.rs` — `TypedSchedule<W>`, `TypedScheduleBuilder<W>`.

`lib.rs` and `prelude` re-export `TypedSchedule` / `TypedScheduleBuilder`
alongside the existing `Schedule` / `ScheduleBuilder`.

## Testing

- Unit (no container): `encode_input_payloads` produces the expected
  `json/plain` payload; `.input()` populates `input_payloads`; `SpecParts`
  spec conversion; typed builder validation (missing input / trigger /
  task_queue / workflow_id); a compile-only check that `create_or_update`
  works with a non-`Clone` `W::Input`.
- Integration (behind `integration-tests`, real container): schedule a
  typed workflow with input and a string-named workflow with input, assert
  each execution receives it.

## Reachability

All SDK items are already re-exported via `altair-temporal`:
`temporalio_common::{HasWorkflowDefinition, WorkflowDefinition,
UntypedWorkflow}`, `temporalio_common::data_converters::RawValue`,
`temporalio_common::protos::temporal::api::common::v1::Payload`,
`temporalio_client::schedules::ScheduleAction`.
