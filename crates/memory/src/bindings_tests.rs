use super::*;
use crate as memory_api;
#[path = "../tests/support/committed_crowd.rs"]
mod committed_crowd;
use std::fs;
use std::path::Path;

use crate::{MemoryPolicy, MemoryWriteSource, ObservatoryMutation};
use keith_agent_types::{ActionId, Generation, RootTreeId, SessionId, WorkerId};
use keith_session_store::{
    ContentBlock, MessageRole, NewSession, SessionEntryPayload, SessionKind, SessionStore,
    StoredMessage, WriterIdentity,
};
use keith_workspace::{PersonalWorkspace, PersonalWorkspaceLimits};
use tempfile::{TempDir, tempdir};

const NOW: UtcTimestamp = UtcTimestamp::from_unix_millis(1000);
const URL: &str = "https://cobalt.example/api";
const NEXT: &str = "https://current.example/api";

struct Fixture {
    root: TempDir,
    store: SessionStore,
    scope: BindingTaskScope,
    memory: MemoryService,
}

impl Fixture {
    fn new() -> Self {
        let root = tempdir().unwrap();
        let store = SessionStore::open(root.path().join("sessions")).unwrap();
        let scope = BindingTaskScope {
            profile_id: ProfileId::new(),
            workspace_id: WorkspaceId::new(),
            session_id: SessionId::new(),
            action_id: ActionId::new(),
            goal_id: None,
        };
        store
            .create(NewSession {
                kind: SessionKind::Root,
                session_id: scope.session_id.clone(),
                root_tree_id: RootTreeId::new(),
                parent_session_id: None,
                profile_id: scope.profile_id.clone(),
                workspace_id: scope.workspace_id.clone(),
                created_at: NOW,
                label: None,
                profile_snapshot: None,
            })
            .unwrap();
        let memory = open(root.path(), &scope.profile_id);
        Self {
            root,
            store,
            scope,
            memory,
        }
    }
    fn source(&self, text: &str) -> EvidenceRecord {
        let receipt = {
            let mut writer = self
                .store
                .acquire_writer(&self.scope.session_id, writer_id())
                .unwrap();
            writer
                .append_committed_source(writer.manifest().active_leaf.clone(), NOW, user(text))
                .unwrap()
        };
        self.memory.ingest_committed_entry(&receipt, NOW).unwrap();
        self.memory
            .observatory
            .evidence_snapshot()
            .unwrap()
            .into_values()
            .find(|record| {
                record.source_identity
                    == format!(
                        "session:{}:entry:{}",
                        self.scope.session_id,
                        receipt.entry().id
                    )
            })
            .unwrap()
    }
    fn create(&self, source: &EvidenceRecord, value: &str) -> BindingWriteReceipt {
        self.memory
            .memory_create_binding(&self.scope, create_request(source), draft(value), NOW)
            .unwrap()
    }
    fn lookup(&self, reference: &ObjectBindingReference) -> BindingResolution {
        self.memory
            .lookup_binding(&self.scope, &reference.key, &BindingQuery::default(), NOW)
            .unwrap()
    }
    fn reopen(&mut self) {
        self.memory = open(self.root.path(), &self.scope.profile_id);
    }
    fn path(&self) -> std::path::PathBuf {
        self.root.path().join(".keith/memory-vault.jsonl")
    }
}
fn open(root: &Path, profile: &ProfileId) -> MemoryService {
    MemoryService::open(
        PersonalWorkspace::open(root, PersonalWorkspaceLimits::default(), NOW).unwrap(),
        profile,
        MemoryPolicy::default(),
    )
    .unwrap()
}
fn writer_id() -> WriterIdentity {
    WriterIdentity {
        worker_id: WorkerId::new(),
        owner_instance: EntityId::new(),
        generation: Generation::new(1),
        acquired_at: NOW,
    }
}
fn user(text: &str) -> SessionEntryPayload {
    SessionEntryPayload::UserMessage {
        message: StoredMessage {
            role: MessageRole::User,
            content: vec![ContentBlock::Text { text: text.into() }],
            provider_metadata: BTreeMap::new(),
        },
    }
}
fn citation(source: &EvidenceRecord) -> MemoryWriteSource {
    MemoryWriteSource {
        evidence_id: Some(source.id.clone()),
        source_entry_id: source.source_entries[0].clone(),
        evidence_quote: source.text.clone(),
    }
}
fn create_request(source: &EvidenceRecord) -> MemoryCreateRequest {
    MemoryCreateRequest {
        source: citation(source),
        text: "The model associates a service with its endpoint.".into(),
        kind: AgentMemoryKind::ProjectContext,
        facets: vec![],
        sensitivity: Sensitivity::Personal,
    }
}
fn draft(value: &str) -> BindingDraft {
    BindingDraft {
        entity: BindingEntityTarget::NewAlias {
            alias: "Cobalt".into(),
        },
        property: "api.endpoint".into(),
        target_kind: BindingTargetKind::HttpUrl,
        value_quote: value.into(),
        value_span: None,
        effective: None,
    }
}
fn corrected(source: &EvidenceRecord, prior: &BindingWriteReceipt) -> MemoryCorrectRequest {
    MemoryCorrectRequest {
        evidence_id: prior.evidence.id.clone(),
        source: citation(source),
        replacement: "The model associates the corrected endpoint.".into(),
        facets: vec![],
        sensitivity: None,
    }
}
fn correction(value: &str) -> BindingCorrectionDraft {
    BindingCorrectionDraft {
        value_quote: value.into(),
        value_span: None,
        effective: None,
    }
}
fn resolved(result: BindingResolution) -> ResolvedBinding {
    match result {
        BindingResolution::Resolved { binding } => binding,
        other => panic!("expected resolved; {other:?}"),
    }
}
fn reason(result: &BindingResolution) -> BindingResolutionReason {
    match result {
        BindingResolution::Missing { reason, .. }
        | BindingResolution::Stale { reason, .. }
        | BindingResolution::Conflicting { reason, .. } => *reason,
        BindingResolution::Resolved { .. } => panic!("unexpected resolution"),
    }
}
fn policy() -> BindingUsePolicy {
    BindingUsePolicy {
        target_kind: BindingTargetKind::HttpUrl,
        max_sensitivity: Sensitivity::Personal,
        freshness: BindingFreshness::default(),
        allow_inferred_association: true,
        allowed_source_authorities: vec![EvidenceAuthority::UserAsserted],
    }
}

#[test]
fn binding_commits_owner_and_original_quote_as_one_event_without_authority_promotion() {
    let fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let before = fs::read(fixture.path()).unwrap();
    let receipt = fixture.create(&source, URL);
    let binding = resolved(fixture.lookup(&receipt.binding));
    assert_eq!(binding.reference.evidence_id, source.id);
    assert_eq!(binding.owner_memory_id, receipt.evidence.id);
    assert_ne!(binding.owner_memory_id, binding.reference.evidence_id);
    assert_eq!(
        receipt.evidence.authority,
        EvidenceAuthority::DerivedInference
    );
    assert_eq!(binding.source_authority, EvidenceAuthority::UserAsserted);
    assert_eq!(
        binding.association_origin,
        BindingAssociationOrigin::Inferred
    );
    assert_eq!(binding.value, URL);
    let after = fs::read(fixture.path()).unwrap();
    assert!(after.starts_with(&before));
    let events: Vec<serde_json::Value> = after[before.len()..]
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["mutation"]["mutation"], "binding_associated");
    assert_eq!(
        events[0]["mutation"]["binding"]["memory"]["id"],
        serde_json::to_value(&receipt.evidence.id).unwrap()
    );
    assert!(!format!("{binding:?}").contains(URL));
    assert_eq!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &receipt.binding, URL, &policy(), NOW)
            .unwrap(),
        binding
    );
    assert!(
        fixture
            .memory
            .validate_binding_use(
                &fixture.scope,
                &receipt.binding,
                "https://guessed.example",
                &policy(),
                NOW
            )
            .is_err()
    );
    let mut restricted = policy();
    restricted.allow_inferred_association = false;
    assert!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &receipt.binding, URL, &restricted, NOW)
            .is_err()
    );
    restricted = policy();
    restricted.allowed_source_authorities = vec![EvidenceAuthority::ToolObserved];
    assert!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &receipt.binding, URL, &restricted, NOW)
            .is_err()
    );
    restricted = policy();
    restricted.target_kind = BindingTargetKind::WorkspacePath;
    assert!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &receipt.binding, URL, &restricted, NOW)
            .is_err()
    );
}

#[test]
fn bound_correction_rejects_source_ids_without_mutation_and_accepts_current_owner() {
    let mut fixture = Fixture::new();
    let original = fixture.source(&format!("Cobalt uses {URL}."));
    let prior = fixture.create(&original, URL);
    let replacement = fixture.source(&format!("Cobalt now uses {NEXT}."));
    let before = fs::read(fixture.path()).unwrap();
    for wrong_owner in [
        original.source_entries[0].as_entity_id().clone(),
        prior.binding.evidence_id.clone(),
        replacement.source_entries[0].as_entity_id().clone(),
        replacement.id.clone(),
    ] {
        let mut request = corrected(&replacement, &prior);
        request.evidence_id = wrong_owner;
        assert!(matches!(
            fixture.memory.memory_correct_binding(
                &fixture.scope,
                request,
                &prior.binding,
                correction(NEXT),
                NOW,
            ),
            Err(MemoryError::Binding(BindingError::OwnerMismatch))
        ));
        assert_eq!(fs::read(fixture.path()).unwrap(), before);
    }
    let mut stale = prior.binding.clone();
    stale.revision = stale.revision.checked_next().unwrap();
    assert!(matches!(
        fixture.memory.memory_correct_binding(
            &fixture.scope,
            corrected(&replacement, &prior),
            &stale,
            correction(NEXT),
            NOW,
        ),
        Err(MemoryError::Binding(BindingError::Unresolved(
            BindingResolutionReason::Changed
        )))
    ));
    assert_eq!(fs::read(fixture.path()).unwrap(), before);
    fixture.reopen();
    assert_eq!(
        resolved(fixture.lookup(&prior.binding)).reference,
        prior.binding
    );
    let current = fixture
        .memory
        .memory_correct_binding(
            &fixture.scope,
            corrected(&replacement, &prior),
            &prior.binding,
            correction(NEXT),
            NOW,
        )
        .unwrap();
    assert_eq!(current.binding.evidence_id, replacement.id);
    assert_eq!(current.evidence.supersedes, Some(prior.evidence.id.clone()));
    fixture.reopen();
    assert_eq!(
        resolved(fixture.lookup(&prior.binding)).reference,
        current.binding
    );
    assert_eq!(
        fixture.memory.observatory.evidence_snapshot().unwrap()[&prior.evidence.id].superseded_by,
        Some(current.evidence.id)
    );
}

#[test]
fn correction_retains_effective_and_recorded_history_but_invalidates_old_use_after_restart() {
    let mut fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let mut initial = draft(URL);
    initial.effective = Some(EvidenceEffectiveInterval {
        from: Some(UtcTimestamp::UNIX_EPOCH),
        until: None,
    });
    let prior = fixture
        .memory
        .memory_create_binding(&fixture.scope, create_request(&source), initial, NOW)
        .unwrap();
    let old_revision = fixture.memory.observatory.revision().unwrap();
    let source = fixture.source(&format!("Cobalt now uses {NEXT}."));
    let mut change = correction(NEXT);
    change.effective = Some(EvidenceEffectiveInterval {
        from: Some(UtcTimestamp::from_unix_millis(500)),
        until: None,
    });
    let current = fixture
        .memory
        .memory_correct_binding(
            &fixture.scope,
            corrected(&source, &prior),
            &prior.binding,
            change,
            NOW,
        )
        .unwrap();
    fixture.reopen();
    assert_eq!(
        resolved(fixture.lookup(&prior.binding)).reference,
        current.binding
    );
    let history = BindingQuery {
        effective_at: Some(UtcTimestamp::from_unix_millis(200)),
        ..BindingQuery::default()
    };
    assert_eq!(
        resolved(
            fixture
                .memory
                .lookup_binding(&fixture.scope, &prior.binding.key, &history, NOW)
                .unwrap()
        )
        .reference,
        prior.binding
    );
    let history = BindingQuery {
        recorded_as_of: Some(old_revision),
        ..BindingQuery::default()
    };
    assert_eq!(
        resolved(
            fixture
                .memory
                .lookup_binding(&fixture.scope, &prior.binding.key, &history, NOW)
                .unwrap()
        )
        .reference,
        prior.binding
    );
    assert!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &prior.binding, URL, &policy(), NOW)
            .is_err()
    );
    assert_eq!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &current.binding, NEXT, &policy(), NOW)
            .unwrap()
            .reference,
        current.binding
    );
    let snapshot = fixture.memory.observatory.evidence_snapshot().unwrap();
    assert_eq!(
        snapshot[&prior.evidence.id].superseded_by,
        Some(current.evidence.id.clone())
    );
    assert_eq!(
        snapshot[&current.evidence.id].supersedes,
        Some(prior.evidence.id.clone())
    );
    assert!(
        fixture
            .memory
            .memory_correct_binding(
                &fixture.scope,
                corrected(&source, &prior),
                &prior.binding,
                correction(NEXT),
                NOW
            )
            .is_err()
    );
}

#[test]
fn ordinary_corrections_of_either_source_or_owner_leave_explicit_unbound_gap() {
    for correct_source in [false, true] {
        let mut fixture = Fixture::new();
        let source = fixture.source(&format!("Cobalt uses {URL}."));
        let binding = fixture.create(&source, URL);
        let new_source = fixture.source("The old association was wrong.");
        let target = if correct_source {
            source.id
        } else {
            binding.evidence.id.clone()
        };
        fixture
            .memory
            .memory_correct(
                MemoryCorrectRequest {
                    evidence_id: target,
                    source: citation(&new_source),
                    replacement: new_source.text.clone(),
                    facets: vec![],
                    sensitivity: None,
                },
                NOW,
            )
            .unwrap();
        fixture.reopen();
        assert_eq!(
            reason(&fixture.lookup(&binding.binding)),
            BindingResolutionReason::UnboundCorrection
        );
        assert!(
            fixture
                .memory
                .validate_binding_use(&fixture.scope, &binding.binding, URL, &policy(), NOW)
                .is_err()
        );
    }
}

#[test]
fn deleting_either_owner_or_source_also_revokes_historical_reads() {
    for delete_source in [false, true] {
        let mut fixture = Fixture::new();
        let source = fixture.source(&format!("Cobalt uses {URL}."));
        let binding = fixture.create(&source, URL);
        let revision = fixture.memory.observatory.revision().unwrap();
        let target = if delete_source {
            source.id
        } else {
            binding.evidence.id.clone()
        };
        fixture
            .memory
            .observatory
            .apply(
                vec![ObservatoryMutation::Delete {
                    evidence_id: target,
                    source_entries: vec![],
                    source_digests: vec![],
                }],
                NOW,
            )
            .unwrap();
        fixture.reopen();
        let expected = if delete_source {
            BindingResolutionReason::DeletedSource
        } else {
            BindingResolutionReason::DeletedOwner
        };
        assert_eq!(reason(&fixture.lookup(&binding.binding)), expected);
        let query = BindingQuery {
            recorded_as_of: Some(revision),
            ..BindingQuery::default()
        };
        assert_eq!(
            reason(
                &fixture
                    .memory
                    .lookup_binding(&fixture.scope, &binding.binding.key, &query, NOW)
                    .unwrap()
            ),
            expected
        );
        assert!(
            fixture
                .memory
                .binding_alias_candidates(&fixture.scope, "Use Cobalt", Sensitivity::Secret, 10)
                .unwrap()
                .candidates
                .is_empty()
        );
    }
}

#[test]
fn current_sensitivity_profile_and_workspace_boundaries_survive_history_and_aliases() {
    let fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let binding = fixture.create(&source, URL);
    let revision = fixture.memory.observatory.revision().unwrap();
    fixture
        .memory
        .observatory
        .apply(
            vec![ObservatoryMutation::ChangeSensitivity {
                evidence_id: source.id,
                sensitivity: Sensitivity::Secret,
            }],
            NOW,
        )
        .unwrap();
    let query = BindingQuery {
        recorded_as_of: Some(revision),
        ..BindingQuery::default()
    };
    assert_eq!(
        reason(
            &fixture
                .memory
                .lookup_binding(&fixture.scope, &binding.binding.key, &query, NOW)
                .unwrap()
        ),
        BindingResolutionReason::SensitivityPolicy
    );
    assert!(
        fixture
            .memory
            .binding_alias_candidates(&fixture.scope, "Cobalt", Sensitivity::Personal, 10)
            .unwrap()
            .candidates
            .is_empty()
    );
    let mut query = query;
    query.max_sensitivity = Sensitivity::Secret;
    assert_eq!(
        resolved(
            fixture
                .memory
                .lookup_binding(&fixture.scope, &binding.binding.key, &query, NOW)
                .unwrap()
        )
        .value,
        URL
    );
    let mut scope = fixture.scope.clone();
    scope.profile_id = ProfileId::new();
    assert!(
        fixture
            .memory
            .lookup_binding(&scope, &binding.binding.key, &BindingQuery::default(), NOW)
            .is_err()
    );
    assert!(
        fixture
            .memory
            .binding_alias_candidates(&scope, "Cobalt", Sensitivity::Secret, 10)
            .is_err()
    );
    scope = fixture.scope.clone();
    scope.workspace_id = WorkspaceId::new();
    assert_eq!(
        reason(
            &fixture
                .memory
                .lookup_binding(&scope, &binding.binding.key, &BindingQuery::default(), NOW)
                .unwrap()
        ),
        BindingResolutionReason::UnknownIdentity
    );
    let mut same_entity = draft(URL);
    same_entity.entity = BindingEntityTarget::Existing {
        entity_id: binding.binding.key.entity_id,
    };
    let source = fixture.source(&format!("Other source {URL}"));
    assert!(
        fixture
            .memory
            .memory_create_binding(&scope, create_request(&source), same_entity, NOW)
            .is_err()
    );
}

#[test]
fn exact_aliases_preserve_ambiguous_identities_and_conflicting_properties() {
    let fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let first = fixture.create(&source, URL);
    let second = fixture.create(&source, URL);
    assert_ne!(first.binding.key.entity_id, second.binding.key.entity_id);
    let aliases = fixture
        .memory
        .binding_alias_candidates(&fixture.scope, "Use Cobalt now", Sensitivity::Personal, 1)
        .unwrap();
    assert_eq!(aliases.candidates.len(), 1);
    assert!(aliases.truncated);
    assert_eq!(aliases.ambiguous_aliases, vec!["Cobalt"]);
    assert!(
        fixture
            .memory
            .binding_alias_candidates(
                &fixture.scope,
                "Cobaltish cobalt",
                Sensitivity::Personal,
                10
            )
            .unwrap()
            .candidates
            .is_empty()
    );
    let other = fixture.source(&format!("Cobalt might use {NEXT}."));
    let mut conflicting = draft(NEXT);
    conflicting.entity = BindingEntityTarget::Existing {
        entity_id: first.binding.key.entity_id.clone(),
    };
    fixture
        .memory
        .memory_create_binding(&fixture.scope, create_request(&other), conflicting, NOW)
        .unwrap();
    assert_eq!(
        reason(&fixture.lookup(&first.binding)),
        BindingResolutionReason::ConflictingValues
    );
    let key = ObjectBindingKey {
        entity_id: first.binding.key.entity_id,
        property: "other.endpoint".into(),
    };
    assert_eq!(
        reason(
            &fixture
                .memory
                .lookup_binding(&fixture.scope, &key, &BindingQuery::default(), NOW)
                .unwrap()
        ),
        BindingResolutionReason::MissingProperty
    );
    let key = ObjectBindingKey {
        entity_id: EntityId::new(),
        property: "api.endpoint".into(),
    };
    assert_eq!(
        reason(
            &fixture
                .memory
                .lookup_binding(&fixture.scope, &key, &BindingQuery::default(), NOW)
                .unwrap()
        ),
        BindingResolutionReason::UnknownIdentity
    );
}

#[test]
fn freshness_effective_gaps_and_inferred_source_policy_are_explicit() {
    let fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let binding = fixture.create(&source, URL);
    let query = BindingQuery {
        freshness: BindingFreshness {
            max_age_ms: Some(10),
            observed_not_before: None,
        },
        ..BindingQuery::default()
    };
    assert_eq!(
        reason(
            &fixture
                .memory
                .lookup_binding(
                    &fixture.scope,
                    &binding.binding.key,
                    &query,
                    UtcTimestamp::from_unix_millis(1011)
                )
                .unwrap()
        ),
        BindingResolutionReason::TooOld
    );
    let query = BindingQuery {
        effective_at: Some(NOW),
        ..BindingQuery::default()
    };
    assert_eq!(
        reason(
            &fixture
                .memory
                .lookup_binding(&fixture.scope, &binding.binding.key, &query, NOW)
                .unwrap()
        ),
        BindingResolutionReason::EffectiveTimeUnknown
    );
    let mut interval = draft(URL);
    interval.effective = Some(EvidenceEffectiveInterval {
        from: Some(UtcTimestamp::from_unix_millis(2000)),
        until: Some(UtcTimestamp::from_unix_millis(3000)),
    });
    let future = fixture
        .memory
        .memory_create_binding(&fixture.scope, create_request(&source), interval, NOW)
        .unwrap();
    assert_eq!(
        reason(&fixture.lookup(&future.binding)),
        BindingResolutionReason::OutsideEffectiveInterval
    );
    let mut request = create_request(&source);
    request.text = format!("Generated interpretation using {URL}");
    let generated = fixture.memory.memory_create(request, NOW).unwrap();
    let generated_binding = fixture.create(&generated, URL);
    assert_eq!(
        resolved(fixture.lookup(&generated_binding.binding)).source_authority,
        EvidenceAuthority::DerivedInference
    );
    assert!(
        fixture
            .memory
            .validate_binding_use(
                &fixture.scope,
                &generated_binding.binding,
                URL,
                &policy(),
                NOW
            )
            .is_err()
    );
}

#[test]
fn ambiguous_quotes_and_invalid_source_contracts_never_append_partial_owners() {
    let fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}, repeated {URL}."));
    let before = fs::read(fixture.path()).unwrap();
    assert!(
        fixture
            .memory
            .memory_create_binding(&fixture.scope, create_request(&source), draft(URL), NOW)
            .is_err()
    );
    let mut explicit = draft(URL);
    let start = source.text.find(URL).unwrap();
    explicit.value_span = Some(BindingSourceSpan {
        start: u32::try_from(start).unwrap(),
        end: u32::try_from(start + URL.len()).unwrap(),
    });
    let mut bad = explicit.clone();
    bad.value_span.as_mut().unwrap().end -= 1;
    assert!(
        fixture
            .memory
            .memory_create_binding(&fixture.scope, create_request(&source), bad, NOW)
            .is_err()
    );
    let mut bad = explicit.clone();
    bad.property = "API Endpoint".into();
    assert!(
        fixture
            .memory
            .memory_create_binding(&fixture.scope, create_request(&source), bad, NOW)
            .is_err()
    );
    let mut bad = explicit.clone();
    bad.entity = BindingEntityTarget::NewAlias {
        alias: "unsafe\nname".into(),
    };
    assert!(
        fixture
            .memory
            .memory_create_binding(&fixture.scope, create_request(&source), bad, NOW)
            .is_err()
    );
    assert_eq!(fs::read(fixture.path()).unwrap(), before);
    fixture
        .memory
        .memory_create_binding(&fixture.scope, create_request(&source), explicit, NOW)
        .unwrap();
}

#[test]
fn required_lookup_is_one_current_snapshot_and_refuses_duplicate_or_historical_requirements() {
    let fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let binding = fixture.create(&source, URL);
    let request = BindingLookupRequest {
        key: binding.binding.key.clone(),
        query: BindingQuery::default(),
    };
    let result = fixture
        .memory
        .resolve_required_bindings(&fixture.scope, std::slice::from_ref(&request), NOW)
        .unwrap();
    assert!(result.complete);
    assert_eq!(
        resolved(result.bindings[0].clone()).archive_revision,
        result.archive_revision
    );
    assert!(
        fixture
            .memory
            .resolve_required_bindings(&fixture.scope, &[request.clone(), request.clone()], NOW)
            .is_err()
    );
    let mut historical = request.clone();
    historical.query.recorded_as_of = Some(result.archive_revision);
    assert!(
        fixture
            .memory
            .resolve_required_bindings(&fixture.scope, &[historical], NOW)
            .is_err()
    );
    let mut absent = request;
    absent.key.property = "missing".into();
    let result = fixture
        .memory
        .resolve_required_bindings(&fixture.scope, &[absent], NOW)
        .unwrap();
    assert!(!result.complete);
    assert_eq!(
        reason(&result.bindings[0].clone()),
        BindingResolutionReason::MissingProperty
    );
}

#[test]
fn independent_services_revalidate_correction_and_source_changes_from_canonical_vault() {
    let fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let first = fixture.create(&source, URL);
    let other = open(fixture.root.path(), &fixture.scope.profile_id);
    let source = fixture.source(&format!("Cobalt now uses {NEXT}."));
    let next = other
        .memory_correct_binding(
            &fixture.scope,
            corrected(&source, &first),
            &first.binding,
            correction(NEXT),
            NOW,
        )
        .unwrap();
    assert_eq!(
        resolved(fixture.lookup(&first.binding)).reference,
        next.binding
    );
    assert!(
        fixture
            .memory
            .memory_correct_binding(
                &fixture.scope,
                corrected(&source, &first),
                &first.binding,
                correction(NEXT),
                NOW
            )
            .is_err()
    );
    other
        .observatory
        .apply(
            vec![ObservatoryMutation::Delete {
                evidence_id: source.id,
                source_entries: vec![],
                source_digests: vec![],
            }],
            NOW,
        )
        .unwrap();
    assert_eq!(
        reason(&fixture.lookup(&next.binding)),
        BindingResolutionReason::DeletedSource
    );
}

#[test]
fn torn_bound_correction_recovers_whole_previous_pair_and_preserves_old_vault_bytes() {
    let mut fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let old = fixture.create(&source, URL);
    let source = fixture.source(&format!("Cobalt now uses {NEXT}."));
    let before = fs::read(fixture.path()).unwrap();
    let new = fixture
        .memory
        .memory_correct_binding(
            &fixture.scope,
            corrected(&source, &old),
            &old.binding,
            correction(NEXT),
            NOW,
        )
        .unwrap();
    let after = fs::read(fixture.path()).unwrap();
    assert!(after.starts_with(&before));
    let cut = before.len() + (after.len() - before.len()) / 2;
    fs::write(fixture.path(), &after[..cut]).unwrap();
    fixture.reopen();
    assert_eq!(
        resolved(fixture.lookup(&old.binding)).reference,
        old.binding
    );
    assert!(
        !fixture
            .memory
            .observatory
            .evidence_snapshot()
            .unwrap()
            .contains_key(&new.evidence.id)
    );
    assert_eq!(fs::read(fixture.path()).unwrap(), before);
}

#[test]
fn correction_closure_reports_cycles_missing_links_and_limits_without_vector_identity() {
    let fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let old = fixture.create(&source, URL);
    let mut snapshot = fixture.memory.observatory.binding_snapshot(None).unwrap();
    snapshot
        .index
        .records
        .get_mut(&old.binding.binding_id)
        .unwrap()
        .prior = Some(old.binding.clone());
    assert_eq!(
        reason(&resolve(
            &snapshot,
            &fixture.scope,
            &old.binding.key,
            &BindingQuery::default(),
            NOW
        )),
        BindingResolutionReason::Cycle
    );
    let mut missing = old.binding.clone();
    missing.binding_id = EntityId::new();
    snapshot
        .index
        .records
        .get_mut(&old.binding.binding_id)
        .unwrap()
        .prior = Some(missing);
    assert_eq!(
        reason(&resolve(
            &snapshot,
            &fixture.scope,
            &old.binding.key,
            &BindingQuery::default(),
            NOW
        )),
        BindingResolutionReason::MissingCorrectionLink
    );
    snapshot
        .index
        .keys
        .get_mut(&(fixture.scope.workspace_id.clone(), old.binding.key.clone()))
        .unwrap()
        .resize(MAX_HISTORY + 1, old.binding.binding_id);
    assert_eq!(
        reason(&resolve(
            &snapshot,
            &fixture.scope,
            &old.binding.key,
            &BindingQuery::default(),
            NOW
        )),
        BindingResolutionReason::Limit
    );
}

#[test]
fn exact_current_binding_survives_ten_thousand_committed_distractors_and_restart_without_index() {
    let mut fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let initial = fixture.create(&source, URL);
    let corrected_source = fixture.source(&format!("Cobalt now uses {NEXT}."));
    let current = fixture
        .memory
        .memory_correct_binding(
            &fixture.scope,
            corrected(&corrected_source, &initial),
            &initial.binding,
            correction(NEXT),
            NOW,
        )
        .unwrap();
    assert_eq!(
        committed_crowd::seed_committed_crowd(
            &fixture.memory,
            &fixture.store,
            &fixture.scope.profile_id,
            &fixture.scope.workspace_id,
            NOW,
            10_000,
            "Cobalt endpoint district statistics"
        )
        .unwrap(),
        10_000
    );
    assert_eq!(
        fixture
            .memory
            .observatory
            .evidence_snapshot()
            .unwrap()
            .values()
            .filter(|e| e.source_kind == EvidenceSourceKind::UserMessage)
            .count(),
        10_002
    );
    fs::remove_file(fixture.root.path().join(".keith/memory-atlas.json")).unwrap();
    fixture.reopen();
    let requests = [BindingLookupRequest {
        key: current.binding.key.clone(),
        query: BindingQuery::default(),
    }];
    let result = fixture
        .memory
        .resolve_required_bindings(&fixture.scope, &requests, NOW)
        .unwrap();
    assert!(result.complete);
    assert_eq!(
        resolved(result.bindings[0].clone()).reference,
        current.binding
    );
    assert_eq!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &current.binding, NEXT, &policy(), NOW)
            .unwrap()
            .value,
        NEXT
    );
    assert!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &current.binding, URL, &policy(), NOW)
            .is_err()
    );
}

#[test]
fn malformed_complete_binding_chains_fail_closed_during_real_vault_replay() {
    let fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let binding = fixture.create(&source, URL);
    let original = fs::read(fixture.path()).unwrap();
    for missing in [false, true] {
        let mut events: Vec<serde_json::Value> = original
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).unwrap())
            .collect();
        let event = events.last_mut().unwrap();
        let mut prior = binding.binding.clone();
        if missing {
            prior.binding_id = EntityId::new();
        }
        event["mutation"]["binding"]["prior"] = serde_json::to_value(prior).unwrap();
        event.as_object_mut().unwrap().remove("digest");
        let hash = format!(
            "{:x}",
            Sha256::digest(keith_agent_types::canonical_json_bytes(event).unwrap())
        );
        event["digest"] = hash.into();
        let mut bytes = Vec::new();
        for event in events {
            bytes.extend(keith_agent_types::canonical_json_bytes(&event).unwrap());
            bytes.push(b'\n');
        }
        fs::write(fixture.path(), &bytes).unwrap();
        assert!(
            crate::MemoryObservatory::open(
                fixture.root.path(),
                &fixture.scope.profile_id,
                crate::ObservatoryLimits::default(),
                NOW
            )
            .is_err()
        );
        assert_eq!(fs::read(fixture.path()).unwrap(), bytes);
    }
}

#[test]
fn future_dated_bound_correction_switches_only_at_its_effective_time() {
    let fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let old = fixture.create(&source, URL);
    let source = fixture.source(&format!("Cobalt will use {NEXT}."));
    let mut change = correction(NEXT);
    change.effective = Some(EvidenceEffectiveInterval {
        from: Some(UtcTimestamp::from_unix_millis(2000)),
        until: None,
    });
    let next = fixture
        .memory
        .memory_correct_binding(
            &fixture.scope,
            corrected(&source, &old),
            &old.binding,
            change,
            NOW,
        )
        .unwrap();
    assert_eq!(
        resolved(fixture.lookup(&old.binding)).reference,
        old.binding
    );
    assert_eq!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &old.binding, URL, &policy(), NOW)
            .unwrap()
            .reference,
        old.binding
    );
    let later = UtcTimestamp::from_unix_millis(2000);
    assert_eq!(
        resolved(
            fixture
                .memory
                .lookup_binding(
                    &fixture.scope,
                    &old.binding.key,
                    &BindingQuery::default(),
                    later
                )
                .unwrap()
        )
        .reference,
        next.binding
    );
    assert!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &old.binding, URL, &policy(), later)
            .is_err()
    );
}

#[test]
fn explicit_bound_repair_follows_canonical_owner_chain_and_requires_current_owner_id() {
    let mut fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let old = fixture.create(&source, URL);
    let correction_source =
        fixture.source("The previously saved service association needs correction.");
    let first = fixture
        .memory
        .memory_correct(
            MemoryCorrectRequest {
                evidence_id: old.evidence.id.clone(),
                source: citation(&correction_source),
                replacement: "First unbound correction".into(),
                facets: vec![],
                sensitivity: None,
            },
            NOW,
        )
        .unwrap();
    let second = fixture
        .memory
        .memory_correct(
            MemoryCorrectRequest {
                evidence_id: first.id,
                source: citation(&correction_source),
                replacement: "Second unbound correction".into(),
                facets: vec![],
                sensitivity: None,
            },
            NOW,
        )
        .unwrap();
    assert_eq!(
        reason(&fixture.lookup(&old.binding)),
        BindingResolutionReason::UnboundCorrection
    );
    let fresh_source = fixture.source(&format!("Cobalt now uses {NEXT}."));
    let before = fs::read(fixture.path()).unwrap();
    assert!(matches!(
        fixture.memory.memory_correct_binding(
            &fixture.scope,
            corrected(&fresh_source, &old),
            &old.binding,
            correction(NEXT),
            NOW
        ),
        Err(MemoryError::Binding(BindingError::OwnerMismatch))
    ));
    assert_eq!(fs::read(fixture.path()).unwrap(), before);
    let mut repair = corrected(&fresh_source, &old);
    repair.evidence_id = second.id.clone();
    let repaired = fixture
        .memory
        .memory_correct_binding(&fixture.scope, repair, &old.binding, correction(NEXT), NOW)
        .unwrap();
    let after = fs::read(fixture.path()).unwrap();
    assert!(after.starts_with(&before));
    assert_eq!(
        after[before.len()..]
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .count(),
        1
    );
    fixture.reopen();
    assert_eq!(
        resolved(fixture.lookup(&old.binding)).reference,
        repaired.binding
    );
    assert_eq!(repaired.evidence.supersedes, Some(second.id.clone()));
    assert_eq!(
        fixture.memory.observatory.evidence_snapshot().unwrap()[&second.id].superseded_by,
        Some(repaired.evidence.id)
    );
    assert!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &old.binding, URL, &policy(), NOW)
            .is_err()
    );
}

#[test]
fn explicit_bound_repair_can_replace_a_superseded_source_without_reusing_its_authority() {
    let mut fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let old = fixture.create(&source, URL);
    let fresh = fixture.source(&format!("Cobalt now uses {NEXT}."));
    fixture
        .memory
        .memory_correct(
            MemoryCorrectRequest {
                evidence_id: source.id,
                source: citation(&fresh),
                replacement: fresh.text.clone(),
                facets: vec![],
                sensitivity: None,
            },
            NOW,
        )
        .unwrap();
    assert_eq!(
        reason(&fixture.lookup(&old.binding)),
        BindingResolutionReason::UnboundCorrection
    );
    let repaired = fixture
        .memory
        .memory_correct_binding(
            &fixture.scope,
            corrected(&fresh, &old),
            &old.binding,
            correction(NEXT),
            NOW,
        )
        .unwrap();
    fixture.reopen();
    let resolved = resolved(fixture.lookup(&old.binding));
    assert_eq!(resolved.reference, repaired.binding);
    assert_eq!(resolved.reference.evidence_id, fresh.id);
    assert_eq!(resolved.source_authority, EvidenceAuthority::UserAsserted);
    assert_eq!(
        resolved.association_origin,
        BindingAssociationOrigin::Inferred
    );
}

#[test]
fn explicitly_retiring_a_conflicting_owner_preserves_the_other_current_binding() {
    let fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let correct = fixture.create(&source, URL);
    let source = fixture.source(&format!("The same Cobalt might use {NEXT}."));
    let mut competing = draft(NEXT);
    competing.entity = BindingEntityTarget::Existing {
        entity_id: correct.binding.key.entity_id.clone(),
    };
    let wrong = fixture
        .memory
        .memory_create_binding(&fixture.scope, create_request(&source), competing, NOW)
        .unwrap();
    assert_eq!(
        reason(&fixture.lookup(&correct.binding)),
        BindingResolutionReason::ConflictingValues
    );
    fixture
        .memory
        .observatory
        .apply(
            vec![ObservatoryMutation::Delete {
                evidence_id: wrong.evidence.id,
                source_entries: vec![],
                source_digests: vec![],
            }],
            NOW,
        )
        .unwrap();
    assert_eq!(
        resolved(fixture.lookup(&correct.binding)).reference,
        correct.binding
    );
    assert!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &wrong.binding, NEXT, &policy(), NOW)
            .is_err()
    );
}

#[test]
fn historical_lookup_does_not_undo_a_canonical_source_authority_downgrade() {
    let fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let binding = fixture.create(&source, URL);
    let revision = fixture.memory.observatory.revision().unwrap();
    fixture
        .memory
        .observatory
        .apply(
            vec![ObservatoryMutation::AnnotateProvenance {
                evidence_id: source.id,
                metadata: source.causal.unwrap(),
                authority: Some(EvidenceAuthority::DerivedInference),
            }],
            NOW,
        )
        .unwrap();
    let query = BindingQuery {
        recorded_as_of: Some(revision),
        ..BindingQuery::default()
    };
    assert_eq!(
        resolved(
            fixture
                .memory
                .lookup_binding(&fixture.scope, &binding.binding.key, &query, NOW)
                .unwrap()
        )
        .source_authority,
        EvidenceAuthority::DerivedInference
    );
    assert!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &binding.binding, URL, &policy(), NOW)
            .is_err()
    );
}

#[test]
fn future_owner_transition_does_not_waive_current_original_source_correction() {
    let fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let old = fixture.create(&source, URL);
    let future_source = fixture.source(&format!("Cobalt will use {NEXT}."));
    let mut change = correction(NEXT);
    change.effective = Some(EvidenceEffectiveInterval {
        from: Some(UtcTimestamp::from_unix_millis(2000)),
        until: None,
    });
    fixture
        .memory
        .memory_correct_binding(
            &fixture.scope,
            corrected(&future_source, &old),
            &old.binding,
            change,
            NOW,
        )
        .unwrap();
    assert_eq!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &old.binding, URL, &policy(), NOW)
            .unwrap()
            .reference,
        old.binding
    );
    let retraction = fixture.source("The old endpoint is wrong immediately.");
    fixture
        .memory
        .memory_correct(
            MemoryCorrectRequest {
                evidence_id: source.id,
                source: citation(&retraction),
                replacement: retraction.text.clone(),
                facets: vec![],
                sensitivity: None,
            },
            NOW,
        )
        .unwrap();
    assert_eq!(
        reason(&fixture.lookup(&old.binding)),
        BindingResolutionReason::UnboundCorrection
    );
    assert!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &old.binding, URL, &policy(), NOW)
            .is_err()
    );
}

#[test]
fn deleting_an_unbound_terminal_owner_retires_that_exact_conflicting_branch() {
    let fixture = Fixture::new();
    let source = fixture.source(&format!("Cobalt uses {URL}."));
    let correct = fixture.create(&source, URL);
    let source = fixture.source(&format!("Cobalt might use {NEXT}."));
    let mut competing = draft(NEXT);
    competing.entity = BindingEntityTarget::Existing {
        entity_id: correct.binding.key.entity_id.clone(),
    };
    let wrong = fixture
        .memory
        .memory_create_binding(&fixture.scope, create_request(&source), competing, NOW)
        .unwrap();
    let terminal = fixture
        .memory
        .memory_correct(
            MemoryCorrectRequest {
                evidence_id: wrong.evidence.id,
                source: citation(&source),
                replacement: "Unbound correction of the conflicting association".into(),
                facets: vec![],
                sensitivity: None,
            },
            NOW,
        )
        .unwrap();
    assert_eq!(
        reason(&fixture.lookup(&correct.binding)),
        BindingResolutionReason::UnboundCorrection
    );
    fixture
        .memory
        .observatory
        .apply(
            vec![ObservatoryMutation::Delete {
                evidence_id: terminal.id,
                source_entries: vec![],
                source_digests: vec![],
            }],
            NOW,
        )
        .unwrap();
    assert_eq!(
        resolved(fixture.lookup(&correct.binding)).reference,
        correct.binding
    );
    assert!(
        fixture
            .memory
            .validate_binding_use(&fixture.scope, &wrong.binding, NEXT, &policy(), NOW)
            .is_err()
    );
}

fn committed_tool_source(fixture: &Fixture, name: &str, content: &str) -> EvidenceRecord {
    let (call, result) = {
        let mut writer = fixture
            .store
            .acquire_writer(&fixture.scope.session_id, writer_id())
            .unwrap();
        let call_id = keith_agent_types::ToolCallId::new();
        let call = writer
            .append_committed_source(
                writer.manifest().active_leaf.clone(),
                NOW,
                SessionEntryPayload::ToolCall {
                    call_id: call_id.clone(),
                    name: name.into(),
                    arguments: "{}".into(),
                },
            )
            .unwrap();
        let result = writer
            .append_committed_source(
                writer.manifest().active_leaf.clone(),
                NOW,
                SessionEntryPayload::ToolResult {
                    call_id,
                    content: vec![ContentBlock::Text {
                        text: content.into(),
                    }],
                    is_error: false,
                    failure: None,
                },
            )
            .unwrap();
        (call, result)
    };
    fixture.memory.ingest_committed_entry(&call, NOW).unwrap();
    fixture.memory.ingest_committed_entry(&result, NOW).unwrap();
    fixture
        .memory
        .observatory
        .evidence_snapshot()
        .unwrap()
        .into_values()
        .find(|source| {
            source.source_identity
                == format!(
                    "session:{}:entry:{}",
                    fixture.scope.session_id,
                    result.entry().id
                )
        })
        .unwrap()
}

#[test]
fn generated_commitment_descriptions_cannot_be_promoted_into_binding_permitted_observations() {
    for tool in ["commitment_create", "commitment_get"] {
        let fixture = Fixture::new();
        let observed =
            committed_tool_source(&fixture, "read_file", &format!("Observed endpoint {URL}"));
        assert_eq!(observed.authority, EvidenceAuthority::ToolObserved);
        let old = fixture.create(&observed, URL);
        let generated = committed_tool_source(
            &fixture,
            tool,
            &format!(r#"{{"description":"Model invented endpoint {NEXT}","state":"pending"}}"#),
        );
        assert_eq!(generated.authority, EvidenceAuthority::DerivedInference);
        assert!(generated.causal.as_ref().unwrap().source_roots.is_empty());
        let changed = fixture
            .memory
            .memory_correct_binding(
                &fixture.scope,
                corrected(&generated, &old),
                &old.binding,
                correction(NEXT),
                NOW,
            )
            .unwrap();
        assert_eq!(
            resolved(fixture.lookup(&changed.binding)).source_authority,
            EvidenceAuthority::DerivedInference
        );
        let mut permitted = policy();
        permitted
            .allowed_source_authorities
            .push(EvidenceAuthority::ToolObserved);
        assert!(
            fixture
                .memory
                .validate_binding_use(&fixture.scope, &changed.binding, NEXT, &permitted, NOW)
                .is_err()
        );
    }
}
