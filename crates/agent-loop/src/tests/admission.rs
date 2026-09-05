//! Admission-port coverage with the real HTTP provider adapter, session owner,
//! process execution and filesystem failures. Exact memory resolution is tested
//! by local-runtime, which owns the production implementation of this port.
use super::*;
use keith_agent_types::{ActionId, BindingTaskScope, CURRENT_SCHEMA_VERSION};
use keith_session_store::{FrozenBindingAdmission, binding_arguments_digest};
use std::fmt::Write as _;
use std::path::PathBuf;

struct StoreAdmission {
    scope: BindingTaskScope,
    history: PathBuf,
}

struct CommandExecutor;

impl ToolExecutor for CommandExecutor {
    fn execute(
        &self,
        invocation: &ToolInvocation,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, ToolExecutionError> {
        cancellation
            .check()
            .map_err(|error| ToolExecutionError::new(error.to_string()))?;
        let command = invocation
            .arguments
            .get("command")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ToolExecutionError::new("missing command"))?;
        let output = Command::new("sh")
            .args(["-c", command])
            .output()
            .map_err(|error| ToolExecutionError::new(error.to_string()))?;
        if !output.status.success() {
            return Err(ToolExecutionError::new("test subprocess failed"));
        }
        Ok(output.stdout)
    }
}

impl ToolAdmission for StoreAdmission {
    fn admit(
        &self,
        session: &mut SessionWriter,
        turn_id: &TurnId,
        invocation: &ToolInvocation,
    ) -> Result<(), ToolAdmissionError> {
        if invocation.name == "blocked" {
            return Err(ToolAdmissionError::Rejected(Box::new(
                ToolFailure::not_committed(
                    ToolErrorCategory::PolicyDenied,
                    "BINDING_TARGET_MISMATCH",
                    "binding_target_mismatch",
                    "the proposed target differs from its required binding",
                ),
            )));
        }
        let admission = FrozenBindingAdmission {
            version: CURRENT_SCHEMA_VERSION,
            scope: self.scope.clone(),
            turn_id: turn_id.clone(),
            call_id: invocation.call_id.clone(),
            tool_name: invocation.name.clone(),
            arguments_digest: binding_arguments_digest(&invocation.arguments)?,
            required: session.required_object_bindings(&self.scope)?,
            bindings: Vec::new(),
            admitted_at: UtcTimestamp::UNIX_EPOCH,
        };
        if invocation.name == "storage_fault" {
            // Make the actual owner's persistence path unreadable, then restore
            // it so the scheduler can durably report what did and did not start.
            let saved = self.history.with_extension("saved");
            fs::rename(&self.history, &saved).unwrap();
            fs::create_dir(&self.history).unwrap();
            let result = session.append_binding_admission(admission);
            fs::remove_dir(&self.history).unwrap();
            fs::rename(saved, &self.history).unwrap();
            result?;
        } else {
            session.append_binding_admission(admission)?;
        }
        Ok(())
    }
}

fn accepted_scope(writer: &mut SessionWriter, request: &mut ModelRequest) -> BindingTaskScope {
    let turn_id = request.context.messages[0][0].turn_id.clone();
    let entry = writer
        .append(
            writer.manifest().active_leaf.clone(),
            UtcTimestamp::UNIX_EPOCH,
            SessionEntryPayload::UserMessage {
                message: StoredMessage {
                    role: StoredMessageRole::User,
                    content: vec![StoredContentBlock::Text {
                        text: "work".into(),
                    }],
                    provider_metadata: BTreeMap::new(),
                },
            },
        )
        .unwrap();
    request.context.active_user_entry_id = entry.id.clone();
    request.context.messages[0][0].entry_id = entry.id.clone();
    request.context.messages[0][0].session_id = writer.manifest().session_id.clone();
    let scope = BindingTaskScope {
        profile_id: writer.manifest().profile_id.clone(),
        workspace_id: writer.manifest().workspace_id.clone(),
        session_id: writer.manifest().session_id.clone(),
        action_id: ActionId::new(),
        goal_id: None,
    };
    writer
        .accept_turn(
            UtcTimestamp::UNIX_EPOCH,
            scope.action_id.clone(),
            turn_id,
            entry.id,
        )
        .unwrap();
    scope
}

fn tool_stream(calls: &[(&str, &str)]) -> String {
    let mut stream = String::new();
    for (index, (name, command)) in calls.iter().enumerate() {
        let added = json!({"type":"response.output_item.added","output_index":index,
            "item":{"type":"function_call","name":name,"arguments":""}});
        let done = json!({"type":"response.output_item.done","output_index":index,
            "item":{"type":"function_call","name":name,"arguments":json!({"command":command}).to_string()}});
        write!(stream, "data: {added}\n\ndata: {done}\n\n").unwrap();
    }
    stream.push_str("data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":5,\"output_tokens\":3}}}\n\n");
    stream
}

fn server(calls: &[(&str, &str)], final_reply: bool) -> TestServer {
    let mut responses = vec![
        http_response("application/json", r#"{"data":[{"id":"model-a"}]}"#),
        http_response("text/event-stream", &tool_stream(calls)),
    ];
    if final_reply {
        responses.push(http_response("text/event-stream", concat!(
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Admission results inspected.\"}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":7,\"output_tokens\":2}}}\n\n"
        )));
    }
    TestServer::start(responses)
}

fn provider(server: &TestServer) -> Arc<dyn ModelProvider> {
    Arc::new(OpenAiProvider::new(ProviderHttpConfig::new(&server.base_url).unwrap()).unwrap())
}

fn shell_quote(path: &std::path::Path) -> String {
    format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"))
}

#[test]
fn rejected_calls_never_start_or_execute_for_parallel_reads_and_mutations() {
    for behavior in [ToolBehavior::ReadOnly, ToolBehavior::StateChanging] {
        let (directory, store, session_id, profile_id, mut writer) = session();
        let forbidden = directory.path().join("forbidden-effect");
        let command = format!("printf forbidden > {}", shell_quote(&forbidden));
        let server = server(
            &[("blocked", &command), ("allowed", "printf allowed")],
            true,
        );
        let registry = registry(provider(&server), &profile_id);
        let mut request = request(vec![
            tool_definition("blocked", behavior),
            tool_definition("allowed", behavior),
        ]);
        let scope = accepted_scope(&mut writer, &mut request);
        let admission = StoreAdmission {
            scope,
            history: directory
                .path()
                .join("sessions")
                .join(session_id.to_string())
                .join("history.jsonl"),
        };
        let artifacts = tempfile::tempdir().unwrap();
        let spill = test_spill(artifacts.path());
        let events = Arc::new(Mutex::new(Vec::new()));
        let copy = Arc::clone(&events);
        let mut agent = AgentLoop::new(
            &registry,
            &profile_id,
            &credential,
            &CommandExecutor,
            &spill,
            &NoCompaction,
            &NoSteering,
            &mut writer,
            AgentLoopConfig::default(),
        )
        .with_tool_admission(&admission);
        agent.subscribe(move |event: &AgentEvent| {
            copy.lock().unwrap().push(event.clone());
        });
        let result = agent.run(request, &CancellationToken::default()).unwrap();
        assert_eq!(result.outcome, AgentOutcome::Completed);
        drop(agent);
        assert!(!forbidden.exists());
        let ancestry = writer.active_ancestry().unwrap();
        let results = ancestry
            .iter()
            .filter_map(|entry| match &entry.payload {
                SessionEntryPayload::ToolResult { failure, .. } => Some(failure),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(results.len(), 2);
        let denied = results.into_iter().flatten().next().unwrap();
        assert_eq!(denied.status, ToolFailureStatus::NotStarted);
        assert_eq!(denied.effect_state, ToolEffectState::NotStarted);
        assert_eq!(denied.error.code, "BINDING_TARGET_MISMATCH");
        let events = events.lock().unwrap();
        let started = events
            .iter()
            .filter_map(|event| match &event.kind {
                AgentEventKind::ToolStarted { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(started, ["allowed"]);
        assert_eq!(
            final_candidate_text(&store, &session_id, &result.final_candidate_id),
            "Admission results inspected."
        );
        drop(writer);
        let reopened = SessionStore::open(directory.path()).unwrap();
        assert_eq!(
            reopened.load_index(&session_id).unwrap().len(),
            ancestry.len()
        );
    }
}

#[test]
fn actual_admission_io_failure_preserves_started_results_and_marks_rest_not_started() {
    let (directory, _store, session_id, profile_id, mut writer) = session();
    let forbidden = directory.path().join("forbidden-effect");
    let command = format!("printf forbidden > {}", shell_quote(&forbidden));
    let server = server(
        &[
            ("allowed", "printf retained"),
            ("storage_fault", &command),
            ("later", &command),
        ],
        false,
    );
    let registry = registry(provider(&server), &profile_id);
    let mut request = request(
        ["allowed", "storage_fault", "later"]
            .into_iter()
            .map(|name| tool_definition(name, ToolBehavior::ReadOnly))
            .collect(),
    );
    let scope = accepted_scope(&mut writer, &mut request);
    let admission = StoreAdmission {
        scope,
        history: directory
            .path()
            .join("sessions")
            .join(session_id.to_string())
            .join("history.jsonl"),
    };
    let artifacts = tempfile::tempdir().unwrap();
    let spill = test_spill(artifacts.path());
    let events = Arc::new(Mutex::new(Vec::new()));
    let copy = Arc::clone(&events);
    let mut agent = AgentLoop::new(
        &registry,
        &profile_id,
        &credential,
        &CommandExecutor,
        &spill,
        &NoCompaction,
        &NoSteering,
        &mut writer,
        AgentLoopConfig::default(),
    )
    .with_tool_admission(&admission);
    agent.subscribe(move |event: &AgentEvent| {
        copy.lock().unwrap().push(event.clone());
    });
    assert!(matches!(
        agent.run(request, &CancellationToken::default()),
        Err(AgentLoopError::Session(_))
    ));
    drop(agent);
    assert!(!forbidden.exists());
    let ancestry = writer.active_ancestry().unwrap();
    let results = ancestry
        .iter()
        .filter_map(|entry| match &entry.payload {
            SessionEntryPayload::ToolResult {
                failure, content, ..
            } => Some((failure, content)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 3);
    assert_eq!(
        results
            .iter()
            .filter(|(failure, _)| failure.is_none())
            .count(),
        1
    );
    assert!(results.iter().any(|(failure, content)| failure.is_none()
        && *content
            == &vec![StoredContentBlock::Text {
                text: "retained".into()
            }]));
    for failure in results.iter().filter_map(|(failure, _)| failure.as_ref()) {
        assert_eq!(failure.status, ToolFailureStatus::NotStarted);
        assert_eq!(failure.effect_state, ToolEffectState::NotStarted);
    }
    let events = events.lock().unwrap();
    let started = events
        .iter()
        .filter_map(|event| match &event.kind {
            AgentEventKind::ToolStarted { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(started, ["allowed"]);
}

#[test]
fn every_parallel_dispatch_has_its_own_prior_durable_admission() {
    let (directory, _store, session_id, profile_id, mut writer) = session();
    let server = server(
        &[
            ("one", "printf one"),
            ("two", "printf two"),
            ("three", "printf three"),
        ],
        true,
    );
    let registry = registry(provider(&server), &profile_id);
    let mut request = request(
        ["one", "two", "three"]
            .into_iter()
            .map(|name| tool_definition(name, ToolBehavior::ReadOnly))
            .collect(),
    );
    let scope = accepted_scope(&mut writer, &mut request);
    let history = directory
        .path()
        .join("sessions")
        .join(session_id.to_string())
        .join("history.jsonl");
    let admission = StoreAdmission {
        scope,
        history: history.clone(),
    };
    let artifacts = tempfile::tempdir().unwrap();
    let spill = test_spill(artifacts.path());
    let dispatched = Arc::new(Mutex::new(Vec::new()));
    let copy = Arc::clone(&dispatched);
    let mut agent = AgentLoop::new(
        &registry,
        &profile_id,
        &credential,
        &CommandExecutor,
        &spill,
        &NoCompaction,
        &NoSteering,
        &mut writer,
        AgentLoopConfig {
            max_parallel_reads: 2,
            ..AgentLoopConfig::default()
        },
    )
    .with_tool_admission(&admission);
    agent.subscribe(move |event: &AgentEvent| {
        if let AgentEventKind::ToolStarted { call_id, .. } = &event.kind {
            let entries = fs::read_to_string(&history).unwrap().lines()
                .map(|line| serde_json::from_str::<keith_session_store::SessionEntry>(line).unwrap()).collect::<Vec<_>>();
            assert!(entries.iter().any(|entry| matches!(&entry.payload,
                SessionEntryPayload::BindingAdmission { admission } if &admission.call_id == call_id)));
            assert!(!entries.iter().any(|entry| matches!(&entry.payload,
                SessionEntryPayload::ToolResult { call_id: prior, .. } if prior == call_id)));
            copy.lock().unwrap().push(call_id.clone());
        }
    });
    agent.run(request, &CancellationToken::default()).unwrap();
    drop(agent);
    assert_eq!(dispatched.lock().unwrap().len(), 3);
    let entries = writer.active_ancestry().unwrap();
    assert_eq!(
        entries
            .iter()
            .filter(|entry| matches!(entry.payload, SessionEntryPayload::BindingAdmission { .. }))
            .count(),
        3
    );
}
