use std::collections::BTreeMap;
use std::fs;

use keith_agent_types::{
    ActionId, BindingTargetKind, BindingTargetSlot, BindingTaskScope, CURRENT_SCHEMA_VERSION,
    EntityId, Generation, GoalId, ObjectBindingKey, ObjectBindingReference, ProfileId, Revision,
    RootTreeId, SessionId, ToolCallId, TurnId, UtcTimestamp, WorkerId, WorkspaceId,
};
use keith_session_store::{
    CompactionOutput, CompactionPolicy, CompactionTrigger, ContentBlock, FrozenBindingAdmission,
    FrozenObjectBindingUse, MessageRole, NewSession, RequiredObjectBinding, SessionEntry,
    SessionEntryPayload, SessionKind, SessionStore, SessionStoreError, SessionWriter,
    StoredMessage, TurnTerminalStatus, WriterIdentity, binding_arguments_digest,
};
use serde_json::json;

const AT: UtcTimestamp = UtcTimestamp::UNIX_EPOCH;

fn identity() -> WriterIdentity {
    WriterIdentity {
        worker_id: WorkerId::new(),
        owner_instance: EntityId::new(),
        generation: Generation::ZERO,
        acquired_at: AT,
    }
}

fn message(role: MessageRole, text: &str) -> StoredMessage {
    StoredMessage {
        role,
        content: vec![ContentBlock::Text { text: text.into() }],
        provider_metadata: BTreeMap::new(),
    }
}

fn append(writer: &mut SessionWriter, payload: SessionEntryPayload) -> SessionEntry {
    writer
        .append(writer.manifest().active_leaf.clone(), AT, payload)
        .unwrap()
}

struct Fixture {
    root: tempfile::TempDir,
    store: SessionStore,
    writer: SessionWriter,
    scope: BindingTaskScope,
    turn: TurnId,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let store = SessionStore::open(root.path()).unwrap();
        let manifest = store
            .create(NewSession {
                kind: SessionKind::Root,
                session_id: SessionId::new(),
                root_tree_id: RootTreeId::new(),
                parent_session_id: None,
                profile_id: ProfileId::new(),
                workspace_id: WorkspaceId::new(),
                created_at: AT,
                label: None,
                profile_snapshot: None,
            })
            .unwrap();
        let mut writer = store
            .acquire_writer(&manifest.session_id, identity())
            .unwrap();
        let scope = BindingTaskScope {
            profile_id: manifest.profile_id,
            workspace_id: manifest.workspace_id,
            session_id: manifest.session_id,
            action_id: ActionId::new(),
            goal_id: None,
        };
        let turn = TurnId::new();
        let ingress = append(
            &mut writer,
            SessionEntryPayload::UserMessage {
                message: message(MessageRole::User, "Read both current project files."),
            },
        );
        writer
            .accept_turn(AT, scope.action_id.clone(), turn.clone(), ingress.id)
            .unwrap();
        Self {
            root,
            store,
            writer,
            scope,
            turn,
        }
    }

    fn intent(&mut self, required: Vec<RequiredObjectBinding>) -> FrozenBindingAdmission {
        self.writer
            .require_object_bindings(self.scope.clone(), required.clone(), AT)
            .unwrap();
        append(
            &mut self.writer,
            SessionEntryPayload::AssistantActivity {
                turn_id: self.turn.clone(),
                message: message(MessageRole::Assistant, "Read the bound file."),
            },
        );
        let call_id = ToolCallId::new();
        let arguments = json!({"path":"src/current.txt"});
        append(
            &mut self.writer,
            SessionEntryPayload::ToolCall {
                call_id: call_id.clone(),
                name: "read".into(),
                arguments: arguments.clone(),
            },
        );
        let selected = required
            .first()
            .map(|requirement| FrozenObjectBindingUse {
                reference: ObjectBindingReference {
                    key: requirement.key.clone(),
                    binding_id: EntityId::new(),
                    revision: Revision::new(1),
                    evidence_id: EntityId::new(),
                    evidence_digest: "a".repeat(64),
                    value_digest: "b".repeat(64),
                },
                target: requirement.target.clone(),
            })
            .into_iter()
            .collect();
        FrozenBindingAdmission {
            version: CURRENT_SCHEMA_VERSION,
            scope: self.scope.clone(),
            turn_id: self.turn.clone(),
            call_id,
            tool_name: "read".into(),
            arguments_digest: binding_arguments_digest(&arguments).unwrap(),
            required,
            bindings: selected,
            admitted_at: AT,
        }
    }

    fn history(&self) -> std::path::PathBuf {
        self.root
            .path()
            .join("sessions")
            .join(self.scope.session_id.to_string())
            .join("history.jsonl")
    }
}

fn requirement() -> RequiredObjectBinding {
    RequiredObjectBinding {
        key: ObjectBindingKey {
            entity_id: EntityId::new(),
            property: "document.path".into(),
        },
        target: BindingTargetSlot {
            kind: BindingTargetKind::WorkspacePath,
            tool_name: "read".into(),
            argument_name: "path".into(),
        },
    }
}

fn compact(writer: &mut SessionWriter) {
    let request = writer
        .request_compaction(
            200_000,
            CompactionPolicy {
                target_tokens: 1,
                ..CompactionPolicy::default()
            },
            None,
            CompactionTrigger::Manual,
        )
        .unwrap()
        .unwrap();
    let request = writer.begin_compaction(request, AT).unwrap();
    writer
        .commit_compaction(
            &request,
            CompactionOutput {
                request_id: request.id.clone(),
                session_summary: "Prior project work.".into(),
                raw_provider_output: "Prior project work.".into(),
                provider: None,
                model: None,
                max_output_tokens: 100,
                input_tokens: 100,
                output_tokens: 5,
                cached_input_tokens: 0,
                memory_candidates: Vec::new(),
                daily_entry: None,
                open_commitments: Vec::new(),
                unresolved_items: Vec::new(),
            },
            AT,
        )
        .unwrap();
}

#[test]
fn required_union_survives_real_compaction_restart_and_goal_continuation() {
    let mut fixture = Fixture::new();
    fixture.scope.goal_id = Some(GoalId::new());
    let first = requirement();
    let second = requirement();
    fixture
        .writer
        .require_object_bindings(fixture.scope.clone(), vec![first.clone()], AT)
        .unwrap();
    fixture
        .writer
        .require_object_bindings(fixture.scope.clone(), Vec::new(), AT)
        .unwrap();
    fixture
        .writer
        .append_finalized_turn(
            AT,
            &fixture.turn,
            message(MessageRole::Assistant, "The first action stopped."),
            TurnTerminalStatus::Failed,
            false,
            true,
            Some(fixture.scope.action_id.clone()),
            Vec::new(),
            None,
        )
        .unwrap();
    append(
        &mut fixture.writer,
        SessionEntryPayload::UserMessage {
            message: message(MessageRole::User, "Continue the same goal."),
        },
    );
    compact(&mut fixture.writer);
    assert_eq!(fixture.writer.manifest().compaction_generation, 1);
    let mut continuation = fixture.scope.clone();
    continuation.action_id = ActionId::new();
    fixture
        .writer
        .require_object_bindings(continuation.clone(), vec![second.clone()], AT)
        .unwrap();
    // A branch switch must not erase an already committed dependency either.
    let root = fixture.writer.active_ancestry().unwrap()[0].id.clone();
    fixture
        .writer
        .append(
            Some(root),
            AT,
            SessionEntryPayload::UserMessage {
                message: message(MessageRole::User, "Continue from another branch."),
            },
        )
        .unwrap();
    let history = fixture.history();
    let before = fs::read(&history).unwrap();
    drop(fixture.writer);
    let store = SessionStore::open(fixture.root.path()).unwrap();
    let writer = store
        .acquire_writer(&fixture.scope.session_id, identity())
        .unwrap();
    let required = writer.required_object_bindings(&continuation).unwrap();
    assert_eq!(required.len(), 2);
    assert!(required.contains(&first) && required.contains(&second));
    assert_eq!(fs::read(&history).unwrap(), before);
    continuation.goal_id = Some(GoalId::new());
    assert!(
        writer
            .required_object_bindings(&continuation)
            .unwrap()
            .is_empty()
    );
    continuation.profile_id = ProfileId::new();
    assert!(matches!(
        writer.required_object_bindings(&continuation),
        Err(SessionStoreError::InvalidBindingAdmission(_))
    ));
}

#[test]
fn requirement_validation_is_bounded_and_failure_does_not_append() {
    let mut fixture = Fixture::new();
    let first = requirement();
    let mut malformed = first.clone();
    malformed.key.property = "../invented".into();
    for required in [
        vec![first.clone(), first],
        vec![malformed],
        (0..129).map(|_| requirement()).collect(),
    ] {
        let before = fs::read(fixture.history()).unwrap();
        assert!(
            fixture
                .writer
                .require_object_bindings(fixture.scope.clone(), required, AT)
                .is_err()
        );
        assert_eq!(fs::read(fixture.history()).unwrap(), before);
    }
}

#[test]
fn frozen_selection_keeps_full_union_and_one_selected_key_per_adapter_slot() {
    let mut fixture = Fixture::new();
    let admission = fixture.intent(vec![requirement(), requirement()]);
    let entry = fixture
        .writer
        .append_binding_admission(admission.clone())
        .unwrap();
    let before = fs::read(fixture.history()).unwrap();
    let mut retry = admission.clone();
    retry.admitted_at = UtcTimestamp::from_unix_millis(2);
    assert_eq!(
        fixture.writer.append_binding_admission(retry).unwrap(),
        entry
    );
    assert_eq!(fs::read(fixture.history()).unwrap(), before);
    drop(fixture.writer);
    let writer = fixture
        .store
        .acquire_writer(&fixture.scope.session_id, identity())
        .unwrap();
    let index = fixture.store.load_index(&fixture.scope.session_id).unwrap();
    assert_eq!(
        index.get(&entry.id).unwrap().payload,
        SessionEntryPayload::BindingAdmission { admission }
    );
    assert_eq!(
        writer
            .required_object_bindings(&fixture.scope)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn admissions_reject_omissions_conflicts_and_mismatched_durable_intent() {
    let mut fixture = Fixture::new();
    let admission = fixture.intent(vec![requirement(), requirement()]);
    let mut invalid = Vec::new();
    let mut copy = admission.clone();
    copy.required.pop();
    invalid.push(copy);
    let mut copy = admission.clone();
    copy.bindings.clear();
    invalid.push(copy);
    let mut copy = admission.clone();
    copy.bindings.push(copy.bindings[0].clone());
    invalid.push(copy);
    let mut copy = admission.clone();
    copy.bindings[0].reference.key = requirement().key;
    invalid.push(copy);
    let mut copy = admission.clone();
    copy.scope.action_id = ActionId::new();
    invalid.push(copy);
    let mut copy = admission.clone();
    copy.scope.session_id = SessionId::new();
    invalid.push(copy);
    let mut copy = admission.clone();
    copy.turn_id = TurnId::new();
    invalid.push(copy);
    let mut copy = admission.clone();
    copy.call_id = ToolCallId::new();
    invalid.push(copy);
    let mut copy = admission.clone();
    copy.arguments_digest = "c".repeat(64);
    invalid.push(copy);
    let mut copy = admission.clone();
    copy.tool_name = "write".into();
    invalid.push(copy);
    let mut copy = admission.clone();
    copy.version.minor += 1;
    invalid.push(copy);
    let mut copy = admission.clone();
    copy.bindings[0].reference.revision = Revision::ZERO;
    invalid.push(copy);
    let before = fs::read(fixture.history()).unwrap();
    for candidate in invalid {
        assert!(matches!(
            fixture.writer.append_binding_admission(candidate),
            Err(SessionStoreError::InvalidBindingAdmission(_))
        ));
        assert_eq!(fs::read(fixture.history()).unwrap(), before);
    }
    fixture
        .writer
        .append_binding_admission(admission.clone())
        .unwrap();
    let mut conflicting = admission;
    conflicting.bindings[0].reference.revision = Revision::new(2);
    assert!(
        fixture
            .writer
            .append_binding_admission(conflicting)
            .is_err()
    );
}

#[test]
fn selecting_one_key_cannot_omit_a_different_required_argument_slot() {
    let mut fixture = Fixture::new();
    let mut second = requirement();
    second.target.argument_name = "another_path".into();
    let admission = fixture.intent(vec![requirement(), second]);
    assert!(fixture.writer.append_binding_admission(admission).is_err());
}

#[test]
fn stale_branches_duplicate_calls_and_already_completed_intents_are_refused() {
    for case in 0..4 {
        let mut fixture = Fixture::new();
        let admission = fixture.intent(vec![requirement()]);
        match case {
            0 => {
                let root = fixture.writer.active_ancestry().unwrap()[0].id.clone();
                fixture
                    .writer
                    .append(
                        Some(root),
                        AT,
                        SessionEntryPayload::UserMessage {
                            message: message(MessageRole::User, "A different branch."),
                        },
                    )
                    .unwrap();
            }
            1 => {
                append(
                    &mut fixture.writer,
                    SessionEntryPayload::ToolCall {
                        call_id: admission.call_id.clone(),
                        name: "read".into(),
                        arguments: json!({"path":"src/current.txt"}),
                    },
                );
            }
            2 => {
                append(
                    &mut fixture.writer,
                    SessionEntryPayload::ToolResult {
                        call_id: admission.call_id.clone(),
                        content: Vec::new(),
                        is_error: false,
                        failure: None,
                    },
                );
            }
            3 => {
                fixture
                    .writer
                    .append_finalized_turn(
                        AT,
                        &fixture.turn,
                        message(MessageRole::Assistant, "The turn stopped."),
                        TurnTerminalStatus::Failed,
                        false,
                        true,
                        Some(fixture.scope.action_id.clone()),
                        Vec::new(),
                        None,
                    )
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let before = fs::read(fixture.history()).unwrap();
        assert!(fixture.writer.append_binding_admission(admission).is_err());
        assert_eq!(fs::read(fixture.history()).unwrap(), before);
    }
}

#[test]
fn admission_storage_failure_is_reported_without_a_frozen_record() {
    let mut fixture = Fixture::new();
    let admission = fixture.intent(vec![requirement()]);
    let history = fixture.history();
    let saved = history.with_extension("saved");
    fs::rename(&history, &saved).unwrap();
    fs::create_dir(&history).unwrap();
    assert!(fixture.writer.append_binding_admission(admission).is_err());
    fs::remove_dir(&history).unwrap();
    fs::rename(saved, history).unwrap();
    assert!(
        !fixture
            .writer
            .active_ancestry()
            .unwrap()
            .iter()
            .any(|entry| matches!(entry.payload, SessionEntryPayload::BindingAdmission { .. }))
    );
}
