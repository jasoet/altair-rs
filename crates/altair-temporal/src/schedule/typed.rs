//! Typed schedule builder: schedule a Rust `#[workflow]` type with
//! compile-time-checked input.
//!
//! Unlike the string-based [`ScheduleBuilder`](super::ScheduleBuilder),
//! [`TypedScheduleBuilder`] is generic over the workflow type `W`, so the
//! `input` you attach is checked against the workflow's declared input type
//! and serialized by the client's `DataConverter` (custom converters are
//! respected).

// `tracing::instrument` interacts with Rust 2024's tail-expr-drop-order
// rule; the schedule helpers carry no `Drop` side effects, so silence it.
#![allow(tail_expr_drop_order)]

use temporalio_client::schedules::{CreateScheduleOptions, ScheduleAction};
use temporalio_common::HasWorkflowDefinition;
use temporalio_common::data_converters::TemporalSerializable;

use super::{SpecParts, apply_spec_update};
use crate::error::{Error, Result};

/// Entry point for building a schedule that targets a typed Rust workflow.
///
/// ```no_run
/// # use altair_temporal::TypedSchedule;
/// # use altair_temporal::temporalio_common::{WorkflowDefinition, HasWorkflowDefinition};
/// # #[derive(Default)]
/// # struct MyWorkflow;
/// # impl WorkflowDefinition for MyWorkflow {
/// #     type Input = i64;
/// #     type Output = ();
/// #     fn name(&self) -> &str { "MyWorkflow" }
/// # }
/// # impl HasWorkflowDefinition for MyWorkflow { type Run = Self; }
/// # async fn ex(client: &altair_temporal::temporalio_client::Client) -> altair_temporal::Result<()> {
/// TypedSchedule::<MyWorkflow>::builder()
///     .cron("0 9 * * *")
///     .input(42)
///     .task_queue("tq")
///     .workflow_id("wid")
///     .create(client, "sched-id")
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct TypedSchedule<W>(core::marker::PhantomData<W>);

impl<W: HasWorkflowDefinition> TypedSchedule<W> {
    /// Start building a typed schedule for workflow `W`.
    #[must_use]
    pub fn builder() -> TypedScheduleBuilder<W> {
        TypedScheduleBuilder {
            spec: SpecParts::default(),
            task_queue: None,
            workflow_id: None,
            input: None,
        }
    }
}

/// Builder for a schedule targeting the typed workflow `W`.
///
/// Construct via [`TypedSchedule::builder`].
pub struct TypedScheduleBuilder<W: HasWorkflowDefinition> {
    spec: SpecParts,
    task_queue: Option<String>,
    workflow_id: Option<String>,
    input: Option<W::Input>,
}

impl<W: HasWorkflowDefinition> TypedScheduleBuilder<W> {
    /// Add a cron expression. Repeatable.
    #[must_use]
    pub fn cron(mut self, cron: impl Into<String>) -> Self {
        self.spec.cron_strings.push(cron.into());
        self
    }

    /// Add an interval between runs. Repeatable.
    #[must_use]
    pub fn interval(mut self, d: std::time::Duration) -> Self {
        self.spec.intervals.push(d);
        self
    }

    /// Set the IANA timezone cron expressions are interpreted in.
    #[must_use]
    pub fn timezone(mut self, tz: impl Into<String>) -> Self {
        self.spec.timezone = Some(tz.into());
        self
    }

    /// Set a human-readable note (shown in the Temporal UI).
    #[must_use]
    pub fn note(mut self, n: impl Into<String>) -> Self {
        self.spec.note = Some(n.into());
        self
    }

    /// Whether the schedule starts paused (default `false`).
    #[must_use]
    pub fn paused(mut self, p: bool) -> Self {
        self.spec.paused = p;
        self
    }

    /// Attach the workflow input. Checked against `W`'s declared input
    /// type; serialized by the client's `DataConverter` at create time.
    #[must_use]
    pub fn input(mut self, input: W::Input) -> Self {
        self.input = Some(input);
        self
    }

    /// Set the task queue the scheduled workflow runs on.
    #[must_use]
    pub fn task_queue(mut self, tq: impl Into<String>) -> Self {
        self.task_queue = Some(tq.into());
        self
    }

    /// Set the workflow id (the server may append a timestamp).
    #[must_use]
    pub fn workflow_id(mut self, wid: impl Into<String>) -> Self {
        self.workflow_id = Some(wid.into());
        self
    }

    /// Validate that the builder has a trigger, task queue, workflow id,
    /// and input configured.
    fn validate(&self) -> Result<()> {
        if self.task_queue.is_none() || self.workflow_id.is_none() {
            return Err(Error::Configuration(
                "typed schedule requires task_queue(...) and workflow_id(...)".to_string(),
            ));
        }
        if self.input.is_none() {
            return Err(Error::Configuration(
                "typed schedule requires input(...)".to_string(),
            ));
        }
        if !self.spec.has_trigger() {
            return Err(Error::Configuration(
                "schedule requires at least one cron or interval".to_string(),
            ));
        }
        Ok(())
    }
}

impl<W> TypedScheduleBuilder<W>
where
    W: HasWorkflowDefinition + Default,
    W::Input: TemporalSerializable + Send + Sync + 'static,
{
    /// Create the schedule on the server.
    ///
    /// Fails with [`Error::Schedule`] if a schedule with the same id
    /// already exists — use [`create_or_update`](Self::create_or_update)
    /// for the idempotent path.
    #[tracing::instrument(skip_all, fields(schedule_id))]
    pub async fn create(
        self,
        client: &temporalio_client::Client,
        id: impl Into<String>,
    ) -> Result<()> {
        self.validate()?;
        let id = id.into();
        tracing::Span::current().record("schedule_id", id.as_str());
        let opts = self.into_create_options();
        client
            .create_schedule(id, opts)
            .await
            .map(|_handle| ())
            .map_err(|e| Error::schedule(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))
    }

    /// Update spec / paused / note on an existing schedule. Does not
    /// change the workflow type or input.
    #[tracing::instrument(skip_all, fields(schedule_id))]
    pub async fn update(
        self,
        client: &temporalio_client::Client,
        id: impl Into<String>,
    ) -> Result<()> {
        if !self.spec.has_trigger() {
            return Err(Error::Configuration(
                "schedule requires at least one cron or interval".to_string(),
            ));
        }
        let id = id.into();
        tracing::Span::current().record("schedule_id", id.as_str());
        apply_spec_update(client, id, &self.spec).await
    }

    /// Create the schedule, or update spec/paused/note if it already
    /// exists. `W::Input` need not be `Clone` — the input is consumed by
    /// `create`, and the update fallback only touches the (cloned) spec.
    #[tracing::instrument(skip_all, fields(schedule_id))]
    pub async fn create_or_update(
        self,
        client: &temporalio_client::Client,
        id: impl Into<String>,
    ) -> Result<()> {
        let id = id.into();
        tracing::Span::current().record("schedule_id", id.as_str());
        let spec_for_update = self.spec.clone();
        match self.create(client, id.clone()).await {
            Ok(()) => Ok(()),
            Err(Error::Schedule { .. }) => {
                tracing::info!("schedule already exists; updating");
                apply_spec_update(client, id, &spec_for_update).await
            }
            Err(other) => Err(other),
        }
    }

    fn into_create_options(self) -> CreateScheduleOptions {
        let action = ScheduleAction::start_workflow(
            W::default(),
            self.input.expect("validated present"),
            self.task_queue.expect("validated present"),
            self.workflow_id.expect("validated present"),
        );
        let spec = self.spec.to_spec();
        let note = self.spec.note.clone().unwrap_or_default();
        CreateScheduleOptions::builder()
            .action(action)
            .spec(spec)
            .paused(self.spec.paused)
            .note(note)
            .build()
    }
}

#[cfg(test)]
mod tests {
    // Hand-written `WorkflowDefinition::name` impls must return `&str` to
    // match the trait; clippy's `&'static str` suggestion doesn't apply.
    #![allow(clippy::unnecessary_literal_bound)]

    use super::*;
    use temporalio_common::WorkflowDefinition;

    #[derive(Default)]
    struct TestWf;
    impl WorkflowDefinition for TestWf {
        type Input = i64;
        type Output = ();
        fn name(&self) -> &str {
            "TestWf"
        }
    }
    impl HasWorkflowDefinition for TestWf {
        type Run = Self;
    }

    #[test]
    fn rejects_missing_input() {
        let b = TypedSchedule::<TestWf>::builder()
            .cron("0 9 * * *")
            .task_queue("tq")
            .workflow_id("wid");
        assert!(b.validate().is_err());
    }

    #[test]
    fn rejects_missing_trigger() {
        let b = TypedSchedule::<TestWf>::builder()
            .input(7)
            .task_queue("tq")
            .workflow_id("wid");
        assert!(b.validate().is_err());
    }

    #[test]
    fn rejects_missing_task_queue() {
        let b = TypedSchedule::<TestWf>::builder()
            .cron("0 9 * * *")
            .input(7)
            .workflow_id("wid");
        assert!(b.validate().is_err());
    }

    #[test]
    fn rejects_missing_workflow_id() {
        let b = TypedSchedule::<TestWf>::builder()
            .cron("0 9 * * *")
            .input(7)
            .task_queue("tq");
        assert!(b.validate().is_err());
    }

    #[test]
    fn accepts_complete() {
        let b = TypedSchedule::<TestWf>::builder()
            .cron("0 9 * * *")
            .input(7)
            .task_queue("tq")
            .workflow_id("wid");
        assert!(b.validate().is_ok());
    }

    // Compile-only: `create_or_update` must work when `W::Input` is NOT
    // `Clone`. If the implementation cloned the input, this would fail to
    // compile.
    #[derive(serde::Serialize, serde::Deserialize)]
    struct NoCloneInput(i64);

    #[derive(Default)]
    struct NoCloneWf;
    impl WorkflowDefinition for NoCloneWf {
        type Input = NoCloneInput;
        type Output = ();
        fn name(&self) -> &str {
            "NoCloneWf"
        }
    }
    impl HasWorkflowDefinition for NoCloneWf {
        type Run = Self;
    }

    #[allow(dead_code)]
    async fn create_or_update_needs_no_clone_input(client: &temporalio_client::Client) {
        let _ = TypedSchedule::<NoCloneWf>::builder()
            .cron("0 9 * * *")
            .input(NoCloneInput(1))
            .task_queue("tq")
            .workflow_id("wid")
            .create_or_update(client, "id")
            .await;
    }
}
