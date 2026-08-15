#![forbid(unsafe_code)]

use keith_agent_types::{GoalId, SessionId, UtcTimestamp};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceState {
    Available,
    Thinking,
    UsingTools,
    WaitingChild,
    WaitingExternal,
    PausedForUser,
    Scheduled,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeFact {
    ActionStarted {
        at: UtcTimestamp,
    },
    ToolStarted {
        at: UtcTimestamp,
        tool: String,
    },
    ToolFinished {
        at: UtcTimestamp,
        tool: String,
    },
    ChildStarted {
        at: UtcTimestamp,
        child: String,
    },
    ChildFinished {
        at: UtcTimestamp,
        child: String,
    },
    WaitingExternal {
        at: UtcTimestamp,
        next_wake: Option<UtcTimestamp>,
    },
    PausedForUser {
        at: UtcTimestamp,
    },
    Scheduled {
        at: UtcTimestamp,
        next_wake: UtcTimestamp,
    },
    Recovered {
        at: UtcTimestamp,
    },
    Completed {
        at: UtcTimestamp,
    },
    Failed {
        at: UtcTimestamp,
        safe_error: String,
    },
    BecameIdle {
        at: UtcTimestamp,
    },
}

impl RuntimeFact {
    const fn occurred_at(&self) -> UtcTimestamp {
        match self {
            Self::ActionStarted { at }
            | Self::ToolStarted { at, .. }
            | Self::ToolFinished { at, .. }
            | Self::ChildStarted { at, .. }
            | Self::ChildFinished { at, .. }
            | Self::WaitingExternal { at, .. }
            | Self::PausedForUser { at }
            | Self::Scheduled { at, .. }
            | Self::Recovered { at }
            | Self::Completed { at }
            | Self::Failed { at, .. }
            | Self::BecameIdle { at } => *at,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresenceProjection {
    pub session_id: SessionId,
    pub goal_id: Option<GoalId>,
    pub state: PresenceState,
    pub updated_at: UtcTimestamp,
    pub next_wake: Option<UtcTimestamp>,
    pub safe_error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressNotification {
    pub session_id: SessionId,
    pub goal_id: Option<GoalId>,
    pub state: PresenceState,
    pub occurred_at: UtcTimestamp,
    pub next_wake: Option<UtcTimestamp>,
    pub safe_error: Option<String>,
    pub summary: String,
}

type PresenceTransition = (
    PresenceState,
    Option<UtcTimestamp>,
    Option<String>,
    String,
    bool,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FabricatedClaim {
    CursorMovement,
    Typing,
    ToolUseWithoutEvent,
    BackgroundThought,
    ProgressPercentage,
    CompletionWithoutEvent,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum PresenceError {
    #[error("progress notification interval must be non-negative")]
    InvalidInterval,
    #[error("runtime event timestamp regressed")]
    TimeRegression,
    #[error("safe failure detail must be non-empty and bounded")]
    UnsafeFailureDetail,
    #[error("presentation claim is not supported by runtime evidence")]
    Fabricated,
}

pub struct PresenceProjector {
    projection: PresenceProjection,
    minimum_notification_interval_ms: i64,
    last_notification_at: Option<UtcTimestamp>,
}

impl PresenceProjector {
    /// Creates an available projection with no implied activity.
    ///
    /// # Errors
    ///
    /// Returns an error for a negative notification interval.
    pub fn new(
        session_id: SessionId,
        goal_id: Option<GoalId>,
        now: UtcTimestamp,
        minimum_notification_interval_ms: i64,
    ) -> Result<Self, PresenceError> {
        if minimum_notification_interval_ms < 0 {
            return Err(PresenceError::InvalidInterval);
        }
        Ok(Self {
            projection: PresenceProjection {
                session_id,
                goal_id,
                state: PresenceState::Available,
                updated_at: now,
                next_wake: None,
                safe_error: None,
            },
            minimum_notification_interval_ms,
            last_notification_at: None,
        })
    }

    /// Restores only an authoritative persisted projection after restart.
    ///
    /// # Errors
    ///
    /// Returns an error for a negative notification interval.
    pub fn restore(
        projection: PresenceProjection,
        minimum_notification_interval_ms: i64,
    ) -> Result<Self, PresenceError> {
        if minimum_notification_interval_ms < 0 {
            return Err(PresenceError::InvalidInterval);
        }
        Ok(Self {
            projection,
            minimum_notification_interval_ms,
            last_notification_at: None,
        })
    }

    pub const fn projection(&self) -> &PresenceProjection {
        &self.projection
    }

    /// Applies one real runtime fact and optionally emits a meaningful rate-limited transition.
    ///
    /// Terminal failures and completions are never rate-suppressed.
    ///
    /// # Errors
    ///
    /// Returns an error for regressing time or unsafe failure detail.
    pub fn apply(
        &mut self,
        fact: &RuntimeFact,
    ) -> Result<Option<ProgressNotification>, PresenceError> {
        let at = fact.occurred_at();
        if at < self.projection.updated_at {
            return Err(PresenceError::TimeRegression);
        }
        let previous = self.projection.state;
        let (state, next_wake, safe_error, summary, always_emit) = transition(fact)?;
        self.projection.state = state;
        self.projection.updated_at = at;
        self.projection.next_wake = next_wake;
        self.projection.safe_error.clone_from(&safe_error);
        let meaningful = previous != state || matches!(fact, RuntimeFact::Recovered { .. });
        if !meaningful {
            return Ok(None);
        }
        let rate_limited = self.last_notification_at.is_some_and(|last| {
            at.unix_millis().saturating_sub(last.unix_millis())
                < self.minimum_notification_interval_ms
        });
        if rate_limited && !always_emit {
            return Ok(None);
        }
        self.last_notification_at = Some(at);
        Ok(Some(ProgressNotification {
            session_id: self.projection.session_id.clone(),
            goal_id: self.projection.goal_id.clone(),
            state,
            occurred_at: at,
            next_wake,
            safe_error,
            summary,
        }))
    }
}

/// Explicitly rejects presentation states that have no runtime evidence representation.
///
/// # Errors
///
/// Always returns [`PresenceError::Fabricated`].
pub const fn reject_fabricated_claim(_claim: FabricatedClaim) -> Result<(), PresenceError> {
    Err(PresenceError::Fabricated)
}

fn transition(fact: &RuntimeFact) -> Result<PresenceTransition, PresenceError> {
    let transition = match fact {
        RuntimeFact::ActionStarted { .. } => (
            PresenceState::Thinking,
            None,
            None,
            "Work started".to_owned(),
            false,
        ),
        RuntimeFact::ToolStarted { tool, .. } => (
            PresenceState::UsingTools,
            None,
            None,
            bounded_summary(&format!("Using {tool}")),
            false,
        ),
        RuntimeFact::ToolFinished { tool, .. } => (
            PresenceState::Thinking,
            None,
            None,
            bounded_summary(&format!("Finished {tool}")),
            false,
        ),
        RuntimeFact::ChildStarted { child, .. } => (
            PresenceState::WaitingChild,
            None,
            None,
            bounded_summary(&format!("Waiting for {child}")),
            false,
        ),
        RuntimeFact::ChildFinished { child, .. } => (
            PresenceState::Thinking,
            None,
            None,
            bounded_summary(&format!("Received result from {child}")),
            false,
        ),
        RuntimeFact::WaitingExternal { next_wake, .. } => (
            PresenceState::WaitingExternal,
            *next_wake,
            None,
            "Waiting for an external event".to_owned(),
            false,
        ),
        RuntimeFact::PausedForUser { .. } => (
            PresenceState::PausedForUser,
            None,
            None,
            "Waiting for your input".to_owned(),
            false,
        ),
        RuntimeFact::Scheduled { next_wake, .. } => (
            PresenceState::Scheduled,
            Some(*next_wake),
            None,
            "Scheduled".to_owned(),
            false,
        ),
        RuntimeFact::Recovered { .. } => (
            PresenceState::Thinking,
            None,
            None,
            "Recovered and resumed".to_owned(),
            true,
        ),
        RuntimeFact::Completed { .. } => (
            PresenceState::Completed,
            None,
            None,
            "Completed".to_owned(),
            true,
        ),
        RuntimeFact::Failed { safe_error, .. } => {
            let safe_error = safe_error.trim();
            if safe_error.is_empty() || safe_error.len() > 512 {
                return Err(PresenceError::UnsafeFailureDetail);
            }
            (
                PresenceState::Failed,
                None,
                Some(safe_error.to_owned()),
                "Failed".to_owned(),
                true,
            )
        }
        RuntimeFact::BecameIdle { .. } => (
            PresenceState::Available,
            None,
            None,
            "Available".to_owned(),
            false,
        ),
    };
    Ok(transition)
}

fn bounded_summary(summary: &str) -> String {
    const LIMIT: usize = 256;
    if summary.len() <= LIMIT {
        return summary.to_owned();
    }
    let mut boundary = LIMIT;
    while !summary.is_char_boundary(boundary) {
        boundary -= 1;
    }
    summary[..boundary].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_history_drives_tool_child_wait_recovery_failure_and_restart_projection() {
        let session = SessionId::new();
        let goal = GoalId::new();
        let mut projector = PresenceProjector::new(
            session.clone(),
            Some(goal.clone()),
            UtcTimestamp::UNIX_EPOCH,
            10,
        )
        .expect("projector");
        assert_eq!(projector.projection().state, PresenceState::Available);
        let history = [
            RuntimeFact::ActionStarted {
                at: UtcTimestamp::from_unix_millis(10),
            },
            RuntimeFact::ToolStarted {
                at: UtcTimestamp::from_unix_millis(11),
                tool: "search".to_owned(),
            },
            RuntimeFact::ToolFinished {
                at: UtcTimestamp::from_unix_millis(12),
                tool: "search".to_owned(),
            },
            RuntimeFact::ChildStarted {
                at: UtcTimestamp::from_unix_millis(20),
                child: "research".to_owned(),
            },
            RuntimeFact::ChildFinished {
                at: UtcTimestamp::from_unix_millis(30),
                child: "research".to_owned(),
            },
            RuntimeFact::WaitingExternal {
                at: UtcTimestamp::from_unix_millis(40),
                next_wake: Some(UtcTimestamp::from_unix_millis(100)),
            },
            RuntimeFact::Recovered {
                at: UtcTimestamp::from_unix_millis(50),
            },
        ];
        let mut emitted = Vec::new();
        for fact in &history {
            if let Some(notification) = projector.apply(fact).expect("real transition") {
                emitted.push(notification);
            }
        }
        assert!(emitted.len() < history.len());
        assert_eq!(projector.projection().state, PresenceState::Thinking);
        let restored = PresenceProjector::restore(projector.projection().clone(), 10)
            .expect("restart projection");
        assert_eq!(restored.projection(), projector.projection());

        let mut restored = restored;
        let failure = restored
            .apply(&RuntimeFact::Failed {
                at: UtcTimestamp::from_unix_millis(51),
                safe_error: "provider unavailable".to_owned(),
            })
            .expect("failure")
            .expect("terminal notification bypasses rate limit");
        assert_eq!(failure.session_id, session);
        assert_eq!(failure.goal_id, Some(goal));
        assert_eq!(failure.state, PresenceState::Failed);
        assert_eq!(failure.safe_error.as_deref(), Some("provider unavailable"));
    }

    #[test]
    fn scheduled_completion_and_zero_activity_are_truthful() {
        let mut projector =
            PresenceProjector::new(SessionId::new(), None, UtcTimestamp::UNIX_EPOCH, 1_000)
                .expect("projector");
        assert_eq!(projector.projection().state, PresenceState::Available);
        let scheduled = projector
            .apply(&RuntimeFact::Scheduled {
                at: UtcTimestamp::from_unix_millis(1),
                next_wake: UtcTimestamp::from_unix_millis(2_000),
            })
            .expect("scheduled")
            .expect("notification");
        assert_eq!(
            scheduled.next_wake,
            Some(UtcTimestamp::from_unix_millis(2_000))
        );
        let completed = projector
            .apply(&RuntimeFact::Completed {
                at: UtcTimestamp::from_unix_millis(2),
            })
            .expect("completed")
            .expect("terminal not rate limited");
        assert_eq!(completed.state, PresenceState::Completed);
    }

    #[test]
    fn fabricated_presentation_claims_are_rejected() {
        for claim in [
            FabricatedClaim::CursorMovement,
            FabricatedClaim::Typing,
            FabricatedClaim::ToolUseWithoutEvent,
            FabricatedClaim::BackgroundThought,
            FabricatedClaim::ProgressPercentage,
            FabricatedClaim::CompletionWithoutEvent,
        ] {
            assert_eq!(
                reject_fabricated_claim(claim),
                Err(PresenceError::Fabricated)
            );
        }
    }
}
