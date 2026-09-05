//! Exact source-attributed object bindings. Registry identity is not observed truth.
//! The vault owns associations; session-store owns required action dependencies.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use keith_agent_types::{
    BindingTargetKind, BindingTaskScope, EntityId, ObjectBindingKey, ObjectBindingReference,
    ProfileId, Revision, SchemaVersion, UtcTimestamp, WorkspaceId,
};
use keith_session_store::{RetentionClass, Sensitivity};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::unified::{
    normalize_facets, revalidate_source, source_digest_for, strongest_sensitivity,
};
use crate::{
    AgentMemoryKind, EvidenceAuthority, EvidenceEffectiveInterval, EvidenceFacet,
    EvidenceFacetKind, EvidenceRecord, EvidenceSourceKind, EvidenceValidity, MemoryCorrectRequest,
    MemoryCreateRequest, MemoryError, MemoryService, ObservatoryError, ObservatoryMutation,
};

const VERSION: SchemaVersion = SchemaVersion::new(1, 0);
const MAX_VALUE: usize = 16 * 1024;
const MAX_ALIAS: usize = 256;
const MAX_REQUIRED: usize = 128;
const MAX_HISTORY: usize = 4096;
type EvidenceMap = BTreeMap<EntityId, EvidenceRecord>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum BindingEntityTarget {
    Existing { entity_id: EntityId },
    NewAlias { alias: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingSourceSpan {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingDraft {
    pub entity: BindingEntityTarget,
    pub property: String,
    pub target_kind: BindingTargetKind,
    pub value_quote: String,
    #[serde(default)]
    pub value_span: Option<BindingSourceSpan>,
    #[serde(default)]
    pub effective: Option<EvidenceEffectiveInterval>,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingCorrectionDraft {
    pub value_quote: String,
    #[serde(default)]
    pub value_span: Option<BindingSourceSpan>,
    #[serde(default)]
    pub effective: Option<EvidenceEffectiveInterval>,
}

macro_rules! redacted_debug {
    ($($name:ty),+ $(,)?) => { $(impl fmt::Debug for $name {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct(stringify!($name)).finish_non_exhaustive()
        }
    })+ };
}
redacted_debug!(
    BindingDraft,
    BindingCorrectionDraft,
    ResolvedBinding,
    BindingWriteReceipt,
    BindingRecord
);

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingFreshness {
    #[serde(default)]
    pub max_age_ms: Option<u64>,
    #[serde(default)]
    pub observed_not_before: Option<UtcTimestamp>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingQuery {
    #[serde(default)]
    pub effective_at: Option<UtcTimestamp>,
    #[serde(default)]
    pub recorded_as_of: Option<u64>,
    #[serde(default)]
    pub freshness: BindingFreshness,
    pub max_sensitivity: Sensitivity,
}
impl Default for BindingQuery {
    fn default() -> Self {
        Self {
            effective_at: None,
            recorded_as_of: None,
            freshness: BindingFreshness::default(),
            max_sensitivity: Sensitivity::Personal,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingUsePolicy {
    pub target_kind: BindingTargetKind,
    pub max_sensitivity: Sensitivity,
    #[serde(default)]
    pub freshness: BindingFreshness,
    pub allow_inferred_association: bool,
    pub allowed_source_authorities: Vec<EvidenceAuthority>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingLookupRequest {
    pub key: ObjectBindingKey,
    pub query: BindingQuery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingAssociationOrigin {
    Inferred,
    RuntimeDeclared,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedBinding {
    pub reference: ObjectBindingReference,
    pub owner_memory_id: EntityId,
    pub owner_memory_digest: String,
    pub value: String,
    pub target_kind: BindingTargetKind,
    pub source_span: BindingSourceSpan,
    pub source_authority: EvidenceAuthority,
    pub association_origin: BindingAssociationOrigin,
    pub observed_at: UtcTimestamp,
    pub recorded_at: UtcTimestamp,
    pub effective: Option<EvidenceEffectiveInterval>,
    pub archive_revision: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingResolutionReason {
    UnknownIdentity,
    MissingProperty,
    MissingSource,
    DeletedSource,
    MissingOwner,
    DeletedOwner,
    UnboundCorrection,
    MissingCorrectionLink,
    Cycle,
    Limit,
    AmbiguousAlias,
    ConflictingValues,
    EffectiveTimeUnknown,
    OutsideEffectiveInterval,
    TooOld,
    AssociationPolicy,
    SourceAuthorityPolicy,
    SensitivityPolicy,
    WrongTargetKind,
    ValueMismatch,
    Changed,
    DisputedSource,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum BindingResolution {
    Resolved {
        binding: ResolvedBinding,
    },
    Missing {
        key: ObjectBindingKey,
        reason: BindingResolutionReason,
    },
    Stale {
        key: ObjectBindingKey,
        reference: Option<ObjectBindingReference>,
        reason: BindingResolutionReason,
    },
    Conflicting {
        key: ObjectBindingKey,
        candidates: Vec<ObjectBindingReference>,
        reason: BindingResolutionReason,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredBindingResolution {
    pub archive_revision: u64,
    pub bindings: Vec<BindingResolution>,
    pub complete: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingAliasCandidate {
    pub alias: String,
    pub key: ObjectBindingKey,
    pub target_kind: BindingTargetKind,
    pub association_origin: BindingAssociationOrigin,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingAliasCandidates {
    pub archive_revision: u64,
    pub candidates: Vec<BindingAliasCandidate>,
    pub ambiguous_aliases: Vec<String>,
    pub truncated: bool,
}
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingWriteReceipt {
    pub evidence: EvidenceRecord,
    pub binding: ObjectBindingReference,
    pub association_origin: BindingAssociationOrigin,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum BindingError {
    #[error("invalid or oversized binding contract")]
    Invalid,
    #[error("binding scope does not match its canonical owner")]
    Scope,
    #[error(
        "binding correction evidence_id must name the current owning memory, not its source evidence or session entry"
    )]
    OwnerMismatch,
    #[error("binding cannot be used: {0:?}")]
    Unresolved(BindingResolutionReason),
}

/// An opaque mutation generated by canonical, source-checked binding methods.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingMutation(pub(crate) Box<BindingRecord>);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BindingEntity {
    pub id: EntityId,
    pub workspace_id: WorkspaceId,
    pub alias: String,
}
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BindingRecord {
    /// The owner and association commit in one hash-chained event.
    pub memory: EvidenceRecord,
    pub version: SchemaVersion,
    pub scope: BindingTaskScope,
    pub reference: ObjectBindingReference,
    pub owner_memory_id: EntityId,
    pub owner_memory_digest: String,
    pub source_span: BindingSourceSpan,
    pub target_kind: BindingTargetKind,
    pub association_origin: BindingAssociationOrigin,
    pub observed_at: UtcTimestamp,
    pub recorded_at: UtcTimestamp,
    pub effective: Option<EvidenceEffectiveInterval>,
    pub prior: Option<ObjectBindingReference>,
    pub entity: Option<BindingEntity>,
}
#[derive(Clone, Default)]
pub(crate) struct BindingIndex {
    pub entities: BTreeMap<EntityId, BindingEntity>,
    pub records: BTreeMap<EntityId, BindingRecord>,
    pub keys: BTreeMap<(WorkspaceId, ObjectBindingKey), Vec<EntityId>>,
}
pub(crate) struct BindingSnapshot {
    pub evidence: EvidenceMap,
    pub current: EvidenceMap,
    pub index: BindingIndex,
    pub revision: u64,
}

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn interval_valid(interval: Option<&EvidenceEffectiveInterval>) -> bool {
    interval.is_none_or(|v| v.from.zip(v.until).is_none_or(|(a, b)| a < b))
}
fn alias_valid(alias: &str) -> bool {
    !alias.trim().is_empty()
        && alias == alias.trim()
        && alias.len() <= MAX_ALIAS
        && !alias.chars().any(char::is_control)
}
fn sensitivity_allowed(actual: Sensitivity, allowed: Sensitivity) -> bool {
    strongest_sensitivity(actual, allowed) == allowed
}
fn scope_valid(profile: &ProfileId, scope: &BindingTaskScope) -> Result<(), BindingError> {
    if profile == &scope.profile_id {
        Ok(())
    } else {
        Err(BindingError::Scope)
    }
}

impl BindingIndex {
    /// Validate at the event's original position, both before append and during replay.
    pub(crate) fn apply(
        &mut self,
        profile: &ProfileId,
        evidence: &EvidenceMap,
        record: &BindingRecord,
        sequence: u64,
    ) -> Result<(), ObservatoryError> {
        self.check(profile, evidence, record, sequence)
            .map_err(|_| ObservatoryError::InvalidEvidence)?;
        if let Some(entity) = &record.entity {
            self.entities.insert(entity.id.clone(), entity.clone());
        }
        self.keys
            .entry((
                record.scope.workspace_id.clone(),
                record.reference.key.clone(),
            ))
            .or_default()
            .push(record.reference.binding_id.clone());
        self.records
            .insert(record.reference.binding_id.clone(), record.clone());
        Ok(())
    }

    fn check(
        &self,
        profile: &ProfileId,
        evidence: &EvidenceMap,
        record: &BindingRecord,
        sequence: u64,
    ) -> Result<(), BindingError> {
        scope_valid(profile, &record.scope)?;
        record
            .reference
            .validate()
            .map_err(|_| BindingError::Invalid)?;
        if record.version != VERSION
            || record.reference.revision.get() != sequence
            || self.records.contains_key(&record.reference.binding_id)
            || !interval_valid(record.effective.as_ref())
        {
            return Err(BindingError::Invalid);
        }
        let entity = record
            .entity
            .as_ref()
            .or_else(|| self.entities.get(&record.reference.key.entity_id))
            .ok_or(BindingError::Unresolved(
                BindingResolutionReason::UnknownIdentity,
            ))?;
        if entity.id != record.reference.key.entity_id
            || entity.workspace_id != record.scope.workspace_id
            || !alias_valid(&entity.alias)
            || (record.entity.is_some() && self.entities.contains_key(&entity.id))
        {
            return Err(BindingError::Invalid);
        }
        let source = evidence
            .get(&record.reference.evidence_id)
            .ok_or(BindingError::Invalid)?;
        let owner = evidence
            .get(&record.owner_memory_id)
            .ok_or(BindingError::Invalid)?;
        if source.profile_id != *profile
            || owner.profile_id != *profile
            || source.content_digest != record.reference.evidence_digest
            || owner.content_digest != record.owner_memory_digest
            || source.occurred_at != record.observed_at
            || !matches!(
                source.validity,
                EvidenceValidity::Active | EvidenceValidity::Disputed
            )
            || owner.validity != EvidenceValidity::Active
            || owner.source_kind != EvidenceSourceKind::DurableMemory
        {
            return Err(BindingError::Invalid);
        }
        if record.memory.id != owner.id
            || record.memory.content_digest != owner.content_digest
            || record.association_origin != BindingAssociationOrigin::Inferred
            || owner.source_session != source.source_session
            || !owner
                .source_entries
                .iter()
                .zip(&owner.source_digests)
                .all(|(id, checksum)| {
                    source
                        .source_entries
                        .iter()
                        .position(|entry| entry == id)
                        .and_then(|position| source.source_digests.get(position))
                        == Some(checksum)
                })
            || !sensitivity_allowed(source.sensitivity, owner.sensitivity)
            || record.recorded_at != owner.occurred_at
        {
            return Err(BindingError::Invalid);
        }
        let value = span_value(source, record.source_span).ok_or(BindingError::Invalid)?;
        if value.is_empty()
            || value.len() > MAX_VALUE
            || digest(value) != record.reference.value_digest
        {
            return Err(BindingError::Invalid);
        }
        if let Some(prior) = &record.prior {
            let previous = self
                .records
                .get(&prior.binding_id)
                .ok_or(BindingError::Invalid)?;
            if previous.reference != *prior
                || previous.reference.key != record.reference.key
                || previous.scope.workspace_id != record.scope.workspace_id
                || previous.target_kind != record.target_kind
                || self
                    .records
                    .values()
                    .any(|r| r.prior.as_ref() == Some(prior))
                || owner_chain(evidence, previous, Some(&owner.id), true).is_err()
            {
                return Err(BindingError::Invalid);
            }
        }
        Ok(())
    }
}
fn span_value(source: &EvidenceRecord, span: BindingSourceSpan) -> Option<&str> {
    source
        .text
        .get(usize::try_from(span.start).ok()?..usize::try_from(span.end).ok()?)
}

impl MemoryService {
    /// Creates a cited memory and inferred entity/property association in one vault event.
    /// The quoted source, rather than the rewritten memory, owns the binding's evidence reference.
    ///
    /// # Errors
    /// Rejects invalid scope, ambiguous source spans, noncanonical keys, or changed sources.
    /// `MemoryError::Binding`, `InvalidEvidenceQuote`, `InvalidRequest`, and `EmptyText`
    /// are returned before the requested binding write can append. Storage errors and
    /// `Changed` do not guarantee that the write remained uncommitted.
    pub fn memory_create_binding(
        &self,
        scope: &BindingTaskScope,
        request: MemoryCreateRequest,
        draft: BindingDraft,
        now: UtcTimestamp,
    ) -> Result<BindingWriteReceipt, MemoryError> {
        scope_valid(&self.profile_id, scope)?;
        if request.kind == AgentMemoryKind::PreferredName {
            return Err(BindingError::Invalid.into());
        }
        let requires_user = matches!(
            request.kind,
            AgentMemoryKind::Preference
                | AgentMemoryKind::PersonalFact
                | AgentMemoryKind::Routine
                | AgentMemoryKind::Relationship
        );
        let source = self.validate_write_source(
            &request.source,
            requires_user.then_some(EvidenceAuthority::UserAsserted),
        )?;
        let span = quoted_span(
            &source,
            &request.source.evidence_quote,
            &draft.value_quote,
            draft.value_span,
        )?;
        if !interval_valid(draft.effective.as_ref()) {
            return Err(BindingError::Invalid.into());
        }
        let mut facets = request.facets;
        facets.push(EvidenceFacet {
            kind: EvidenceFacetKind::Theme,
            value: request.kind.theme().into(),
        });
        let mut owner = make_owner(
            &source,
            &request.source,
            &request.text,
            facets,
            request.sensitivity,
            now,
        )?;
        let mut result = None;
        self.observatory
            .apply_binding_snapshot(now, |evidence, _, index, revision| {
                let source = revalidate_source(evidence, &source)?;
                let (entity_id, entity) = match &draft.entity {
                    BindingEntityTarget::Existing { entity_id } => {
                        index
                            .entities
                            .get(entity_id)
                            .filter(|entity| entity.workspace_id == scope.workspace_id)
                            .ok_or(ObservatoryError::MissingEvidence)?;
                        (entity_id.clone(), None)
                    }
                    BindingEntityTarget::NewAlias { alias } => {
                        if !alias_valid(alias) {
                            return Err(ObservatoryError::InvalidEvidence);
                        }
                        let entity_id = EntityId::new();
                        (
                            entity_id.clone(),
                            Some(BindingEntity {
                                id: entity_id,
                                workspace_id: scope.workspace_id.clone(),
                                alias: alias.clone(),
                            }),
                        )
                    }
                };
                let key = ObjectBindingKey {
                    entity_id,
                    property: draft.property,
                };
                key.validate()
                    .map_err(|_| ObservatoryError::InvalidEvidence)?;
                owner.causal = Some(crate::ingestion::context_lineage(source, revision));
                owner.sensitivity = strongest_sensitivity(owner.sensitivity, source.sensitivity);
                let record = make_record(
                    scope,
                    key,
                    &owner,
                    source,
                    span,
                    draft.target_kind,
                    draft.effective,
                    revision,
                    now,
                    None,
                    entity,
                )?;
                result = Some(receipt(&record));
                Ok(vec![ObservatoryMutation::Binding(BindingMutation(
                    Box::new(record),
                ))])
            })?;
        self.invalidate_hot_cache();
        result.ok_or(MemoryError::Changed)
    }

    /// Atomically supersedes the owning memory and its exact association. Ordinary memory
    /// corrections remain valid but leave an associated operational dependency unresolved.
    ///
    /// # Errors
    /// Rejects stale references, changes to either source/owner, and unbound corrections.
    /// `MemoryError::Binding`, `InvalidEvidenceQuote`, `InvalidRequest`, `EmptyText`,
    /// and `MissingRecord` are returned before the requested binding write can append.
    /// `BindingError::OwnerMismatch` identifies a wrong current owner ID; it does not
    /// replace stale-reference validation. Storage errors and `Changed` do not
    /// guarantee that the write remained uncommitted.
    pub fn memory_correct_binding(
        &self,
        scope: &BindingTaskScope,
        request: MemoryCorrectRequest,
        expected: &ObjectBindingReference,
        draft: BindingCorrectionDraft,
        now: UtcTimestamp,
    ) -> Result<BindingWriteReceipt, MemoryError> {
        scope_valid(&self.profile_id, scope)?;
        expected.validate().map_err(|_| BindingError::Invalid)?;
        if !interval_valid(draft.effective.as_ref()) {
            return Err(BindingError::Invalid.into());
        }
        let snapshot = self.observatory.binding_snapshot(None)?;
        let previous = snapshot
            .index
            .records
            .get(&expected.binding_id)
            .filter(|record| {
                record.reference == *expected && record.scope.workspace_id == scope.workspace_id
            })
            .ok_or(BindingError::Unresolved(BindingResolutionReason::Changed))?;
        let prior = current_binding_owner(&snapshot.evidence, previous)
            .map_err(BindingError::Unresolved)?;
        if prior.id != request.evidence_id {
            return Err(BindingError::OwnerMismatch.into());
        }
        let previous_source = snapshot
            .evidence
            .get(&expected.evidence_id)
            .ok_or(MemoryError::MissingRecord)?;
        let source = self.validate_write_source(
            &request.source,
            (previous_source.authority == EvidenceAuthority::UserAsserted)
                .then_some(EvidenceAuthority::UserAsserted),
        )?;
        let span = quoted_span(
            &source,
            &request.source.evidence_quote,
            &draft.value_quote,
            draft.value_span,
        )?;
        let facets = if request.facets.is_empty() {
            prior.facets.clone()
        } else {
            request.facets
        };
        let mut owner = make_owner(
            &source,
            &request.source,
            &request.replacement,
            facets,
            request.sensitivity.unwrap_or(prior.sensitivity),
            now,
        )?;
        let mut result = None;
        self.observatory
            .apply_binding_snapshot(now, |evidence, _, index, revision| {
                let source = revalidate_source(evidence, &source)?;
                let prior = revalidate_source(evidence, prior)?;
                revalidate_previous_source(evidence, previous_source)?;
                let previous = index
                    .records
                    .get(&expected.binding_id)
                    .filter(|record| record.reference == *expected)
                    .ok_or(ObservatoryError::MissingEvidence)?;
                if current_binding_owner(evidence, previous)
                    .map_err(|_| ObservatoryError::MissingEvidence)?
                    .id
                    != prior.id
                {
                    return Err(ObservatoryError::MissingEvidence);
                }
                if index
                    .records
                    .values()
                    .any(|record| record.prior.as_ref() == Some(expected))
                {
                    return Err(ObservatoryError::MissingEvidence);
                }
                owner.causal = Some(crate::ingestion::context_lineage(source, revision));
                owner.sensitivity = strongest_sensitivity(
                    strongest_sensitivity(owner.sensitivity, source.sensitivity),
                    prior.sensitivity,
                );
                owner.supersedes = Some(prior.id.clone());
                let record = make_record(
                    scope,
                    expected.key.clone(),
                    &owner,
                    source,
                    span,
                    previous.target_kind,
                    draft.effective,
                    revision,
                    now,
                    Some(expected.clone()),
                    None,
                )?;
                result = Some(receipt(&record));
                Ok(vec![ObservatoryMutation::Binding(BindingMutation(
                    Box::new(record),
                ))])
            })?;
        self.invalidate_hot_cache();
        result.ok_or(MemoryError::Changed)
    }

    /// Resolves a canonical entity/property exactly, independently of ranking or indexing.
    /// Historical reads still enforce current deletion and sensitivity policy.
    ///
    /// # Errors
    /// Rejects malformed/cross-profile queries and unavailable canonical storage.
    pub fn lookup_binding(
        &self,
        scope: &BindingTaskScope,
        key: &ObjectBindingKey,
        query: &BindingQuery,
        now: UtcTimestamp,
    ) -> Result<BindingResolution, MemoryError> {
        scope_valid(&self.profile_id, scope)?;
        key.validate().map_err(|_| BindingError::Invalid)?;
        let snapshot = self.observatory.binding_snapshot(query.recorded_as_of)?;
        Ok(resolve(&snapshot, scope, key, query, now))
    }

    /// Resolves all required keys from one locked canonical snapshot. Historical requests
    /// are intentionally excluded from action admission.
    ///
    /// # Errors
    /// Rejects duplicate, historical, oversized or cross-profile requirements.
    pub fn resolve_required_bindings(
        &self,
        scope: &BindingTaskScope,
        requests: &[BindingLookupRequest],
        now: UtcTimestamp,
    ) -> Result<RequiredBindingResolution, MemoryError> {
        scope_valid(&self.profile_id, scope)?;
        let mut keys = BTreeSet::new();
        if requests.len() > MAX_REQUIRED
            || requests.iter().any(|request| {
                request.key.validate().is_err()
                    || request.query.recorded_as_of.is_some()
                    || request.query.effective_at.is_some()
                    || !keys.insert(&request.key)
            })
        {
            return Err(BindingError::Invalid.into());
        }
        let snapshot = self.observatory.binding_snapshot(None)?;
        let bindings = requests
            .iter()
            .map(|request| resolve(&snapshot, scope, &request.key, &request.query, now))
            .collect::<Vec<_>>();
        let complete = bindings
            .iter()
            .all(|binding| matches!(binding, BindingResolution::Resolved { .. }));
        Ok(RequiredBindingResolution {
            archive_revision: snapshot.revision,
            bindings,
            complete,
        })
    }

    /// Revalidates the exact revision and adapter value immediately before its use.
    /// A matching digest does not grant authority or establish external service truth.
    ///
    /// # Errors
    /// Rejects unresolved/stale references, guessed targets, and disallowed provenance.
    pub fn validate_binding_use(
        &self,
        scope: &BindingTaskScope,
        expected: &ObjectBindingReference,
        proposed_value: &str,
        policy: &BindingUsePolicy,
        now: UtcTimestamp,
    ) -> Result<ResolvedBinding, MemoryError> {
        expected.validate().map_err(|_| BindingError::Invalid)?;
        if policy.allowed_source_authorities.len() > 6 {
            return Err(BindingError::Invalid.into());
        }
        let query = BindingQuery {
            max_sensitivity: policy.max_sensitivity,
            freshness: policy.freshness.clone(),
            ..BindingQuery::default()
        };
        let binding = match self.lookup_binding(scope, &expected.key, &query, now)? {
            BindingResolution::Resolved { binding } => binding,
            BindingResolution::Missing { reason, .. }
            | BindingResolution::Stale { reason, .. }
            | BindingResolution::Conflicting { reason, .. } => {
                return Err(BindingError::Unresolved(reason).into());
            }
        };
        let reason = if binding.reference != *expected {
            Some(BindingResolutionReason::Changed)
        } else if binding.target_kind != policy.target_kind {
            Some(BindingResolutionReason::WrongTargetKind)
        } else if binding.value != proposed_value {
            Some(BindingResolutionReason::ValueMismatch)
        } else if binding.association_origin == BindingAssociationOrigin::Inferred
            && !policy.allow_inferred_association
        {
            Some(BindingResolutionReason::AssociationPolicy)
        } else if !policy
            .allowed_source_authorities
            .contains(&binding.source_authority)
        {
            Some(BindingResolutionReason::SourceAuthorityPolicy)
        } else {
            None
        };
        if let Some(reason) = reason {
            return Err(BindingError::Unresolved(reason).into());
        }
        Ok(binding)
    }

    /// Returns bounded candidates only for registered aliases occurring as exact tokens.
    /// Aliases never merge entity identities and ambiguous aliases stay explicit.
    ///
    /// # Errors
    /// Rejects oversized context/limits, cross-profile scope, and unavailable storage.
    pub fn binding_alias_candidates(
        &self,
        scope: &BindingTaskScope,
        context: &str,
        max_sensitivity: Sensitivity,
        limit: usize,
    ) -> Result<BindingAliasCandidates, MemoryError> {
        scope_valid(&self.profile_id, scope)?;
        if context.len() > 64 * 1024 || !(1..=MAX_REQUIRED).contains(&limit) {
            return Err(BindingError::Invalid.into());
        }
        let snapshot = self.observatory.binding_snapshot(None)?;
        let mut aliases: BTreeMap<String, BTreeSet<EntityId>> = BTreeMap::new();
        let mut candidates = Vec::new();
        let mut truncated = false;
        for ((workspace, key), ids) in &snapshot.index.keys {
            if *workspace != scope.workspace_id {
                continue;
            }
            let Some(entity) = snapshot.index.entities.get(&key.entity_id) else {
                continue;
            };
            if !alias_occurs(context, &entity.alias) {
                continue;
            }
            let visible = ids
                .iter()
                .filter_map(|id| snapshot.index.records.get(id))
                .find(|record| privacy_check(&snapshot, record, max_sensitivity).is_ok());
            let Some(record) = visible else {
                continue;
            };
            aliases
                .entry(entity.alias.clone())
                .or_default()
                .insert(entity.id.clone());
            if candidates.len() == limit {
                truncated = true;
                continue;
            }
            candidates.push(BindingAliasCandidate {
                alias: entity.alias.clone(),
                key: key.clone(),
                target_kind: record.target_kind,
                association_origin: record.association_origin,
            });
        }
        let ambiguous_aliases = aliases
            .into_iter()
            .filter(|(_, ids)| ids.len() > 1)
            .map(|(alias, _)| alias)
            .take(MAX_REQUIRED)
            .collect();
        Ok(BindingAliasCandidates {
            archive_revision: snapshot.revision,
            candidates,
            ambiguous_aliases,
            truncated,
        })
    }
}

fn revalidate_previous_source(
    evidence: &EvidenceMap,
    previous: &EvidenceRecord,
) -> Result<(), ObservatoryError> {
    evidence
        .get(&previous.id)
        .filter(|source| {
            source.profile_id == previous.profile_id
                && source.content_digest == previous.content_digest
                && source.source_digests == previous.source_digests
                && source.authority == previous.authority
                && source.validity != EvidenceValidity::Deleted
        })
        .map(|_| ())
        .ok_or(ObservatoryError::MissingEvidence)
}

fn alias_occurs(context: &str, alias: &str) -> bool {
    context.match_indices(alias).any(|(start, _)| {
        let end = start + alias.len();
        context[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric() && c != '_')
            && context[end..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric() && c != '_')
    })
}

fn quoted_span(
    source: &EvidenceRecord,
    evidence_quote: &str,
    value: &str,
    span: Option<BindingSourceSpan>,
) -> Result<BindingSourceSpan, MemoryError> {
    if value.is_empty() || value.len() > MAX_VALUE || !evidence_quote.trim().contains(value) {
        return Err(BindingError::Invalid.into());
    }
    if let Some(span) = span {
        if span_value(source, span) == Some(value) {
            return Ok(span);
        }
        return Err(BindingError::Invalid.into());
    }
    let mut matches = source.text.match_indices(value);
    let (start, _) = matches.next().ok_or(BindingError::Invalid)?;
    if matches.next().is_some() {
        return Err(BindingError::Invalid.into());
    }
    Ok(BindingSourceSpan {
        start: u32::try_from(start).map_err(|_| BindingError::Invalid)?,
        end: u32::try_from(start + value.len()).map_err(|_| BindingError::Invalid)?,
    })
}

fn make_owner(
    source: &EvidenceRecord,
    citation: &crate::MemoryWriteSource,
    text: &str,
    mut facets: Vec<EvidenceFacet>,
    sensitivity: Sensitivity,
    now: UtcTimestamp,
) -> Result<EvidenceRecord, MemoryError> {
    if text.trim().is_empty() {
        return Err(MemoryError::EmptyText);
    }
    normalize_facets(&mut facets)?;
    let id = EntityId::new();
    let mut owner = EvidenceRecord::new(
        source.profile_id.clone(),
        source.source_session.clone(),
        vec![citation.source_entry_id.clone()],
        vec![source_digest_for(source, &citation.source_entry_id)?],
        format!("agent-memory:{id}"),
        Some(citation.source_entry_id.clone()),
        EvidenceSourceKind::DurableMemory,
        if text.trim() == citation.evidence_quote.trim() {
            source.authority
        } else {
            EvidenceAuthority::DerivedInference
        },
        text.trim().to_owned(),
        now,
        sensitivity,
        RetentionClass::Durable,
        facets,
    );
    owner.id = id;
    Ok(owner)
}

#[allow(clippy::too_many_arguments)]
fn make_record(
    scope: &BindingTaskScope,
    key: ObjectBindingKey,
    owner: &EvidenceRecord,
    source: &EvidenceRecord,
    span: BindingSourceSpan,
    target_kind: BindingTargetKind,
    effective: Option<EvidenceEffectiveInterval>,
    revision: u64,
    now: UtcTimestamp,
    prior: Option<ObjectBindingReference>,
    entity: Option<BindingEntity>,
) -> Result<BindingRecord, ObservatoryError> {
    let value = span_value(source, span).ok_or(ObservatoryError::InvalidEvidence)?;
    Ok(BindingRecord {
        memory: owner.clone(),
        version: VERSION,
        scope: scope.clone(),
        reference: ObjectBindingReference {
            key,
            binding_id: EntityId::new(),
            revision: Revision::new(
                revision
                    .checked_add(1)
                    .ok_or(ObservatoryError::InvalidEvidence)?,
            ),
            evidence_id: source.id.clone(),
            evidence_digest: source.content_digest.clone(),
            value_digest: digest(value),
        },
        owner_memory_id: owner.id.clone(),
        owner_memory_digest: owner.content_digest.clone(),
        source_span: span,
        target_kind,
        association_origin: BindingAssociationOrigin::Inferred,
        observed_at: source.occurred_at,
        recorded_at: now,
        effective,
        prior,
        entity,
    })
}

fn receipt(record: &BindingRecord) -> BindingWriteReceipt {
    BindingWriteReceipt {
        evidence: record.memory.clone(),
        binding: record.reference.clone(),
        association_origin: record.association_origin,
    }
}

fn missing(key: &ObjectBindingKey, reason: BindingResolutionReason) -> BindingResolution {
    BindingResolution::Missing {
        key: key.clone(),
        reason,
    }
}
fn stale(
    key: &ObjectBindingKey,
    record: Option<&BindingRecord>,
    reason: BindingResolutionReason,
) -> BindingResolution {
    BindingResolution::Stale {
        key: key.clone(),
        reference: record.map(|r| r.reference.clone()),
        reason,
    }
}

fn resolve(
    snapshot: &BindingSnapshot,
    scope: &BindingTaskScope,
    key: &ObjectBindingKey,
    query: &BindingQuery,
    now: UtcTimestamp,
) -> BindingResolution {
    if snapshot
        .index
        .entities
        .get(&key.entity_id)
        .is_none_or(|e| e.workspace_id != scope.workspace_id)
    {
        return missing(key, BindingResolutionReason::UnknownIdentity);
    }
    let Some(ids) = snapshot
        .index
        .keys
        .get(&(scope.workspace_id.clone(), key.clone()))
    else {
        return missing(key, BindingResolutionReason::MissingProperty);
    };
    let chains = match correction_chains(&snapshot.index, ids) {
        Ok(chains) => chains,
        Err(reason) => return stale(key, None, reason),
    };
    let mut candidates = Vec::new();
    let mut inactive = None;
    for chain in chains {
        let record = match effective_record(&chain, query, now) {
            Ok(record) => record,
            Err(BindingResolutionReason::OutsideEffectiveInterval) => {
                inactive = Some(stale(
                    key,
                    chain.last().copied(),
                    BindingResolutionReason::OutsideEffectiveInterval,
                ));
                continue;
            }
            Err(reason) => return stale(key, chain.last().copied(), reason),
        };
        if let Err(reason) = check_owner_chain(snapshot, &chain) {
            if reason == BindingResolutionReason::DeletedOwner {
                inactive = Some(stale(key, Some(record), reason));
                continue;
            }
            return stale(key, Some(record), reason);
        }
        let historical = query.recorded_as_of.is_some() || query.effective_at.is_some();
        let earlier_owner = chain
            .last()
            .is_some_and(|last| last.reference != record.reference);
        match resolve_record(snapshot, record, query, now, historical, earlier_owner) {
            Ok(binding) => candidates.push(binding),
            Err(
                reason @ (BindingResolutionReason::DeletedSource
                | BindingResolutionReason::DeletedOwner),
            ) => {
                inactive = Some(stale(key, Some(record), reason));
            }
            Err(reason) => return stale(key, Some(record), reason),
        }
    }
    match candidates.len() {
        0 => inactive.unwrap_or_else(|| missing(key, BindingResolutionReason::MissingProperty)),
        1 => BindingResolution::Resolved {
            binding: candidates.remove(0),
        },
        _ => BindingResolution::Conflicting {
            key: key.clone(),
            candidates: candidates
                .into_iter()
                .map(|binding| binding.reference)
                .collect(),
            reason: BindingResolutionReason::ConflictingValues,
        },
    }
}

fn correction_chains<'a>(
    index: &'a BindingIndex,
    ids: &[EntityId],
) -> Result<Vec<Vec<&'a BindingRecord>>, BindingResolutionReason> {
    if ids.len() > MAX_HISTORY {
        return Err(BindingResolutionReason::Limit);
    }
    let mut children: BTreeMap<&EntityId, &BindingRecord> = BTreeMap::new();
    let mut roots = Vec::new();
    let expected = ids.iter().collect::<BTreeSet<_>>();
    if expected.len() != ids.len() {
        return Err(BindingResolutionReason::MissingCorrectionLink);
    }
    for id in ids {
        let record = index
            .records
            .get(id)
            .ok_or(BindingResolutionReason::MissingCorrectionLink)?;
        if let Some(prior) = &record.prior {
            let previous = index
                .records
                .get(&prior.binding_id)
                .ok_or(BindingResolutionReason::MissingCorrectionLink)?;
            if previous.reference != *prior
                || record.reference.key != prior.key
                || previous.scope.workspace_id != record.scope.workspace_id
                || !expected.contains(&prior.binding_id)
            {
                return Err(BindingResolutionReason::MissingCorrectionLink);
            }
            if children.insert(&prior.binding_id, record).is_some() {
                return Err(BindingResolutionReason::ConflictingValues);
            }
        } else {
            roots.push(record);
        }
    }
    let mut visited = BTreeSet::new();
    let mut chains = Vec::new();
    for root in roots {
        let mut chain = Vec::new();
        let mut current = Some(root);
        while let Some(record) = current {
            if !visited.insert(&record.reference.binding_id) {
                return Err(BindingResolutionReason::Cycle);
            }
            chain.push(record);
            current = children.get(&record.reference.binding_id).copied();
        }
        chains.push(chain);
    }
    if visited.len() != ids.len() {
        return Err(BindingResolutionReason::Cycle);
    }
    Ok(chains)
}

fn effective_record<'a>(
    chain: &[&'a BindingRecord],
    query: &BindingQuery,
    now: UtcTimestamp,
) -> Result<&'a BindingRecord, BindingResolutionReason> {
    let at = query.effective_at.unwrap_or(now);
    for record in chain.iter().rev() {
        match &record.effective {
            None if query.effective_at.is_some() => {
                return Err(BindingResolutionReason::EffectiveTimeUnknown);
            }
            None => return Ok(record),
            Some(interval) => {
                if interval.from.is_some_and(|from| at < from) {
                    continue;
                }
                if interval.until.is_some_and(|until| at >= until) {
                    return Err(BindingResolutionReason::OutsideEffectiveInterval);
                }
                return Ok(record);
            }
        }
    }
    Err(BindingResolutionReason::OutsideEffectiveInterval)
}

fn check_owner_chain(
    snapshot: &BindingSnapshot,
    chain: &[&BindingRecord],
) -> Result<(), BindingResolutionReason> {
    for (position, record) in chain.iter().enumerate() {
        let owner = snapshot
            .evidence
            .get(&record.owner_memory_id)
            .ok_or(BindingResolutionReason::MissingOwner)?;
        let next = chain.get(position + 1);
        if let Some(next) = next {
            owner_chain(
                &snapshot.evidence,
                record,
                Some(&next.owner_memory_id),
                true,
            )?;
        } else if owner.superseded_by.is_some() || owner.validity == EvidenceValidity::Superseded {
            let terminal = owner_chain(&snapshot.evidence, record, None, true)?;
            return Err(if terminal.validity == EvidenceValidity::Deleted {
                BindingResolutionReason::DeletedOwner
            } else {
                BindingResolutionReason::UnboundCorrection
            });
        }
    }
    Ok(())
}

/// Finds the current canonical owner for an explicit repair. It grants no new
/// identity or access; the caller must also name this exact owner in its request.
pub(crate) fn current_binding_owner<'a>(
    evidence: &'a EvidenceMap,
    record: &BindingRecord,
) -> Result<&'a EvidenceRecord, BindingResolutionReason> {
    owner_chain(evidence, record, None, false)
}

fn owner_chain<'a>(
    evidence: &'a EvidenceMap,
    record: &BindingRecord,
    stop: Option<&EntityId>,
    allow_deleted: bool,
) -> Result<&'a EvidenceRecord, BindingResolutionReason> {
    let mut current = evidence
        .get(&record.owner_memory_id)
        .ok_or(BindingResolutionReason::MissingOwner)?;
    if current.content_digest != record.owner_memory_digest {
        return Err(BindingResolutionReason::Changed);
    }
    let mut visited = BTreeSet::new();
    loop {
        if visited.len() >= MAX_HISTORY {
            return Err(BindingResolutionReason::Limit);
        }
        if !visited.insert(&current.id) {
            return Err(BindingResolutionReason::Cycle);
        }
        if current.profile_id != record.scope.profile_id
            || current.source_kind != EvidenceSourceKind::DurableMemory
        {
            return Err(BindingResolutionReason::MissingCorrectionLink);
        }
        if stop == Some(&current.id) {
            return Ok(current);
        }
        if !allow_deleted && current.validity == EvidenceValidity::Deleted {
            return Err(BindingResolutionReason::DeletedOwner);
        }
        let Some(next) = current.superseded_by.as_ref() else {
            if stop.is_some() || current.validity == EvidenceValidity::Superseded {
                return Err(BindingResolutionReason::MissingCorrectionLink);
            }
            return Ok(current);
        };
        if !matches!(
            current.validity,
            EvidenceValidity::Superseded | EvidenceValidity::Deleted
        ) {
            return Err(BindingResolutionReason::MissingCorrectionLink);
        }
        let next = evidence
            .get(next)
            .ok_or(BindingResolutionReason::MissingCorrectionLink)?;
        if next.supersedes.as_ref() != Some(&current.id) {
            return Err(BindingResolutionReason::MissingCorrectionLink);
        }
        current = next;
    }
}

fn privacy_check(
    snapshot: &BindingSnapshot,
    record: &BindingRecord,
    sensitivity: Sensitivity,
) -> Result<(), BindingResolutionReason> {
    let source = snapshot
        .current
        .get(&record.reference.evidence_id)
        .ok_or(BindingResolutionReason::MissingSource)?;
    let owner = snapshot
        .current
        .get(&record.owner_memory_id)
        .ok_or(BindingResolutionReason::MissingOwner)?;
    if source.validity == EvidenceValidity::Deleted {
        return Err(BindingResolutionReason::DeletedSource);
    }
    if owner.validity == EvidenceValidity::Deleted {
        return Err(BindingResolutionReason::DeletedOwner);
    }
    if !sensitivity_allowed(source.sensitivity, sensitivity)
        || !sensitivity_allowed(owner.sensitivity, sensitivity)
    {
        return Err(BindingResolutionReason::SensitivityPolicy);
    }
    Ok(())
}

fn resolve_record(
    snapshot: &BindingSnapshot,
    record: &BindingRecord,
    query: &BindingQuery,
    now: UtcTimestamp,
    historical: bool,
    earlier_owner: bool,
) -> Result<ResolvedBinding, BindingResolutionReason> {
    privacy_check(snapshot, record, query.max_sensitivity)?;
    let source = snapshot
        .evidence
        .get(&record.reference.evidence_id)
        .ok_or(BindingResolutionReason::MissingSource)?;
    let owner = snapshot
        .evidence
        .get(&record.owner_memory_id)
        .ok_or(BindingResolutionReason::MissingOwner)?;
    if !sensitivity_allowed(source.sensitivity, query.max_sensitivity)
        || !sensitivity_allowed(owner.sensitivity, query.max_sensitivity)
    {
        return Err(BindingResolutionReason::SensitivityPolicy);
    }
    if source.content_digest != record.reference.evidence_digest
        || owner.content_digest != record.owner_memory_digest
    {
        return Err(BindingResolutionReason::Changed);
    }
    if source.validity == EvidenceValidity::Disputed || owner.validity == EvidenceValidity::Disputed
    {
        return Err(BindingResolutionReason::DisputedSource);
    }
    if !historical
        && (source.validity == EvidenceValidity::Superseded
            || (!earlier_owner && owner.validity == EvidenceValidity::Superseded))
    {
        return Err(BindingResolutionReason::UnboundCorrection);
    }
    if source.validity == EvidenceValidity::Deleted {
        return Err(BindingResolutionReason::DeletedSource);
    }
    if owner.validity == EvidenceValidity::Deleted {
        return Err(BindingResolutionReason::DeletedOwner);
    }
    if query
        .freshness
        .observed_not_before
        .is_some_and(|earliest| source.occurred_at < earliest)
        || query.freshness.max_age_ms.is_some_and(|max| {
            let age = i128::from(now.unix_millis()) - i128::from(source.occurred_at.unix_millis());
            age < 0 || age > i128::from(max)
        })
    {
        return Err(BindingResolutionReason::TooOld);
    }
    let value = span_value(source, record.source_span).ok_or(BindingResolutionReason::Changed)?;
    if digest(value) != record.reference.value_digest {
        return Err(BindingResolutionReason::Changed);
    }
    Ok(ResolvedBinding {
        reference: record.reference.clone(),
        owner_memory_id: owner.id.clone(),
        owner_memory_digest: owner.content_digest.clone(),
        value: value.to_owned(),
        target_kind: record.target_kind,
        source_span: record.source_span,
        // A historical prefix must not undo a later provenance downgrade.
        source_authority: snapshot
            .current
            .get(&source.id)
            .ok_or(BindingResolutionReason::MissingSource)?
            .authority,
        association_origin: record.association_origin,
        observed_at: source.occurred_at,
        recorded_at: record.recorded_at,
        effective: record.effective.clone(),
        archive_revision: snapshot.revision,
    })
}

#[cfg(test)]
#[path = "bindings_tests.rs"]
mod tests;
