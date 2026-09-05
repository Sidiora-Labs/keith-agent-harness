//! Real store, tool manager, workspace, and provider-boundary proofs for binding admission.
use super::bindings::{BindingExecutor, target_slot};
use super::tests::{ProviderServer, response, responses_text_stream, seed_provider_credential};
use super::*;
use keith_agent_loop::{ToolAdmission, ToolAdmissionError};
use keith_agent_types::{BindingTargetKind, ToolCallId, ToolFailureStatus};
use keith_memory::BindingWriteReceipt;
use keith_session_store::{RequiredObjectBinding, SessionWriter};
use keith_tool_core::{ToolExecutor, ToolInvocation};
use serde_json::{Value, json};

use keith_memory as memory_api;
#[path = "../../memory/tests/support/committed_crowd.rs"]
mod committed_crowd;

struct Fixture {
    _root: tempfile::TempDir,
    runtime: LocalRuntime,
    profile: RegisteredProfile,
    session: SessionManifest,
    scope: BindingTaskScope,
    modules: Arc<ProfileModules>,
    tools: ToolManager,
}

impl Fixture {
    fn new(base_url: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let credentials = root.path().join("credentials");
        seed_provider_credential(
            &credentials,
            [93; 32],
            "openai",
            "synthetic-binding-credential",
        );
        let runtime = LocalRuntime::open(LocalRuntimeConfig {
            data_root: root.path().join("data"),
            credential_root: credentials,
            credential_key: MasterKey::from_bytes([93; 32]),
            workspace_root: root.path().join("workspace"),
            openai_base_url: base_url.into(),
            anthropic_base_url: base_url.into(),
            provider_base_urls: BTreeMap::new(),
            root_scope: None,
            worker_id: WorkerId::new(),
            owner_instance: EntityId::new(),
        })
        .unwrap();
        let profile = runtime.registered_profiles().unwrap().remove(0);
        let session = runtime
            .create_session(&profile.profile.id, &profile.profile.workspace_id, None)
            .unwrap();
        let scope = runtime
            .binding_task_scope(&session, &ActionId::new())
            .unwrap();
        let modules = runtime.profile_modules(&profile).unwrap();
        let tools = runtime
            .tool_manager(&profile, &session.session_id, "binding proof", &scope)
            .unwrap();
        Self {
            _root: root,
            runtime,
            profile,
            session,
            scope,
            modules,
            tools,
        }
    }

    fn writer(&self) -> SessionWriter {
        self.runtime
            .sessions
            .acquire_writer(
                &self.session.session_id,
                self.runtime
                    .writer_identity(Generation::new(1), UtcTimestamp::now().unwrap()),
            )
            .unwrap()
    }

    fn source(&self, text: &str) -> EntryId {
        let receipt = {
            let mut writer = self.writer();
            writer
                .append_committed_source(
                    writer.manifest().active_leaf.clone(),
                    UtcTimestamp::now().unwrap(),
                    SessionEntryPayload::UserMessage {
                        message: message(StoredMessageRole::User, text),
                    },
                )
                .unwrap()
        };
        self.modules
            .memory
            .ingest_committed_entry(&receipt, UtcTimestamp::now().unwrap())
            .unwrap();
        receipt.entry().id.clone()
    }

    fn binding(&self, alias: &str, value: &str, kind: BindingTargetKind) -> BindingWriteReceipt {
        let text = format!("The {alias} location is {value}");
        let source = self.source(&text);
        let invocation = call(
            "memory_create",
            json!({"source_entry_id": source, "evidence_quote": text,
            "text": format!("{alias} has an attributed location."), "kind": "project_context",
            "binding": {"entity": {"mode": "new_alias", "alias": alias}, "property": "location",
                "target_kind": kind, "value_quote": value}}),
        );
        serde_json::from_slice(
            &self
                .tools
                .execute(&invocation, &CancellationToken::default())
                .unwrap(),
        )
        .unwrap()
    }

    fn turn(&self, required: Vec<RequiredObjectBinding>) -> (SessionWriter, TurnId) {
        let mut writer = self.writer();
        let ingress = append(
            &mut writer,
            SessionEntryPayload::UserMessage {
                message: message(StoredMessageRole::User, "Use the current project objects"),
            },
        );
        let turn = TurnId::new();
        writer
            .accept_turn(
                UtcTimestamp::now().unwrap(),
                self.scope.action_id.clone(),
                turn.clone(),
                ingress.id,
            )
            .unwrap();
        if !required.is_empty() {
            writer
                .require_object_bindings(self.scope.clone(), required, UtcTimestamp::now().unwrap())
                .unwrap();
        }
        append(
            &mut writer,
            SessionEntryPayload::AssistantActivity {
                turn_id: turn.clone(),
                message: message(StoredMessageRole::Assistant, "Inspect the current object"),
            },
        );
        (writer, turn)
    }

    fn executor(&self) -> BindingExecutor<'_> {
        BindingExecutor::new(
            self.scope.clone(),
            Arc::clone(&self.modules),
            &self.tools,
            "Use current project objects",
        )
    }
}

fn message(role: StoredMessageRole, text: &str) -> StoredMessage {
    StoredMessage {
        role,
        content: vec![StoredContentBlock::Text { text: text.into() }],
        provider_metadata: BTreeMap::new(),
    }
}
fn append(writer: &mut SessionWriter, payload: SessionEntryPayload) -> SessionEntry {
    writer
        .append(
            writer.manifest().active_leaf.clone(),
            UtcTimestamp::now().unwrap(),
            payload,
        )
        .unwrap()
}
fn call(name: &str, arguments: Value) -> ToolInvocation {
    ToolInvocation {
        call_id: ToolCallId::new(),
        name: name.into(),
        arguments,
    }
}

fn tool_stream(name: &str, arguments: &Value) -> String {
    let item = json!({"type": "function_call", "name": name, "arguments": arguments.to_string()});
    let added = json!({"type": "response.output_item.added", "output_index": 0,
        "item": {"type": "function_call", "name": name, "arguments": ""}});
    let done = json!({"type": "response.output_item.done", "output_index": 0, "item": item});
    format!(
        "data: {added}\n\ndata: {done}\n\ndata: {{\"type\":\"response.completed\",\"response\":{{\"usage\":{{\"input_tokens\":1,\"output_tokens\":1}}}}}}\n\n"
    )
}
fn intent(writer: &mut SessionWriter, invocation: &ToolInvocation) {
    append(
        writer,
        SessionEntryPayload::ToolCall {
            call_id: invocation.call_id.clone(),
            name: invocation.name.clone(),
            arguments: invocation.arguments.clone(),
        },
    );
}
fn requirement(receipt: &BindingWriteReceipt, kind: BindingTargetKind) -> RequiredObjectBinding {
    RequiredObjectBinding {
        key: receipt.binding.key.clone(),
        target: target_slot(kind).unwrap(),
    }
}
fn denied(result: Result<(), ToolAdmissionError>, code: &str) {
    match result {
        Err(ToolAdmissionError::Rejected(failure)) => {
            assert_eq!(failure.error.code, code);
            assert_eq!(failure.status, ToolFailureStatus::NotStarted);
            assert_eq!(failure.effect_state, ToolEffectState::NotStarted);
        }
        other => panic!("expected pre-dispatch rejection: {other:?}"),
    }
}

#[test]
fn exact_read_rejects_guesses_and_opaque_bypass_even_when_reference_is_omitted() {
    let fixture = Fixture::new("http://127.0.0.1:65535");
    let receipt = fixture.binding("Cobalt", "current.txt", BindingTargetKind::WorkspacePath);
    fs::write(
        fixture.profile.resources.workspace_root.join("current.txt"),
        "file-only-current-nonce",
    )
    .unwrap();
    fs::write(
        fixture.profile.resources.workspace_root.join("guessed.txt"),
        "wrong-object-nonce",
    )
    .unwrap();
    let (mut writer, turn) = fixture.turn(vec![requirement(
        &receipt,
        BindingTargetKind::WorkspacePath,
    )]);
    let executor = fixture.executor();
    let guessed = call("read", json!({"path": "guessed.txt"}));
    intent(&mut writer, &guessed);
    denied(
        executor.admit(&mut writer, &turn, &guessed),
        "BINDING_TARGET_MISMATCH",
    );
    assert!(
        executor
            .execute(&guessed, &CancellationToken::default())
            .is_err()
    );
    let bypass = call("bash", json!({"command": "printf bypass > bypass.txt"}));
    intent(&mut writer, &bypass);
    denied(
        executor.admit(&mut writer, &turn, &bypass),
        "BINDING_UNSUPPORTED_TARGET",
    );
    for name in ["write", "list", "search", "review_content"] {
        let invocation = call(name, json!({"path": "guessed.txt"}));
        intent(&mut writer, &invocation);
        denied(
            executor.admit(&mut writer, &turn, &invocation),
            "BINDING_UNSUPPORTED_TARGET",
        );
    }
    assert!(
        !fixture
            .profile
            .resources
            .workspace_root
            .join("bypass.txt")
            .exists()
    );
    let correct = call("read", json!({"path": "current.txt"}));
    intent(&mut writer, &correct);
    executor.admit(&mut writer, &turn, &correct).unwrap();
    assert_eq!(
        executor
            .execute(&correct, &CancellationToken::default())
            .unwrap(),
        b"file-only-current-nonce"
    );
    drop(writer);
    assert_eq!(
        fixture
            .writer()
            .required_object_bindings(&fixture.scope)
            .unwrap(),
        vec![requirement(&receipt, BindingTargetKind::WorkspacePath)]
    );
}

#[test]
fn alias_refresh_catches_new_bindings_and_unavailable_context_blocks_opaque_dispatch() {
    let fixture = Fixture::new("http://127.0.0.1:65535");
    // The executor exists before capture; admission must consult the current registry.
    let executor = BindingExecutor::new(
        fixture.scope.clone(),
        Arc::clone(&fixture.modules),
        &fixture.tools,
        "Inspect Cobalt",
    );
    let receipt = fixture.binding("Cobalt", "current.txt", BindingTargetKind::WorkspacePath);
    let (mut writer, turn) = fixture.turn(vec![]);
    let wrong = call("read", json!({"path": "wrong.txt"}));
    intent(&mut writer, &wrong);
    denied(
        executor.admit(&mut writer, &turn, &wrong),
        "BINDING_TARGET_MISMATCH",
    );
    assert_eq!(
        writer.required_object_bindings(&fixture.scope).unwrap(),
        vec![requirement(&receipt, BindingTargetKind::WorkspacePath)]
    );
    drop(writer);

    let empty = Fixture::new("http://127.0.0.1:65535");
    let oversized = "x".repeat(65 * 1024);
    let unavailable = BindingExecutor::new(
        empty.scope.clone(),
        Arc::clone(&empty.modules),
        &empty.tools,
        &oversized,
    );
    let (mut writer, turn) = empty.turn(vec![]);
    let opaque = call(
        "bash",
        json!({"command": "printf forbidden > sentinel.txt"}),
    );
    intent(&mut writer, &opaque);
    denied(
        unavailable.admit(&mut writer, &turn, &opaque),
        "BINDING_UNSUPPORTED_TARGET",
    );
    assert!(
        !empty
            .profile
            .resources
            .workspace_root
            .join("sentinel.txt")
            .exists()
    );
}

#[test]
fn bound_write_rejections_distinguish_owner_source_and_uncertain_storage_failures() {
    let fixture = Fixture::new("http://127.0.0.1:65535");
    let receipt = fixture.binding("Cobalt", "old.txt", BindingTargetKind::WorkspacePath);
    let source = fixture.source("Cobalt moved to new.txt");
    let correction = call(
        "memory_correct",
        json!({"evidence_id": receipt.evidence.id,
        "source_entry_id": source, "evidence_quote": "Cobalt moved to new.txt",
        "replacement": "Cobalt has a corrected location",
        "expected_binding": receipt.binding, "binding": {"value_quote": "new.txt"}}),
    );
    let vault = fixture
        .profile
        .resources
        .workspace_root
        .join(".keith/.keith/memory-vault.jsonl");
    let before = fs::read(&vault).unwrap();
    for wrong_owner in [
        json!(EntityId::new()),
        json!(source),
        json!(receipt.binding.evidence_id),
    ] {
        let mut invalid = correction.clone();
        invalid.call_id = ToolCallId::new();
        invalid.arguments["evidence_id"] = wrong_owner;
        let error = fixture
            .tools
            .execute(&invalid, &CancellationToken::default())
            .unwrap_err();
        assert_eq!(error.failure.error.code, "BINDING_OWNER_MISMATCH");
        assert_eq!(error.failure.effect_state, ToolEffectState::NotCommitted);
        assert!(!error.failure.retry.automatic);
        assert!(!error.retryable);
        assert!(error.message.contains("owner_memory_id"));
        assert_eq!(fs::read(&vault).unwrap(), before);
    }
    for (field, invalid_value) in [
        ("evidence_quote", "an invented source quotation"),
        ("replacement", ""),
    ] {
        let mut invalid = correction.clone();
        invalid.call_id = ToolCallId::new();
        invalid.arguments[field] = json!(invalid_value);
        let error = fixture
            .tools
            .execute(&invalid, &CancellationToken::default())
            .unwrap_err();
        assert_eq!(error.failure.error.code, "BINDING_WRITE_REJECTED");
        assert_eq!(error.failure.effect_state, ToolEffectState::NotCommitted);
        assert!(!error.failure.retry.automatic);
        assert_eq!(fs::read(&vault).unwrap(), before);
    }

    // An actual unavailable vault still has a conservative classification. This
    // proves storage errors are not covered by the precondition-only mapping;
    // it does not claim a universally recoverable write or an after-append fault.
    let backup = vault.with_extension("saved");
    fs::rename(&vault, &backup).unwrap();
    fs::create_dir(&vault).unwrap();
    let error = fixture
        .tools
        .execute(&correction, &CancellationToken::default())
        .unwrap_err();
    assert_eq!(error.failure.effect_state, ToolEffectState::Unknown);
    assert!(!error.failure.retry.automatic);
    fs::remove_dir(&vault).unwrap();
    fs::rename(&backup, &vault).unwrap();
    assert_eq!(fs::read(&vault).unwrap(), before);

    let current = fixture
        .modules
        .memory
        .lookup_binding(
            &fixture.scope,
            &receipt.binding.key,
            &keith_memory::BindingQuery::default(),
            UtcTimestamp::now().unwrap(),
        )
        .unwrap();
    let keith_memory::BindingResolution::Resolved { binding } = current else {
        panic!("original binding must remain current after rejected writes");
    };
    assert_eq!(binding.reference, receipt.binding);
    let corrected: BindingWriteReceipt = serde_json::from_slice(
        &fixture
            .tools
            .execute(&correction, &CancellationToken::default())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(corrected.binding.key, receipt.binding.key);
    assert!(corrected.binding.revision > receipt.binding.revision);
    assert_ne!(corrected.evidence.id, receipt.evidence.id);
}

#[test]
fn correction_between_admission_and_dispatch_requires_a_new_current_reference() {
    let fixture = Fixture::new("http://127.0.0.1:65535");
    let receipt = fixture.binding("Cobalt", "old.txt", BindingTargetKind::WorkspacePath);
    fs::write(
        fixture.profile.resources.workspace_root.join("old.txt"),
        "stale",
    )
    .unwrap();
    fs::write(
        fixture.profile.resources.workspace_root.join("new.txt"),
        "fresh-only-nonce",
    )
    .unwrap();
    let source = fixture.source("Cobalt moved to new.txt");
    let (mut writer, turn) = fixture.turn(vec![requirement(
        &receipt,
        BindingTargetKind::WorkspacePath,
    )]);
    let executor = fixture.executor();
    let old = call("read", json!({"path": "old.txt"}));
    intent(&mut writer, &old);
    executor.admit(&mut writer, &turn, &old).unwrap();
    let correction = call(
        "memory_correct",
        json!({"evidence_id": receipt.evidence.id,
        "source_entry_id": source, "evidence_quote": "Cobalt moved to new.txt", "replacement": "Cobalt has a corrected location",
        "expected_binding": receipt.binding, "binding": {"value_quote": "new.txt"}}),
    );
    let updated: BindingWriteReceipt = serde_json::from_slice(
        &fixture
            .tools
            .execute(&correction, &CancellationToken::default())
            .unwrap(),
    )
    .unwrap();
    let failure = executor
        .execute(&old, &CancellationToken::default())
        .unwrap_err()
        .failure;
    assert_eq!(failure.error.code, "BINDING_CHANGED");
    assert_eq!(failure.effect_state, ToolEffectState::NotStarted);
    let new = call("read", json!({"path": "new.txt"}));
    intent(&mut writer, &new);
    executor.admit(&mut writer, &turn, &new).unwrap();
    assert_eq!(
        executor
            .execute(&new, &CancellationToken::default())
            .unwrap(),
        b"fresh-only-nonce"
    );
    let context = call(
        "memory_context",
        json!({"query": "current", "required_bindings": [requirement(&receipt, BindingTargetKind::WorkspacePath)]}),
    );
    intent(&mut writer, &context);
    executor.admit(&mut writer, &turn, &context).unwrap();
    let value: Value = serde_json::from_slice(
        &executor
            .execute(&context, &CancellationToken::default())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        value["required_bindings"]["bindings"][0]["binding"]["reference"],
        json!(updated.binding)
    );
}

#[test]
fn guessed_web_target_never_reaches_the_network_adapter() {
    let fixture = Fixture::new("http://127.0.0.1:65535");
    let receipt = fixture.binding(
        "Cobalt",
        "https://canonical.example/api",
        BindingTargetKind::HttpUrl,
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let (mut writer, turn) = fixture.turn(vec![requirement(&receipt, BindingTargetKind::HttpUrl)]);
    let executor = fixture.executor();
    let guessed = call(
        "web_fetch",
        json!({"url": format!("http://{}/plausible-page", listener.local_addr().unwrap())}),
    );
    intent(&mut writer, &guessed);
    denied(
        executor.admit(&mut writer, &turn, &guessed),
        "BINDING_TARGET_MISMATCH",
    );
    assert!(
        executor
            .execute(&guessed, &CancellationToken::default())
            .is_err()
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn two_required_files_need_an_explicit_selection_and_preserve_both_dependencies() {
    let fixture = Fixture::new("http://127.0.0.1:65535");
    let first = fixture.binding("Cobalt", "first.txt", BindingTargetKind::WorkspacePath);
    let second = fixture.binding("Amber", "second.txt", BindingTargetKind::WorkspacePath);
    fs::write(
        fixture.profile.resources.workspace_root.join("second.txt"),
        "second-object",
    )
    .unwrap();
    let required = vec![
        requirement(&first, BindingTargetKind::WorkspacePath),
        requirement(&second, BindingTargetKind::WorkspacePath),
    ];
    let (mut writer, turn) = fixture.turn(required);
    let executor = fixture.executor();
    let ambiguous = call("read", json!({"path": "second.txt"}));
    intent(&mut writer, &ambiguous);
    denied(
        executor.admit(&mut writer, &turn, &ambiguous),
        "BINDING_AMBIGUOUS",
    );
    let selected = call(
        "read",
        json!({"path": "second.txt", "object_binding": second.binding.key}),
    );
    intent(&mut writer, &selected);
    executor.admit(&mut writer, &turn, &selected).unwrap();
    assert_eq!(
        executor
            .execute(&selected, &CancellationToken::default())
            .unwrap(),
        b"second-object"
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
fn actual_provider_context_and_loop_refuse_a_wrong_object_without_reading_its_content() {
    let stream = tool_stream("read", &json!({"path": "wrong.txt"}));
    let server = ProviderServer::start(vec![
        response("application/json", r#"{"data":[{"id":"gpt-4.1-mini"}]}"#),
        response("text/event-stream", &stream),
        response(
            "text/event-stream",
            &responses_text_stream("The target mismatch needs correction.", 2, 2),
        ),
    ]);
    let fixture = Fixture::new(&server.base_url);
    let receipt = fixture.binding("Cobalt", "current.txt", BindingTargetKind::WorkspacePath);
    fs::write(
        fixture.profile.resources.workspace_root.join("wrong.txt"),
        "WRONG_OBJECT_SECRET_SENTINEL",
    )
    .unwrap();
    let snapshot = fixture
        .runtime
        .run_prompt(
            &fixture.session.session_id,
            "Inspect Cobalt",
            Generation::new(1),
        )
        .unwrap();
    assert_eq!(
        snapshot.terminal.as_ref().unwrap().status,
        ProjectionTurnTerminalStatus::Completed
    );
    let _discovery = server.request();
    let first = server.request();
    let second = server.request();
    assert!(first.contains(&receipt.binding.key.entity_id.to_string()));
    assert!(first.contains("current.txt"));
    let second_body: Value =
        serde_json::from_str(second.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert!(
        second.contains(
            "The proposed target or source policy does not match the current required binding"
        ),
        "provider input: {}",
        second_body["input"]
    );
    assert!(!second.contains("WRONG_OBJECT_SECRET_SENTINEL"));
    let history = fixture.writer().active_ancestry().unwrap();
    assert!(history.iter().any(|entry| matches!(&entry.payload, SessionEntryPayload::ToolResult {failure: Some(failure), ..} if failure.effect_state == ToolEffectState::NotStarted)));
}

#[test]
fn corrected_binding_reaches_actual_provider_and_read_amid_ten_thousand_sources() {
    let nonce = "crowded-archive-file-only-nonce";
    let server = ProviderServer::start(vec![
        response("application/json", r#"{"data":[{"id":"gpt-4.1-mini"}]}"#),
        response(
            "text/event-stream",
            &tool_stream("read", &json!({"path": "current.txt"})),
        ),
        response("text/event-stream", &responses_text_stream(nonce, 2, 2)),
    ]);
    let fixture = Fixture::new(&server.base_url);
    let old = fixture.binding("Cobalt", "old.txt", BindingTargetKind::WorkspacePath);
    let source = fixture.source("Cobalt moved to current.txt");
    let correction = call(
        "memory_correct",
        json!({"evidence_id": old.evidence.id,
        "source_entry_id": source, "evidence_quote": "Cobalt moved to current.txt",
        "replacement": "Cobalt has a corrected location", "expected_binding": old.binding,
        "binding": {"value_quote": "current.txt"}}),
    );
    let current: BindingWriteReceipt = serde_json::from_slice(
        &fixture
            .tools
            .execute(&correction, &CancellationToken::default())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        committed_crowd::seed_committed_crowd(
            &fixture.modules.memory,
            &fixture.runtime.sessions,
            &fixture.profile.profile.id,
            &fixture.profile.profile.workspace_id,
            UtcTimestamp::now().unwrap(),
            10_000,
            "Cobalt location current status statistics",
        )
        .unwrap(),
        10_000
    );
    assert_eq!(
        fixture
            .modules
            .memory
            .observatory()
            .evidence_snapshot()
            .unwrap()
            .values()
            .filter(|record| record.source_kind == memory_api::EvidenceSourceKind::UserMessage)
            .count(),
        10_002
    );
    fs::write(
        fixture.profile.resources.workspace_root.join("current.txt"),
        nonce,
    )
    .unwrap();
    let fresh = fixture
        .runtime
        .create_session(
            &fixture.profile.profile.id,
            &fixture.profile.profile.workspace_id,
            None,
        )
        .unwrap();
    let snapshot = fixture
        .runtime
        .run_prompt(&fresh.session_id, "Inspect Cobalt", Generation::new(1))
        .unwrap();
    assert_eq!(
        snapshot.terminal.as_ref().unwrap().status,
        ProjectionTurnTerminalStatus::Completed
    );
    let _discovery = server.request();
    let first = server.request();
    let second = server.request();
    assert!(first.contains(&current.binding.binding_id.to_string()));
    assert!(first.contains("current.txt"));
    assert!(!first.contains(nonce));
    assert!(second.contains(nonce));
    let manifest = fixture
        .runtime
        .sessions
        .manifest(&fresh.session_id)
        .unwrap();
    let history = fixture
        .runtime
        .sessions
        .load_index(&fresh.session_id)
        .unwrap()
        .ancestry(manifest.active_leaf.as_ref().unwrap())
        .unwrap();
    assert!(history.iter().any(|entry| matches!(&entry.payload,
        SessionEntryPayload::BindingAdmission { admission } if admission.bindings.iter().any(|used| used.reference == current.binding))));
    assert!(history.iter().any(|entry| matches!(&entry.payload,
        SessionEntryPayload::ToolResult { content, is_error: false, .. } if stored_text(content) == nonce)));
}

#[test]
fn commitment_get_reads_current_revision_and_preserves_profile_isolation() {
    let fixture = Fixture::new("http://127.0.0.1:65535");
    let create = call(
        "commitment_create",
        json!({"description": "Check current Cobalt status"}),
    );
    let created: keith_commitments::Commitment = serde_json::from_slice(
        &fixture
            .tools
            .execute(&create, &CancellationToken::default())
            .unwrap(),
    )
    .unwrap();
    fixture
        .runtime
        .system_modules
        .commitments
        .activate(&created.id, UtcTimestamp::now().unwrap())
        .unwrap();
    let get = call("commitment_get", json!({"id": created.id}));
    let current: Value = serde_json::from_slice(
        &fixture
            .tools
            .execute(&get, &CancellationToken::default())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(current["commitment"]["state"], "active");
    assert_eq!(current["revision"], 1);
    assert!(
        fixture
            .runtime
            .system_modules
            .commitments
            .inspect_scoped(&ProfileId::new(), &created.id)
            .unwrap()
            .is_none()
    );
}
