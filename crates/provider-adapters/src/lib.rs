#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use keith_agent_types::{EntityId, ToolCallId};
use keith_provider_core::{
    CancellationToken, ContentBlock, Message, MessageRole, ModelDescriptor, ModelEvent,
    ModelEventSink, ModelProvider, ModelRequest, ProviderCredential, ProviderError,
    ProviderErrorKind, StopReason, ToolBehavior, Usage, approximate_token_count,
    classify_http_status, emit, validate_request,
};
use serde_json::{Value, json};
use ureq::http::Response;
use ureq::{Agent, Error as HttpError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderHttpConfig {
    pub base_url: String,
    pub timeout: Duration,
    pub max_response_bytes: u64,
}

impl ProviderHttpConfig {
    /// # Errors
    ///
    /// Returns an invalid-request error for non-HTTPS non-loopback URLs or invalid limits.
    pub fn new(base_url: impl Into<String>) -> Result<Self, ProviderError> {
        let base_url = base_url.into().trim_end_matches('/').to_owned();
        let secure = base_url.starts_with("https://");
        let loopback = base_url.starts_with("http://127.0.0.1:")
            || base_url.starts_with("http://localhost:")
            || base_url.starts_with("http://[::1]:");
        if (!secure && !loopback) || base_url.len() > 2_048 {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "provider base URL must use HTTPS or an explicit loopback address",
            ));
        }
        Ok(Self {
            base_url,
            timeout: Duration::from_secs(120),
            max_response_bytes: 32 * 1_024 * 1_024,
        })
    }
}

struct HttpRuntime {
    config: ProviderHttpConfig,
    agent: Agent,
    active: Mutex<BTreeMap<EntityId, CancellationToken>>,
}

impl HttpRuntime {
    fn new(config: ProviderHttpConfig) -> Result<Self, ProviderError> {
        if config.timeout.is_zero() || config.max_response_bytes == 0 {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "provider timeout and response limit must be non-zero",
            ));
        }
        let https_only = config.base_url.starts_with("https://");
        let agent_config = Agent::config_builder()
            .timeout_global(Some(config.timeout))
            .https_only(https_only)
            .http_status_as_error(false)
            .build();
        Ok(Self {
            config,
            agent: Agent::new_with_config(agent_config),
            active: Mutex::new(BTreeMap::new()),
        })
    }

    fn register(
        &self,
        request_id: &EntityId,
        cancellation: &CancellationToken,
    ) -> Result<(), ProviderError> {
        let mut active = self.lock_active()?;
        if active
            .insert(request_id.clone(), cancellation.clone())
            .is_some()
        {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "provider request ID is already active",
            ));
        }
        Ok(())
    }

    fn unregister(&self, request_id: &EntityId) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(request_id);
        }
    }

    fn cancel(&self, request_id: &EntityId) -> Result<(), ProviderError> {
        let active = self.lock_active()?;
        let token = active.get(request_id).ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "provider request is not active",
            )
        })?;
        token.cancel();
        Ok(())
    }

    fn lock_active(
        &self,
    ) -> Result<MutexGuard<'_, BTreeMap<EntityId, CancellationToken>>, ProviderError> {
        self.active.lock().map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::Internal,
                "provider cancellation registry lock was poisoned",
            )
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.config.base_url)
    }
}

#[derive(Clone)]
pub struct OpenAiProvider {
    runtime: Arc<HttpRuntime>,
}

impl OpenAiProvider {
    /// # Errors
    ///
    /// Returns an error when the HTTP configuration is invalid.
    pub fn new(config: ProviderHttpConfig) -> Result<Self, ProviderError> {
        Ok(Self {
            runtime: Arc::new(HttpRuntime::new(config)?),
        })
    }

    fn stream_inner(
        &self,
        request: &ModelRequest,
        credential: &ProviderCredential,
        cancellation: &CancellationToken,
        sink: &mut dyn ModelEventSink,
    ) -> Result<Usage, ProviderError> {
        let body = openai_request(request)?;
        let authorization = format!("Bearer {}", credential.expose_utf8()?);
        let response = self
            .runtime
            .agent
            .post(self.runtime.url("/v1/chat/completions"))
            .header("authorization", &authorization)
            .header("content-type", "application/json")
            .send(&body)
            .map_err(map_http_error)?;
        check_status(response.status().as_u16())?;
        emit(
            sink,
            cancellation,
            ModelEvent::Started {
                provider_request_id: request_header(&response, "x-request-id"),
                model: request.model.clone(),
            },
        )?;
        parse_openai_stream(
            response,
            request,
            cancellation,
            sink,
            self.runtime.config.max_response_bytes,
        )
    }
}

impl ModelProvider for OpenAiProvider {
    fn provider_id(&self) -> &'static str {
        "openai"
    }

    fn list_models(
        &self,
        credential: &ProviderCredential,
    ) -> Result<Vec<ModelDescriptor>, ProviderError> {
        let authorization = format!("Bearer {}", credential.expose_utf8()?);
        let mut response = self
            .runtime
            .agent
            .get(self.runtime.url("/v1/models"))
            .header("authorization", &authorization)
            .call()
            .map_err(map_http_error)?;
        check_status(response.status().as_u16())?;
        let bytes = read_bounded(&mut response, self.runtime.config.max_response_bytes)?;
        let value: Value = serde_json::from_slice(&bytes).map_err(malformed)?;
        model_list(&value, self.provider_id())
    }

    fn stream(
        &self,
        request: &ModelRequest,
        credential: &ProviderCredential,
        cancellation: &CancellationToken,
        sink: &mut dyn ModelEventSink,
    ) -> Result<Usage, ProviderError> {
        validate_request(request)?;
        cancellation.check()?;
        self.runtime.register(&request.request_id, cancellation)?;
        let result = self.stream_inner(request, credential, cancellation, sink);
        self.runtime.unregister(&request.request_id);
        result
    }

    fn count_tokens(
        &self,
        request: &ModelRequest,
        _credential: &ProviderCredential,
    ) -> Result<u64, ProviderError> {
        validate_request(request)?;
        approximate_token_count(request)
    }

    fn cancel(&self, request_id: &EntityId) -> Result<(), ProviderError> {
        self.runtime.cancel(request_id)
    }
}

#[derive(Clone)]
pub struct AnthropicProvider {
    runtime: Arc<HttpRuntime>,
    api_version: String,
}

impl AnthropicProvider {
    /// # Errors
    ///
    /// Returns an error when the HTTP configuration is invalid.
    pub fn new(config: ProviderHttpConfig) -> Result<Self, ProviderError> {
        Ok(Self {
            runtime: Arc::new(HttpRuntime::new(config)?),
            api_version: "2023-06-01".into(),
        })
    }

    fn stream_inner(
        &self,
        request: &ModelRequest,
        credential: &ProviderCredential,
        cancellation: &CancellationToken,
        sink: &mut dyn ModelEventSink,
    ) -> Result<Usage, ProviderError> {
        let body = anthropic_request(request)?;
        let response = self
            .runtime
            .agent
            .post(self.runtime.url("/v1/messages"))
            .header("x-api-key", credential.expose_utf8()?)
            .header("anthropic-version", &self.api_version)
            .header("content-type", "application/json")
            .send(&body)
            .map_err(map_http_error)?;
        check_status(response.status().as_u16())?;
        emit(
            sink,
            cancellation,
            ModelEvent::Started {
                provider_request_id: request_header(&response, "request-id"),
                model: request.model.clone(),
            },
        )?;
        parse_anthropic_stream(
            response,
            request,
            cancellation,
            sink,
            self.runtime.config.max_response_bytes,
        )
    }
}

impl ModelProvider for AnthropicProvider {
    fn provider_id(&self) -> &'static str {
        "anthropic"
    }

    fn list_models(
        &self,
        credential: &ProviderCredential,
    ) -> Result<Vec<ModelDescriptor>, ProviderError> {
        let mut response = self
            .runtime
            .agent
            .get(self.runtime.url("/v1/models"))
            .header("x-api-key", credential.expose_utf8()?)
            .header("anthropic-version", &self.api_version)
            .call()
            .map_err(map_http_error)?;
        check_status(response.status().as_u16())?;
        let bytes = read_bounded(&mut response, self.runtime.config.max_response_bytes)?;
        let value: Value = serde_json::from_slice(&bytes).map_err(malformed)?;
        model_list(&value, self.provider_id())
    }

    fn stream(
        &self,
        request: &ModelRequest,
        credential: &ProviderCredential,
        cancellation: &CancellationToken,
        sink: &mut dyn ModelEventSink,
    ) -> Result<Usage, ProviderError> {
        validate_request(request)?;
        cancellation.check()?;
        self.runtime.register(&request.request_id, cancellation)?;
        let result = self.stream_inner(request, credential, cancellation, sink);
        self.runtime.unregister(&request.request_id);
        result
    }

    fn count_tokens(
        &self,
        request: &ModelRequest,
        _credential: &ProviderCredential,
    ) -> Result<u64, ProviderError> {
        validate_request(request)?;
        approximate_token_count(request)
    }

    fn cancel(&self, request_id: &EntityId) -> Result<(), ProviderError> {
        self.runtime.cancel(request_id)
    }
}

fn openai_request(request: &ModelRequest) -> Result<Vec<u8>, ProviderError> {
    let mut messages = Vec::new();
    if !request.system.is_empty() {
        messages.push(json!({
            "role": "system",
            "content": openai_content(&request.system),
        }));
    }
    for message in &request.messages {
        messages.extend(openai_messages(message));
    }
    let tools = request
        .tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema,
                    "x-keith-behavior": match tool.behavior {
                        ToolBehavior::ReadOnly => "read_only",
                        ToolBehavior::StateChanging => "state_changing",
                    },
                }
            })
        })
        .collect::<Vec<_>>();
    let mut body = json!({
        "model": request.model,
        "messages": messages,
        "tools": tools,
        "stream": true,
        "stream_options": {"include_usage": true},
    });
    if let Some(max_tokens) = request.max_output_tokens {
        body["max_completion_tokens"] = json!(max_tokens);
    }
    if let Some(temperature) = request.temperature {
        body["temperature"] = json!(temperature);
    }
    if let Some(effort) = &request.reasoning_effort {
        body["reasoning_effort"] = json!(effort);
    }
    serde_json::to_vec(&body).map_err(internal)
}

fn anthropic_request(request: &ModelRequest) -> Result<Vec<u8>, ProviderError> {
    let system = anthropic_content(&request.system);
    let messages = request
        .messages
        .iter()
        .map(anthropic_message)
        .collect::<Vec<_>>();
    let tools = request
        .tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.input_schema,
            })
        })
        .collect::<Vec<_>>();
    let body = json!({
        "model": request.model,
        "system": system,
        "messages": messages,
        "tools": tools,
        "stream": true,
        "max_tokens": request.max_output_tokens.unwrap_or(4096),
        "temperature": request.temperature,
    });
    serde_json::to_vec(&body).map_err(internal)
}

fn openai_messages(message: &Message) -> Vec<Value> {
    let role = match message.role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    };
    let mut result = Vec::new();
    let regular = message
        .content
        .iter()
        .filter(|block| !matches!(block, ContentBlock::ToolResult { .. }))
        .cloned()
        .collect::<Vec<_>>();
    if !regular.is_empty() {
        let tool_calls = regular
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolCall {
                    id,
                    name,
                    arguments,
                } => Some(json!({
                    "id": id.to_string(),
                    "type": "function",
                    "function": {"name": name, "arguments": arguments.to_string()},
                })),
                ContentBlock::Text { .. }
                | ContentBlock::Image { .. }
                | ContentBlock::ToolResult { .. } => None,
            })
            .collect::<Vec<_>>();
        let content = openai_content(&regular);
        result.push(json!({"role": role, "content": content, "tool_calls": tool_calls}));
    }
    for block in &message.content {
        if let ContentBlock::ToolResult {
            call_id, content, ..
        } = block
        {
            result.push(json!({
                "role": "tool",
                "tool_call_id": call_id.to_string(),
                "content": content,
            }));
        }
    }
    result
}

fn openai_content(content: &[ContentBlock]) -> Value {
    let parts = content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(json!({"type": "text", "text": text})),
            ContentBlock::Image { media_type, data } => Some(json!({
                "type": "image_url",
                "image_url": {"url": format!("data:{media_type};base64,{data}")}
            })),
            ContentBlock::ToolCall { .. } | ContentBlock::ToolResult { .. } => None,
        })
        .collect::<Vec<_>>();
    Value::Array(parts)
}

fn anthropic_message(message: &Message) -> Value {
    let role = if message.role == MessageRole::Assistant {
        "assistant"
    } else {
        "user"
    };
    json!({"role": role, "content": anthropic_content(&message.content)})
}

fn anthropic_content(content: &[ContentBlock]) -> Value {
    Value::Array(
        content
            .iter()
            .map(|block| match block {
                ContentBlock::Text { text } => json!({"type": "text", "text": text}),
                ContentBlock::Image { media_type, data } => json!({
                    "type": "image",
                    "source": {"type": "base64", "media_type": media_type, "data": data}
                }),
                ContentBlock::ToolCall {
                    id,
                    name,
                    arguments,
                } => json!({
                    "type": "tool_use",
                    "id": id.to_string(),
                    "name": name,
                    "input": arguments,
                }),
                ContentBlock::ToolResult {
                    call_id,
                    content,
                    is_error,
                } => json!({
                    "type": "tool_result",
                    "tool_use_id": call_id.to_string(),
                    "content": content,
                    "is_error": is_error,
                }),
            })
            .collect(),
    )
}

fn parse_openai_stream(
    mut response: Response<ureq::Body>,
    _request: &ModelRequest,
    cancellation: &CancellationToken,
    sink: &mut dyn ModelEventSink,
    max_bytes: u64,
) -> Result<Usage, ProviderError> {
    let mut usage = Usage::default();
    let mut calls = BTreeMap::<usize, (ToolCallId, String, String)>::new();
    let mut finished = false;
    read_sse(&mut response, max_bytes, |data| {
        handle_openai_event(
            data,
            &mut usage,
            &mut calls,
            &mut finished,
            cancellation,
            sink,
        )
    })?;
    if !calls.is_empty() {
        finish_calls(&mut calls, cancellation, sink)?;
    }
    if !finished {
        emit(
            sink,
            cancellation,
            ModelEvent::Finished {
                reason: StopReason::Other,
            },
        )?;
    }
    Ok(usage)
}

fn parse_anthropic_stream(
    mut response: Response<ureq::Body>,
    _request: &ModelRequest,
    cancellation: &CancellationToken,
    sink: &mut dyn ModelEventSink,
    max_bytes: u64,
) -> Result<Usage, ProviderError> {
    let mut usage = Usage::default();
    let mut calls = BTreeMap::<usize, (ToolCallId, String, String)>::new();
    let mut finished = false;
    read_sse(&mut response, max_bytes, |data| {
        handle_anthropic_event(
            data,
            &mut usage,
            &mut calls,
            &mut finished,
            cancellation,
            sink,
        )
    })?;
    for (_, (id, name, arguments)) in calls {
        emit_completed_call(id, name, &arguments, cancellation, sink)?;
    }
    if !finished {
        emit(
            sink,
            cancellation,
            ModelEvent::Finished {
                reason: StopReason::Other,
            },
        )?;
    }
    Ok(usage)
}

fn handle_openai_event(
    data: &str,
    usage: &mut Usage,
    calls: &mut BTreeMap<usize, (ToolCallId, String, String)>,
    finished: &mut bool,
    cancellation: &CancellationToken,
    sink: &mut dyn ModelEventSink,
) -> Result<bool, ProviderError> {
    cancellation.check()?;
    if data == "[DONE]" {
        return Ok(false);
    }
    let value: Value = serde_json::from_str(data).map_err(malformed)?;
    if let Some(raw_usage) = value.get("usage").filter(|value| !value.is_null()) {
        *usage = usage_from_openai(raw_usage);
        emit(sink, cancellation, ModelEvent::Usage { usage: *usage })?;
    }
    let Some(choice) = value["choices"]
        .as_array()
        .and_then(|values| values.first())
    else {
        return Ok(true);
    };
    let delta = &choice["delta"];
    if let Some(text) = delta["content"].as_str() {
        emit(
            sink,
            cancellation,
            ModelEvent::TextDelta { text: text.into() },
        )?;
    }
    if let Some(text) = delta["reasoning_content"].as_str() {
        emit(
            sink,
            cancellation,
            ModelEvent::ReasoningDelta { text: text.into() },
        )?;
    }
    if let Some(tool_calls) = delta["tool_calls"].as_array() {
        handle_openai_tools(tool_calls, calls, cancellation, sink)?;
    }
    if let Some(reason) = choice["finish_reason"].as_str() {
        if reason == "tool_calls" {
            finish_calls(calls, cancellation, sink)?;
        }
        emit(
            sink,
            cancellation,
            ModelEvent::Finished {
                reason: openai_stop_reason(reason),
            },
        )?;
        *finished = true;
    }
    Ok(true)
}

fn handle_openai_tools(
    tool_calls: &[Value],
    calls: &mut BTreeMap<usize, (ToolCallId, String, String)>,
    cancellation: &CancellationToken,
    sink: &mut dyn ModelEventSink,
) -> Result<(), ProviderError> {
    for tool in tool_calls {
        let index = stream_index(tool);
        let name = tool["function"]["name"].as_str().unwrap_or_default();
        match calls.entry(index) {
            Entry::Vacant(slot) => {
                let id = ToolCallId::new();
                slot.insert((id.clone(), name.into(), String::new()));
                emit(
                    sink,
                    cancellation,
                    ModelEvent::ToolCallStarted {
                        id,
                        name: name.into(),
                    },
                )?;
            }
            Entry::Occupied(mut slot) if !name.is_empty() => {
                slot.get_mut().1 = name.into();
            }
            Entry::Occupied(_) => {}
        }
        if let Some(arguments) = tool["function"]["arguments"].as_str()
            && !arguments.is_empty()
        {
            let (id, _, assembled) = calls.get_mut(&index).ok_or_else(|| {
                ProviderError::new(
                    ProviderErrorKind::MalformedResponse,
                    "tool call index disappeared during assembly",
                )
            })?;
            assembled.push_str(arguments);
            emit(
                sink,
                cancellation,
                ModelEvent::ToolCallArgumentsDelta {
                    id: id.clone(),
                    delta: arguments.into(),
                },
            )?;
        }
    }
    Ok(())
}

fn handle_anthropic_event(
    data: &str,
    usage: &mut Usage,
    calls: &mut BTreeMap<usize, (ToolCallId, String, String)>,
    finished: &mut bool,
    cancellation: &CancellationToken,
    sink: &mut dyn ModelEventSink,
) -> Result<bool, ProviderError> {
    cancellation.check()?;
    let value: Value = serde_json::from_str(data).map_err(malformed)?;
    match value["type"].as_str().unwrap_or_default() {
        "message_start" => {
            usage.input_tokens = value["message"]["usage"]["input_tokens"]
                .as_u64()
                .unwrap_or(0);
        }
        "content_block_start" => start_anthropic_block(&value, calls, cancellation, sink)?,
        "content_block_delta" => {
            handle_anthropic_delta(&value, calls, cancellation, sink)?;
        }
        "content_block_stop" => {
            let index = stream_index(&value);
            if let Some((id, name, arguments)) = calls.remove(&index) {
                emit_completed_call(id, name, &arguments, cancellation, sink)?;
            }
        }
        "message_delta" => {
            usage.output_tokens = value["usage"]["output_tokens"].as_u64().unwrap_or(0);
            emit(sink, cancellation, ModelEvent::Usage { usage: *usage })?;
            if let Some(reason) = value["delta"]["stop_reason"].as_str() {
                emit(
                    sink,
                    cancellation,
                    ModelEvent::Finished {
                        reason: anthropic_stop_reason(reason),
                    },
                )?;
                *finished = true;
            }
        }
        "error" => {
            return Err(ProviderError::new(
                ProviderErrorKind::Unavailable,
                value["error"]["message"]
                    .as_str()
                    .unwrap_or("provider stream failed"),
            ));
        }
        _ => {}
    }
    Ok(true)
}

fn start_anthropic_block(
    value: &Value,
    calls: &mut BTreeMap<usize, (ToolCallId, String, String)>,
    cancellation: &CancellationToken,
    sink: &mut dyn ModelEventSink,
) -> Result<(), ProviderError> {
    if value["content_block"]["type"] == "tool_use" {
        let index = stream_index(value);
        let id = ToolCallId::new();
        let name = value["content_block"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        calls.insert(index, (id.clone(), name.clone(), String::new()));
        emit(sink, cancellation, ModelEvent::ToolCallStarted { id, name })?;
    }
    Ok(())
}

fn handle_anthropic_delta(
    value: &Value,
    calls: &mut BTreeMap<usize, (ToolCallId, String, String)>,
    cancellation: &CancellationToken,
    sink: &mut dyn ModelEventSink,
) -> Result<(), ProviderError> {
    match value["delta"]["type"].as_str().unwrap_or_default() {
        "text_delta" => {
            if let Some(text) = value["delta"]["text"].as_str() {
                emit(
                    sink,
                    cancellation,
                    ModelEvent::TextDelta { text: text.into() },
                )?;
            }
        }
        "thinking_delta" => {
            if let Some(text) = value["delta"]["thinking"].as_str() {
                emit(
                    sink,
                    cancellation,
                    ModelEvent::ReasoningDelta { text: text.into() },
                )?;
            }
        }
        "input_json_delta" => {
            let index = stream_index(value);
            let delta = value["delta"]["partial_json"].as_str().unwrap_or_default();
            let (id, _, assembled) = calls.get_mut(&index).ok_or_else(|| {
                ProviderError::new(
                    ProviderErrorKind::MalformedResponse,
                    "tool arguments arrived before tool start",
                )
            })?;
            assembled.push_str(delta);
            emit(
                sink,
                cancellation,
                ModelEvent::ToolCallArgumentsDelta {
                    id: id.clone(),
                    delta: delta.into(),
                },
            )?;
        }
        _ => {}
    }
    Ok(())
}

fn read_sse(
    response: &mut Response<ureq::Body>,
    max_bytes: u64,
    mut consume: impl FnMut(&str) -> Result<bool, ProviderError>,
) -> Result<(), ProviderError> {
    let mut reader = BufReader::new(response.body_mut().as_reader());
    let mut total = 0_u64;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).map_err(transport_io)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).map_err(|_| response_too_large())?)
            .ok_or_else(response_too_large)?;
        if total > max_bytes {
            return Err(response_too_large());
        }
        if let Some(data) = line.strip_prefix("data:")
            && !consume(data.trim())?
        {
            break;
        }
    }
    Ok(())
}

fn finish_calls(
    calls: &mut BTreeMap<usize, (ToolCallId, String, String)>,
    cancellation: &CancellationToken,
    sink: &mut dyn ModelEventSink,
) -> Result<(), ProviderError> {
    let drained = std::mem::take(calls);
    for (_, (id, name, arguments)) in drained {
        emit_completed_call(id, name, &arguments, cancellation, sink)?;
    }
    Ok(())
}

fn emit_completed_call(
    id: ToolCallId,
    name: String,
    arguments: &str,
    cancellation: &CancellationToken,
    sink: &mut dyn ModelEventSink,
) -> Result<(), ProviderError> {
    let arguments = if arguments.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(arguments).map_err(malformed)?
    };
    emit(
        sink,
        cancellation,
        ModelEvent::ToolCallCompleted {
            id,
            name,
            arguments,
        },
    )
}

fn model_list(value: &Value, provider: &str) -> Result<Vec<ModelDescriptor>, ProviderError> {
    let data = value["data"].as_array().ok_or_else(|| {
        ProviderError::new(
            ProviderErrorKind::MalformedResponse,
            "model discovery response has no data array",
        )
    })?;
    let mut models = data
        .iter()
        .filter_map(|model| model["id"].as_str())
        .map(|id| ModelDescriptor {
            provider: provider.into(),
            id: id.into(),
            display_name: id.into(),
            context_tokens: None,
            output_tokens: None,
            supports_tools: true,
            supports_reasoning: true,
            supports_vision: true,
        })
        .collect::<Vec<_>>();
    models.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(models)
}

fn usage_from_openai(value: &Value) -> Usage {
    Usage {
        input_tokens: value["prompt_tokens"].as_u64().unwrap_or(0),
        output_tokens: value["completion_tokens"].as_u64().unwrap_or(0),
        cached_input_tokens: value["prompt_tokens_details"]["cached_tokens"]
            .as_u64()
            .unwrap_or(0),
    }
}

fn openai_stop_reason(reason: &str) -> StopReason {
    match reason {
        "stop" => StopReason::EndTurn,
        "tool_calls" | "function_call" => StopReason::ToolUse,
        "length" => StopReason::MaxTokens,
        "content_filter" => StopReason::ContentRejected,
        _ => StopReason::Other,
    }
}

fn anthropic_stop_reason(reason: &str) -> StopReason {
    match reason {
        "end_turn" | "stop_sequence" => StopReason::EndTurn,
        "tool_use" => StopReason::ToolUse,
        "max_tokens" => StopReason::MaxTokens,
        _ => StopReason::Other,
    }
}

fn stream_index(value: &Value) -> usize {
    value["index"]
        .as_u64()
        .and_then(|index| usize::try_from(index).ok())
        .unwrap_or(0)
}

fn read_bounded(
    response: &mut Response<ureq::Body>,
    max_bytes: u64,
) -> Result<Vec<u8>, ProviderError> {
    response
        .body_mut()
        .with_config()
        .limit(max_bytes)
        .read_to_vec()
        .map_err(map_http_error)
}

fn request_header(response: &Response<ureq::Body>, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn check_status(status: u16) -> Result<(), ProviderError> {
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(classify_http_status(
            status,
            format!("provider returned HTTP status {status}"),
        ))
    }
}

fn map_http_error(error: HttpError) -> ProviderError {
    match error {
        HttpError::StatusCode(status) => {
            classify_http_status(status, format!("provider returned HTTP status {status}"))
        }
        HttpError::Timeout(_) => {
            ProviderError::new(ProviderErrorKind::Timeout, "provider request timed out")
        }
        HttpError::Io(_) | HttpError::ConnectionFailed | HttpError::HostNotFound => {
            ProviderError::new(
                ProviderErrorKind::Unavailable,
                "provider transport is unavailable",
            )
        }
        other => ProviderError::new(
            ProviderErrorKind::Internal,
            format!("provider HTTP client failed: {other}"),
        ),
    }
}

fn transport_io(_error: std::io::Error) -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::Unavailable,
        "provider stream transport failed",
    )
}

fn malformed(error: impl std::fmt::Display) -> ProviderError {
    ProviderError::new(ProviderErrorKind::MalformedResponse, error.to_string())
}

fn internal(error: impl std::fmt::Display) -> ProviderError {
    ProviderError::new(ProviderErrorKind::Internal, error.to_string())
}

fn response_too_large() -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::MalformedResponse,
        "provider response exceeded its configured byte limit",
    )
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc::{self, Receiver};
    use std::thread;

    use keith_provider_core::StreamControl;

    use super::*;

    struct TestServer {
        base_url: String,
        requests: Receiver<String>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl TestServer {
        fn start(responses: Vec<String>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let (sender, requests) = mpsc::channel();
            let thread = thread::spawn(move || {
                for response in responses {
                    let (mut stream, _) = listener.accept().unwrap();
                    let request = read_request(&mut stream);
                    sender.send(request).unwrap();
                    stream.write_all(response.as_bytes()).unwrap();
                    stream.flush().unwrap();
                }
            });
            Self {
                base_url: format!("http://{address}"),
                requests,
                thread: Some(thread),
            }
        }

        fn request(&self) -> String {
            self.requests.recv_timeout(Duration::from_secs(5)).unwrap()
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            if let Some(thread) = self.thread.take() {
                thread.join().unwrap();
            }
        }
    }

    fn read_request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..header_end + 4]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if bytes.len() >= header_end + 4 + length {
                    break;
                }
            }
        }
        String::from_utf8(bytes).unwrap()
    }

    fn response(content_type: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn request() -> ModelRequest {
        ModelRequest {
            request_id: EntityId::new(),
            model: "model-a".into(),
            system: vec![ContentBlock::Text {
                text: "system".into(),
            }],
            messages: vec![Message {
                role: MessageRole::User,
                content: vec![ContentBlock::Text {
                    text: "hello".into(),
                }],
            }],
            tools: vec![keith_provider_core::ToolDefinition {
                name: "lookup".into(),
                description: "look up a value".into(),
                input_schema: json!({"type": "object", "properties": {}}),
                behavior: ToolBehavior::ReadOnly,
            }],
            max_output_tokens: Some(100),
            temperature: None,
            reasoning_effort: None,
        }
    }

    #[test]
    fn openai_adapter_discovers_streams_tools_usage_and_scopes_secret_to_header() {
        let model_body = r#"{"data":[{"id":"model-a"}]}"#;
        let stream_body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_x\",\"function\":{\"name\":\"lookup\",\"arguments\":\"{\\\"q\\\":1}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":3}}\n\n",
            "data: [DONE]\n\n"
        );
        let server = TestServer::start(vec![
            response("application/json", model_body),
            response("text/event-stream", stream_body),
        ]);
        let provider =
            OpenAiProvider::new(ProviderHttpConfig::new(&server.base_url).unwrap()).unwrap();
        let credential = ProviderCredential::new("openai-secret").unwrap();
        assert_eq!(provider.list_models(&credential).unwrap()[0].id, "model-a");
        let first_request = server.request();
        assert!(first_request.contains("authorization: Bearer openai-secret"));
        let mut events = Vec::new();
        let mut sink = |event| {
            events.push(event);
            Ok(StreamControl::Continue)
        };
        let usage = provider
            .stream(
                &request(),
                &credential,
                &CancellationToken::default(),
                &mut sink,
            )
            .unwrap();
        let second_request = server.request();
        let body = second_request.split("\r\n\r\n").nth(1).unwrap();
        assert!(!body.contains("openai-secret"));
        assert_eq!(usage.total_tokens(), 8);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, ModelEvent::ToolCallCompleted { .. }))
        );
    }

    #[test]
    fn anthropic_adapter_discovers_and_normalizes_text_reasoning_tools_and_usage() {
        let model_body = r#"{"data":[{"id":"claude-a"}]}"#;
        let stream_body = concat!(
            "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":7}}}\n\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n",
            "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tool_x\",\"name\":\"lookup\"}}\n\n",
            "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"q\\\":2}\"}}\n\n",
            "data: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":4}}\n\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        );
        let server = TestServer::start(vec![
            response("application/json", model_body),
            response("text/event-stream", stream_body),
        ]);
        let provider =
            AnthropicProvider::new(ProviderHttpConfig::new(&server.base_url).unwrap()).unwrap();
        let credential = ProviderCredential::new("anthropic-secret").unwrap();
        assert_eq!(provider.list_models(&credential).unwrap()[0].id, "claude-a");
        assert!(server.request().contains("x-api-key: anthropic-secret"));
        let mut events = Vec::new();
        let mut sink = |event| {
            events.push(event);
            Ok(StreamControl::Continue)
        };
        let mut normalized_request = request();
        normalized_request.model = "claude-a".into();
        let usage = provider
            .stream(
                &normalized_request,
                &credential,
                &CancellationToken::default(),
                &mut sink,
            )
            .unwrap();
        assert_eq!(usage.total_tokens(), 11);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, ModelEvent::TextDelta { text } if text == "hi"))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, ModelEvent::ToolCallCompleted { .. }))
        );
    }

    #[test]
    fn status_and_cancellation_are_typed_without_secret_leakage() {
        let server = TestServer::start(vec![
            "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
        ]);
        let provider =
            OpenAiProvider::new(ProviderHttpConfig::new(&server.base_url).unwrap()).unwrap();
        let credential = ProviderCredential::new("never-log-me").unwrap();
        let error = provider.list_models(&credential).unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::Authentication);
        assert!(!error.to_string().contains("never-log-me"));
        let token = CancellationToken::default();
        token.cancel();
        let mut sink = |_event| Ok(StreamControl::Continue);
        let error = provider
            .stream(&request(), &credential, &token, &mut sink)
            .unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::Cancelled);
        let _ = server.request();
    }
}
