//! Schedule builder + helpers.
//!
//! Wraps Temporal's `CreateScheduleOptions` / `ScheduleSpec` / `ScheduleAction`
//! with an opinionated builder that owns the public surface.
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

use std::time::Duration;

use temporalio_client::schedules::{
    CreateScheduleOptions, ScheduleAction, ScheduleIntervalSpec, ScheduleSpec,
};

use crate::error::{Error, Result};

/// A schedule ready to be created or updated.
#[derive(Debug, Clone)]
pub struct Schedule {
    pub(crate) cron_strings: Vec<String>,
    pub(crate) intervals: Vec<Duration>,
    pub(crate) timezone: Option<String>,
    pub(crate) note: Option<String>,
    pub(crate) paused: bool,
    pub(crate) workflow_type: Option<String>,
    pub(crate) task_queue: Option<String>,
    pub(crate) workflow_id: Option<String>,
}

impl Schedule {
    /// Start building a schedule.
    #[must_use]
    pub fn builder() -> ScheduleBuilder {
        ScheduleBuilder {
            schedule: Schedule {
                cron_strings: Vec::new(),
                intervals: Vec::new(),
                timezone: None,
                note: None,
                paused: false,
                workflow_type: None,
                task_queue: None,
                workflow_id: None,
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
        self.schedule.cron_strings.push(cron.into());
        self
    }

    /// Add an interval between runs. Repeatable.
    #[must_use]
    pub fn interval(mut self, d: Duration) -> Self {
        self.schedule.intervals.push(d);
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
        self.schedule.timezone = Some(tz.into());
        self
    }

    /// Set a human-readable note (shown in the Temporal UI).
    #[must_use]
    pub fn note(mut self, n: impl Into<String>) -> Self {
        self.schedule.note = Some(n.into());
        self
    }

    /// Whether the schedule starts paused (default `false`).
    #[must_use]
    pub fn paused(mut self, p: bool) -> Self {
        self.schedule.paused = p;
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
        let spec = to_spec(&schedule);
        let paused = schedule.paused;
        let note = schedule.note.clone();
        let handle = client.get_schedule_handle(id);
        handle
            .update(move |u| {
                u.set_spec(spec.clone());
                u.set_paused(paused);
                if let Some(n) = &note {
                    u.set_note(n.clone());
                }
            })
            .await
            .map_err(|e| Error::schedule(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))
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
        .delete()
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

fn to_spec(s: &Schedule) -> ScheduleSpec {
    ScheduleSpec {
        cron_strings: s.cron_strings.clone(),
        intervals: s
            .intervals
            .iter()
            .map(|d| ScheduleIntervalSpec::new(*d, None))
            .collect(),
        timezone_name: s.timezone.clone().unwrap_or_default(),
        ..Default::default()
    }
}

fn to_create_options(s: &Schedule) -> CreateScheduleOptions {
    // Constructed as a variant literal rather than through
    // `ScheduleAction::start_workflow`, which requires a typed
    // `WorkflowDefinition` plus an input value — this builder is
    // string-based and schedules its workflows without input.
    let action = ScheduleAction::StartWorkflow {
        workflow_type: s.workflow_type.clone().expect("validated above"),
        task_queue: s.task_queue.clone().expect("validated above"),
        workflow_id: s.workflow_id.clone().expect("validated above"),
        input: None,
    };
    let spec = to_spec(s);
    let note = s.note.clone().unwrap_or_default();
    CreateScheduleOptions::builder()
        .action(action)
        .spec(spec)
        .paused(s.paused)
        .note(note)
        .build()
}

fn validate_schedule(s: &Schedule) -> Result<()> {
    if s.workflow_type.is_none() || s.task_queue.is_none() || s.workflow_id.is_none() {
        return Err(Error::Configuration(
            "schedule requires start_workflow(workflow_type, task_queue, workflow_id)".to_string(),
        ));
    }
    if s.cron_strings.is_empty() && s.intervals.is_empty() {
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
        assert_eq!(s.cron_strings.len(), 2);
    }

    #[test]
    fn interval_appends() {
        let s = Schedule::builder()
            .interval(Duration::from_mins(1))
            .interval(Duration::from_mins(2))
            .start_workflow("W", "tq", "wid")
            .build();
        assert_eq!(s.intervals.len(), 2);
    }

    #[test]
    fn note_and_paused() {
        let s = Schedule::builder()
            .note("daily archive")
            .paused(true)
            .cron("0 0 * * *")
            .start_workflow("W", "tq", "wid")
            .build();
        assert_eq!(s.note.as_deref(), Some("daily archive"));
        assert!(s.paused);
    }

    #[test]
    fn cron_and_interval_coexist() {
        let s = Schedule::builder()
            .cron("0 9 * * MON")
            .interval(Duration::from_mins(5))
            .start_workflow("W", "tq", "wid")
            .build();
        assert_eq!(s.cron_strings, vec!["0 9 * * MON"]);
        assert_eq!(s.intervals, vec![Duration::from_mins(5)]);
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
        let spec = to_spec(&s);
        assert_eq!(spec.cron_strings, vec!["0 9 * * *"]);
        assert_eq!(spec.intervals.len(), 1);
        assert_eq!(spec.intervals[0].every, Duration::from_mins(1));
    }
}
