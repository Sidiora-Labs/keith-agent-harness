//! Test-only crowded archive setup. The caller imports its real memory crate as
//! `memory_api` in the parent module. This contains no target binding or controller.
//!
//! All sources first commit through real `SessionWriter`s. Their exact receipts seed
//! one canonical CommitSource/Observe batch. The ordinary automatic intake path is
//! qualified separately; repeating its full-vault projection on many setup pages
//! would measure unrelated setup cost rather than the crowded binding journey.

use super::memory_api::{
    EVIDENCE_CAUSAL_VERSION, EvidenceAuthority, EvidenceCausalMetadata, EvidenceRecord,
    EvidenceSourceKind, EvidenceSourceRoot, MemoryService, ObservatoryMutation,
};
use keith_agent_types::{
    EntityId, Generation, ProfileId, RootTreeId, SessionId, UtcTimestamp, WorkerId, WorkspaceId,
};
use keith_session_store::{
    ContentBlock, MessageRole, NewSession, RetentionClass, Sensitivity, SessionEntryPayload,
    SessionKind, SessionStore, StoredMessage, WriterIdentity,
};
use std::collections::BTreeMap;

/// Commits up to 10,000 unrelated real sources in the caller's profile/workspace,
/// then publishes their exact evidence in a single real canonical vault batch.
/// Returns the number of distinct committed source records added.
///
/// # Errors
/// Propagates actual session/vault errors and rejects an oversized setup request.
pub fn seed_committed_crowd(
    memory: &MemoryService,
    store: &SessionStore,
    profile: &ProfileId,
    workspace: &WorkspaceId,
    now: UtcTimestamp,
    count: usize,
    prefix: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    if count > 10_000 || prefix.len() > 256 {
        return Err(
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "oversized test crowd").into(),
        );
    }
    let mut mutations = Vec::with_capacity(count * 2);
    for base in (0..count).step_by(10) {
        let session = SessionId::new();
        store.create(NewSession {
            kind: SessionKind::Root,
            session_id: session.clone(),
            root_tree_id: RootTreeId::new(),
            parent_session_id: None,
            profile_id: profile.clone(),
            workspace_id: workspace.clone(),
            created_at: now,
            label: None,
            profile_snapshot: None,
        })?;
        let mut writer = store.acquire_writer(
            &session,
            WriterIdentity {
                worker_id: WorkerId::new(),
                owner_instance: EntityId::new(),
                generation: Generation::new(1),
                acquired_at: now,
            },
        )?;
        for number in base..(base + 10).min(count) {
            let receipt = writer.append_committed_source(
                writer.manifest().active_leaf.clone(),
                now,
                SessionEntryPayload::UserMessage {
                    message: StoredMessage {
                        role: MessageRole::User,
                        content: vec![ContentBlock::Text {
                            text: format!("{prefix} unrelated report {number}."),
                        }],
                        provider_metadata: BTreeMap::new(),
                    },
                },
            )?;
            receipt.entry().verify()?;
            let entry = receipt.entry();
            let text = match &entry.payload {
                SessionEntryPayload::UserMessage { message } => match message.content.as_slice() {
                    [ContentBlock::Text { text }] => text.clone(),
                    _ => {
                        return Err(
                            std::io::Error::other("unexpected committed test content").into()
                        );
                    }
                },
                _ => return Err(std::io::Error::other("unexpected committed test source").into()),
            };
            let mut evidence = EvidenceRecord::new(
                profile.clone(),
                session.clone(),
                vec![entry.id.clone()],
                vec![entry.checksum.clone()],
                format!("session:{session}:entry:{}", entry.id),
                entry.parent_id.clone(),
                EvidenceSourceKind::UserMessage,
                EvidenceAuthority::UserAsserted,
                format!("User: {text}"),
                entry.timestamp,
                Sensitivity::Personal,
                RetentionClass::Daily,
                vec![],
            );
            evidence.causal = Some(EvidenceCausalMetadata {
                version: EVIDENCE_CAUSAL_VERSION,
                effective: None,
                source_roots: vec![EvidenceSourceRoot {
                    source_session: session.clone(),
                    source_entry: entry.id.clone(),
                    source_digest: entry.checksum.clone(),
                }],
                derived_from: vec![],
                gaps: vec![],
            });
            mutations.push(ObservatoryMutation::CommitSource(receipt.reference()));
            mutations.push(ObservatoryMutation::Observe(evidence));
        }
    }
    memory.observatory().apply(mutations, now)?;
    Ok(count)
}
