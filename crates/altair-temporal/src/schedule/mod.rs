//! Schedule builder + helpers.
//!
//! Wraps Temporal's `CreateScheduleOptions` / `ScheduleSpec` / `ScheduleAction`
//! with an opinionated builder that owns the public surface.
//!
//! Two entry points share the cron/interval/timezone/note/paused surface via
//! [`SpecParts`]:
//!
//! - [`ScheduleBuilder`] — schedule by workflow-type **name**; optional
//!   [`input`](ScheduleBuilder::input) attaches a serde-serialized payload.
//! - [`TypedScheduleBuilder`] — schedule a Rust `#[workflow]` type with
//!   compile-time-checked input.
//!
//! Calling both [`ScheduleBuilder::cron`] and [`ScheduleBuilder::interval`]
//! is allowed — Temporal accepts and runs the union. Most callers will
//! pick one. Callers wanting strictly one ensure they only call that one.

// `tracing::instrument` attaches a `Drop`-guarded span to the function
// body; under Rust 2024's tail-expr-drop-order rule this changes the
// drop order of locals borrowed by the tail expression. The schedule
// helpers don't carry side effects in `Drop`, so the change is
// observationally neutral — silence the lint here.
#![allow(tail_expr_drop_order)]

mod typed;

pub use typed::{TypedSchedule, TypedScheduleBuilder};

use std::time::Duration;

use serde::Serialize;
use temporalio_client::RpcOptions;
use temporalio_client::schedules::{
    CreateScheduleOptions, DeleteScheduleOptions, ScheduleAction, ScheduleIntervalSpec,
    ScheduleSpec,
};
use temporalio_common::UntypedWorkflow;
use temporalio_common::data_converters::RawValue;
use temporalio_common::protos::temporal::api::common::v1::Payload;

use crate::error::{Error, Result};

/// The cron/interval/timezone/note/paused fields shared by the string-based
/// [`ScheduleBuilder`] and the typed [`TypedScheduleBuilder`].
///
/// Keeping these in one place means the spec-building logic and the
/// setter surface live once, not once per builder.
#[derive(Debug, Clone, Default)]
pub(crate) struct SpecParts {
    pub(crate) cron_strings: Vec<String>,
    pub(crate) intervals: Vec<Duration>,
    pub(crate) timezone: Option<String>,
    pub(crate) note: Option<String>,
    pub(crate) paused: bool,
}

impl SpecParts {
    /// Build the SDK [`ScheduleSpec`] from the cron/interval/timezone fields.
    pub(crate) fn to_spec(&self) -> ScheduleSpec {
        // `ScheduleSpec` is `#[non_exhaustive]` in SDK 1.0 and must be built
        // through its bon builder; every field not set here keeps its default.
        ScheduleSpec::builder()
            .cron_strings(self.cron_strings.clone())
            .intervals(
                self.intervals
                    .iter()
                    .map(|d| ScheduleIntervalSpec::new(*d, None))
                    .collect::<Vec<_>>(),
            )
            .timezone_name(self.timezone.clone().unwrap_or_default())
            .build()
    }

    /// True when at least one cron or interval trigger is configured.
    pub(crate) fn has_trigger(&self) -> bool {
        !self.cron_strings.is_empty() || !self.intervals.is_empty()
    }
}

/// Apply a spec/paused/note update to an existing schedule. Shared by
/// [`ScheduleBuilder::update`] and [`TypedScheduleBuilder::update`].
///
/// Note: this intentionally does **not** change the schedule's action
/// (workflow type / input) — only the spec, paused flag, and note.
async fn apply_spec_update(
    client: &temporalio_client::Client,
    id: String,
    spec_parts: &SpecParts,
) -> Result<()> {
    let spec = spec_parts.to_spec();
    let paused = spec_parts.paused;
    let note = spec_parts.note.clone();
    let handle = client.get_schedule_handle(id);
    handle
        .update(
            move |u| {
                u.set_spec(spec.clone());
                u.set_paused(paused);
                if let Some(n) = &note {
                    u.set_note(n.clone());
                }
            },
            RpcOptions::default(),
        )
        .await
        .map_err(|e| Error::schedule(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))
}

/// Encode a serde value as a single Temporal [`Payload`] using the default
/// `json/plain` encoding (matching Temporal's default `DataConverter`).
///
/// Used by [`ScheduleBuilder::input`] to attach input to a workflow
/// scheduled by type name.
pub(crate) fn encode_input_payloads<T: Serialize + ?Sized>(value: &T) -> Result<Vec<Payload>> {
    let data = serde_json::to_vec(value)
        .map_err(|e| Error::Configuration(format!("schedule input serialise failed: {e}")))?;
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("encoding".to_string(), b"json/plain".to_vec());
    Ok(vec![Payload {
        metadata,
        data,
        ..Default::default()
    }])
}

/// A schedule ready to be created or updated.
#[derive(Debug, Clone)]
pub struct Schedule {
    pub(crate) spec: SpecParts,
    pub(crate) workflow_type: Option<String>,
    pub(crate) task_queue: Option<String>,
    pub(crate) workflow_id: Option<String>,
    pub(crate) input_payloads: Option<Vec<Payload>>,
}

impl Schedule {
    /// Start building a schedule.
    #[must_use]
    pub fn builder() -> ScheduleBuilder {
        ScheduleBuilder {
            schedule: Schedule {
                spec: SpecParts::default(),
                workflow_type: None,
                task_queue: None,
                workflow_id: None,
                input_payloads: None,
            },
        }
    }
}

/// Builder for [`Schedule`].
#[derive(Debug, Clone)]
pub struct ScheduleBuilder {
    schedule: Schedule,
}

impl ScheduleBuilder {
    /// Add a cron expression. Repeatable.
    #[must_use]
    pub fn cron(mut self, cron: impl Into<String>) -> Self {
        self.schedule.spec.cron_strings.push(cron.into());
        self
    }

    /// Add an interval between runs. Repeatable.
    #[must_use]
    pub fn interval(mut self, d: Duration) -> Self {
        self.schedule.spec.intervals.push(d);
        self
    }

    /// Set the IANA timezone the schedule's cron expressions are
    /// interpreted in (e.g. `"US/Eastern"`, `"Asia/Jakarta"`).
    ///
    /// Defaults to UTC if unset — which means a `"0 0 * * *"` cron
    /// fires at midnight UTC, **not** the operator's local midnight.
    /// Set this explicitly for any human-facing schedule.
    #[must_use]
    pub fn timezone(mut self, tz: impl Into<String>) -> Self {
        self.schedule.spec.timezone = Some(tz.into());
        self
    }

    /// Set a human-readable note (shown in the Temporal UI).
    #[must_use]
    pub fn note(mut self, n: impl Into<String>) -> Self {
        self.schedule.spec.note = Some(n.into());
        self
    }

    /// Whether the schedule starts paused (default `false`).
    #[must_use]
    pub fn paused(mut self, p: bool) -> Self {
        self.schedule.spec.paused = p;
        self
    }

    /// Configure the `StartWorkflow` action.
    #[must_use]
    pub fn start_workflow(
        mut self,
        workflow_type: impl Into<String>,
        task_queue: impl Into<String>,
        workflow_id: impl Into<String>,
    ) -> Self {
        self.schedule.workflow_type = Some(workflow_type.into());
        self.schedule.task_queue = Some(task_queue.into());
        self.schedule.workflow_id = Some(workflow_id.into());
        self
    }

    /// Attach input to the scheduled workflow, serialized as a `json/plain`
    /// payload (Temporal's default `DataConverter` encoding).
    ///
    /// Use this for workflows scheduled by type name. For a Rust workflow
    /// whose input type you want compile-time-checked (and serialized by
    /// the client's configured converter), use [`TypedSchedule`] instead.
    ///
    /// # Errors
    ///
    /// [`Error::Configuration`] if `value` cannot be serialized to JSON.
    pub fn input(mut self, value: &impl Serialize) -> Result<Self> {
        self.schedule.input_payloads = Some(encode_input_payloads(value)?);
        Ok(self)
    }

    /// Finalise into a [`Schedule`] without making any RPC.
    #[must_use]
    pub fn build(self) -> Schedule {
        self.schedule
    }

    /// Create the schedule on the server.
    ///
    /// Fails with [`Error::Schedule`] if a schedule with the same id
    /// already exists — use [`ScheduleBuilder::create_or_update`] for
    /// the idempotent path.
    #[tracing::instrument(skip_all, fields(schedule_id))]
    pub async fn create(
        self,
        client: &temporalio_client::Client,
        id: impl Into<String>,
    ) -> Result<()> {
        let id = id.into();
        tracing::Span::current().record("schedule_id", id.as_str());
        let schedule = self.build();
        validate_schedule(&schedule)?;
        let opts = to_create_options(&schedule);
        client
            .create_schedule(id, opts)
            .await
            .map(|_handle| ())
            .map_err(|e| Error::schedule(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))
    }

    /// Update an existing schedule on the server.
    ///
    /// Replaces spec / paused / note on the existing schedule.
    #[tracing::instrument(skip_all, fields(schedule_id))]
    pub async fn update(
        self,
        client: &temporalio_client::Client,
        id: impl Into<String>,
    ) -> Result<()> {
        let id = id.into();
        tracing::Span::current().record("schedule_id", id.as_str());
        let schedule = self.build();
        validate_schedule(&schedule)?;
        apply_spec_update(client, id, &schedule.spec).await
    }

    /// Create the schedule, or update the existing one if it already
    /// exists. The idempotent path for deploy / redeploy flows.
    #[tracing::instrument(skip_all, fields(schedule_id))]
    pub async fn create_or_update(
        self,
        client: &temporalio_client::Client,
        id: impl Into<String>,
    ) -> Result<()> {
        let id = id.into();
        tracing::Span::current().record("schedule_id", id.as_str());
        // Clone once so we can fall back to update().
        let cloned = self.clone();
        match self.create(client, id.clone()).await {
            Ok(()) => Ok(()),
            Err(Error::Schedule { .. }) => {
                tracing::info!("schedule already exists; updating");
                cloned.update(client, id).await
            }
            Err(other) => Err(other),
        }
    }
}

/// Delete a schedule by id.
///
/// Fails with [`Error::Schedule`] if the schedule does not exist —
/// use [`delete_if_exists`] for the idempotent path.
#[tracing::instrument(skip(client))]
pub async fn delete(client: &temporalio_client::Client, id: &str) -> Result<()> {
    let handle = client.get_schedule_handle(id);
    handle
        .delete(DeleteScheduleOptions::default())
        .await
        .map_err(|e| Error::schedule(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))
}

/// Delete a schedule by id, treating "not found" as success.
///
/// Use this when tearing down a deployment whose schedule may or may
/// not have been provisioned.
#[tracing::instrument(skip(client))]
pub async fn delete_if_exists(client: &temporalio_client::Client, id: &str) -> Result<()> {
    match delete(client, id).await {
        Ok(()) => Ok(()),
        Err(Error::Schedule { .. }) => {
            tracing::debug!("schedule did not exist; treating delete as no-op");
            Ok(())
        }
        Err(other) => Err(other),
    }
}

fn to_create_options(s: &Schedule) -> CreateScheduleOptions {
    let workflow_type = s.workflow_type.clone().expect("validated above");
    let task_queue = s.task_queue.clone().expect("validated above");
    let workflow_id = s.workflow_id.clone().expect("validated above");
    let action = match &s.input_payloads {
        // Input present: schedule by name via `UntypedWorkflow`, carrying the
        // pre-encoded payloads. This is the only public SDK path that attaches
        // input without a typed `WorkflowDefinition`.
        Some(payloads) => ScheduleAction::start_workflow(
            UntypedWorkflow::new(workflow_type),
            RawValue::new(payloads.clone()),
            task_queue,
            workflow_id,
        ),
        // No input: the plain string-based action.
        None => ScheduleAction::StartWorkflow {
            workflow_type,
            task_queue,
            workflow_id,
            input: None,
        },
    };
    let spec = s.spec.to_spec();
    let note = s.spec.note.clone().unwrap_or_default();
    CreateScheduleOptions::builder()
        .action(action)
        .spec(spec)
        .paused(s.spec.paused)
        .note(note)
        .build()
}

fn validate_schedule(s: &Schedule) -> Result<()> {
    if s.workflow_type.is_none() || s.task_queue.is_none() || s.workflow_id.is_none() {
        return Err(Error::Configuration(
            "schedule requires start_workflow(workflow_type, task_queue, workflow_id)".to_string(),
        ));
    }
    if !s.spec.has_trigger() {
        return Err(Error::Configuration(
            "schedule requires at least one cron or interval".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cron_repeatable() {
        let s = Schedule::builder()
            .cron("0 9 * * *")
            .cron("0 18 * * *")
            .start_workflow("MyWorkflow", "tq", "wid")
            .build();
        assert_eq!(s.spec.cron_strings.len(), 2);
    }

    #[test]
    fn interval_appends() {
        let s = Schedule::builder()
            .interval(Duration::from_mins(1))
            .interval(Duration::from_mins(2))
            .start_workflow("W", "tq", "wid")
            .build();
        assert_eq!(s.spec.intervals.len(), 2);
    }

    #[test]
    fn note_and_paused() {
        let s = Schedule::builder()
            .note("daily archive")
            .paused(true)
            .cron("0 0 * * *")
            .start_workflow("W", "tq", "wid")
            .build();
        assert_eq!(s.spec.note.as_deref(), Some("daily archive"));
        assert!(s.spec.paused);
    }

    #[test]
    fn cron_and_interval_coexist() {
        let s = Schedule::builder()
            .cron("0 9 * * MON")
            .interval(Duration::from_mins(5))
            .start_workflow("W", "tq", "wid")
            .build();
        assert_eq!(s.spec.cron_strings, vec!["0 9 * * MON"]);
        assert_eq!(s.spec.intervals, vec![Duration::from_mins(5)]);
    }

    #[test]
    fn validate_rejects_missing_action() {
        let s = Schedule::builder().cron("0 0 * * *").build();
        let err = validate_schedule(&s).unwrap_err();
        assert!(matches!(err, Error::Configuration(_)));
    }

    #[test]
    fn validate_rejects_no_trigger() {
        let s = Schedule::builder().start_workflow("W", "tq", "wid").build();
        let err = validate_schedule(&s).unwrap_err();
        assert!(matches!(err, Error::Configuration(_)));
    }

    #[test]
    fn to_spec_includes_cron_and_intervals() {
        let s = Schedule::builder()
            .cron("0 9 * * *")
            .interval(Duration::from_mins(1))
            .start_workflow("W", "tq", "wid")
            .build();
        let spec = s.spec.to_spec();
        assert_eq!(spec.cron_strings, vec!["0 9 * * *"]);
        assert_eq!(spec.intervals.len(), 1);
        assert_eq!(spec.intervals[0].every, Duration::from_mins(1));
    }

    #[test]
    fn encode_input_payloads_uses_json_plain() {
        let payloads = encode_input_payloads(&42_i32).unwrap();
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].data, b"42");
        assert_eq!(
            payloads[0].metadata.get("encoding").map(Vec::as_slice),
            Some(b"json/plain".as_slice()),
        );
    }

    #[test]
    fn input_sets_payloads_on_schedule() {
        let s = Schedule::builder()
            .cron("0 9 * * *")
            .start_workflow("W", "tq", "wid")
            .input(&serde_json::json!({"k": "v"}))
            .unwrap()
            .build();
        assert_eq!(s.input_payloads.as_ref().map(Vec::len), Some(1));
    }

    #[test]
    fn no_input_leaves_payloads_none() {
        let s = Schedule::builder()
            .cron("0 9 * * *")
            .start_workflow("W", "tq", "wid")
            .build();
        assert!(s.input_payloads.is_none());
    }
}
