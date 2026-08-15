use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;

use keith_agent_types::{
    CURRENT_PROTOCOL_VERSION, ClientId, CommandId, EntityId, ProfileId, SessionId, UtcTimestamp,
};
use keith_protocol::{
    ClientCommand, CommandEnvelope, CommandResult, CreateSchedule, DaemonEvent, DeliveryPolicy,
    MessageRole, ReplyRoute, ResponsePayload, ScheduleExpression, SubmitPrompt, WireMessage,
};
use keith_ui_model::{ProjectionReducer, ReductionOutcome, VirtualizationConfig};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    CloseEvent, Document, Element, Event, HtmlFormElement, HtmlTextAreaElement, MessageEvent,
    WebSocket, XmlHttpRequest,
};

struct ClientApp {
    document: Document,
    csrf: String,
    profile: String,
    session: String,
    socket: Option<WebSocket>,
    reducer: Option<ProjectionReducer>,
    connection_epoch: u64,
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("document unavailable"))?;
    let root = document
        .get_element_by_id("app")
        .ok_or_else(|| JsValue::from_str("application root unavailable"))?;
    let csrf = document
        .query_selector("meta[name='keith-csrf']")?
        .and_then(|element| element.get_attribute("content"))
        .ok_or_else(|| JsValue::from_str("CSRF proof unavailable"))?;
    let app = Rc::new(RefCell::new(ClientApp {
        document,
        csrf,
        profile: root.get_attribute("data-profile").unwrap_or_default(),
        session: root.get_attribute("data-session").unwrap_or_default(),
        socket: None,
        reducer: None,
        connection_epoch: 0,
    }));
    bind_navigation(&app)?;
    bind_session_picker(&app)?;
    bind_composer(&app)?;
    bind_domain_forms(&app)?;
    bind_credential_submit(&app)?;
    show_surface(&app.borrow().document, "sessions")?;
    if app.borrow().session.is_empty() {
        set_status(&app.borrow().document, "No session selected");
    } else {
        connect_subscription(&app)?;
    }
    Ok(())
}

fn bind_navigation(app: &Rc<RefCell<ClientApp>>) -> Result<(), JsValue> {
    let buttons = app.borrow().document.query_selector_all("[data-route]")?;
    for index in 0..buttons.length() {
        let Some(node) = buttons.item(index) else {
            continue;
        };
        let element: Element = node.dyn_into()?;
        let document = app.borrow().document.clone();
        let selected = element.clone();
        let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            event.prevent_default();
            if let Some(route) = selected.get_attribute("data-route") {
                let _ = show_surface(&document, &route);
            }
        });
        element.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
        callback.forget();
    }
    Ok(())
}

fn bind_session_picker(app: &Rc<RefCell<ClientApp>>) -> Result<(), JsValue> {
    let buttons = app
        .borrow()
        .document
        .query_selector_all("[data-session][data-profile]")?;
    for index in 0..buttons.length() {
        let Some(node) = buttons.item(index) else {
            continue;
        };
        let element: Element = node.dyn_into()?;
        let selected = element.clone();
        let app = Rc::clone(app);
        let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            event.prevent_default();
            let Some(profile) = selected.get_attribute("data-profile") else {
                return;
            };
            let Some(session) = selected.get_attribute("data-session") else {
                return;
            };
            {
                let mut state = app.borrow_mut();
                state.profile.clone_from(&profile);
                state.session = session;
                state.reducer = None;
            }
            if let Some(form) = app.borrow().document.get_element_by_id("credential-form") {
                let _ =
                    form.set_attribute("action", &format!("/api/profiles/{profile}/credentials"));
            }
            let _ = connect_subscription(&app);
        });
        element.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
        callback.forget();
    }
    Ok(())
}

fn bind_composer(app: &Rc<RefCell<ClientApp>>) -> Result<(), JsValue> {
    let form: HtmlFormElement = app
        .borrow()
        .document
        .get_element_by_id("composer")
        .ok_or_else(|| JsValue::from_str("composer unavailable"))?
        .dyn_into()?;
    let app = Rc::clone(app);
    let form_for_callback = form.clone();
    let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
        event.prevent_default();
        let Ok(Some(input)) = form_for_callback.query_selector("#prompt") else {
            return;
        };
        let Ok(input) = input.dyn_into::<HtmlTextAreaElement>() else {
            return;
        };
        let text = input.value();
        if text.trim().is_empty() {
            return;
        }
        let state = app.borrow();
        let Ok(session_id) = state.session.parse::<SessionId>() else {
            return;
        };
        let command = ClientCommand::SubmitPrompt(SubmitPrompt {
            session_id: session_id.clone(),
            text,
            delivery: DeliveryPolicy::Immediate,
            reply_route: None,
        });
        if send_command(&app, Some(session_id), command).is_ok() {
            input.set_value("");
        }
    });
    form.add_event_listener_with_callback("submit", callback.as_ref().unchecked_ref())?;
    callback.forget();
    Ok(())
}

fn bind_domain_forms(app: &Rc<RefCell<ClientApp>>) -> Result<(), JsValue> {
    let forms = app
        .borrow()
        .document
        .query_selector_all("form.domain-command")?;
    for index in 0..forms.length() {
        let Some(node) = forms.item(index) else {
            continue;
        };
        let form: HtmlFormElement = node.dyn_into()?;
        form.set_attribute("data-rust-bound", "true")?;
        let button = form
            .query_selector("button[type='button']")?
            .ok_or_else(|| JsValue::from_str("domain action unavailable"))?;
        let kind = form.get_attribute("data-kind").unwrap_or_default();
        let app = Rc::clone(app);
        let form_for_callback = form.clone();
        let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            event.prevent_default();
            let Ok(Some(value)) = form_for_callback.query_selector("textarea[name='value']") else {
                return;
            };
            let Ok(value) = value.dyn_into::<HtmlTextAreaElement>() else {
                return;
            };
            let content = value.value();
            if content.trim().is_empty() {
                return;
            }
            let state = app.borrow();
            let Ok(profile_id) = state.profile.parse::<ProfileId>() else {
                return;
            };
            let Ok(session_id) = state.session.parse::<SessionId>() else {
                return;
            };
            drop(state);
            let command = domain_command(&kind, &profile_id, &session_id, content);
            if send_command(&app, Some(session_id), command).is_ok() {
                value.set_value("");
            } else {
                set_status(&app.borrow().document, "Command could not be sent");
            }
        });
        button.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
        callback.forget();
    }
    Ok(())
}

fn bind_credential_submit(app: &Rc<RefCell<ClientApp>>) -> Result<(), JsValue> {
    let Some(form) = app.borrow().document.get_element_by_id("credential-form") else {
        return Ok(());
    };
    let form: HtmlFormElement = form.dyn_into()?;
    let button = app
        .borrow()
        .document
        .get_element_by_id("save-credential")
        .ok_or_else(|| JsValue::from_str("credential action unavailable"))?;
    let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
        event.prevent_default();
        let _ = form.request_submit();
    });
    button.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
    callback.forget();
    Ok(())
}

fn domain_command(
    kind: &str,
    profile_id: &ProfileId,
    session_id: &SessionId,
    value: String,
) -> ClientCommand {
    match kind {
        "schedule" => ClientCommand::CreateSchedule(CreateSchedule {
            profile_id: profile_id.clone(),
            session_id: Some(session_id.clone()),
            expression: ScheduleExpression::IntervalSeconds(24 * 60 * 60),
            time_zone: "UTC".into(),
            prompt: value,
            reply_route: None,
        }),
        "channel" => ClientCommand::SubmitPrompt(SubmitPrompt {
            session_id: session_id.clone(),
            text: value,
            delivery: DeliveryPolicy::Immediate,
            reply_route: Some(ReplyRoute {
                channel: "configured-channel".into(),
                conversation: session_id.to_string(),
                thread: None,
            }),
        }),
        "memory" => ClientCommand::SubmitPrompt(SubmitPrompt {
            session_id: session_id.clone(),
            text: format!("Update durable memory with this user-authored content:\n{value}"),
            delivery: DeliveryPolicy::Immediate,
            reply_route: None,
        }),
        _ => ClientCommand::SubmitPrompt(SubmitPrompt {
            session_id: session_id.clone(),
            text: format!("Review and propose a guarded refinement for:\n{value}"),
            delivery: DeliveryPolicy::WhenIdle,
            reply_route: None,
        }),
    }
}

fn send_command(
    app: &Rc<RefCell<ClientApp>>,
    session_id: Option<SessionId>,
    command: ClientCommand,
) -> Result<(), JsValue> {
    let state = app.borrow();
    let profile = state.profile.clone();
    let envelope = CommandEnvelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        command_id: CommandId::from(browser_entity_id()?),
        client_id: ClientId::from(browser_entity_id()?),
        sent_at: browser_timestamp(),
        session_id,
        command,
    };
    let csrf = state.csrf.clone();
    drop(state);
    let request = XmlHttpRequest::new()?;
    request.open_with_async("POST", &format!("/api/profiles/{profile}/commands"), true)?;
    request.set_request_header("content-type", "application/json")?;
    request.set_request_header("x-keith-csrf", &csrf)?;
    let request_for_callback = request.clone();
    let app_for_callback = Rc::clone(app);
    let callback = Closure::<dyn FnMut(Event)>::new(move |_event: Event| {
        if request_for_callback.status().unwrap_or_default() == 200 {
            if let Ok(Some(response)) = request_for_callback.response_text()
                && let Ok(message) = serde_json::from_str::<WireMessage>(&response)
            {
                apply_wire_message(&app_for_callback, message);
            }
        } else {
            set_status(&app_for_callback.borrow().document, "Command was rejected");
        }
    });
    request.set_onload(Some(callback.as_ref().unchecked_ref()));
    callback.forget();
    request.send_with_opt_str(Some(&serde_json::to_string(&envelope).map_err(js_error)?))?;
    Ok(())
}

fn connect_subscription(app: &Rc<RefCell<ClientApp>>) -> Result<(), JsValue> {
    let mut state = app.borrow_mut();
    if let Some(socket) = state.socket.take() {
        let _ = socket.close();
    }
    state.connection_epoch = state.connection_epoch.saturating_add(1);
    let epoch = state.connection_epoch;
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
    let location = window.location();
    let scheme = if location.protocol()? == "https:" {
        "wss"
    } else {
        "ws"
    };
    let mut url = format!(
        "{scheme}://{}/api/events/{}/{}",
        location.host()?,
        state.profile,
        state.session
    );
    if let Some(reducer) = &state.reducer {
        let snapshot = reducer.snapshot();
        let _ = write!(
            url,
            "?generation={}&sequence={}",
            snapshot.generation.get(),
            snapshot.through_sequence.get()
        );
    }
    let socket = WebSocket::new(&url)?;
    let document = state.document.clone();
    let on_open = Closure::<dyn FnMut(Event)>::new(move |_event: Event| {
        set_status(&document, "Connected");
    });
    socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    on_open.forget();

    let app_for_message = Rc::clone(app);
    let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        if let Some(text) = event.data().as_string()
            && let Ok(message) = serde_json::from_str::<WireMessage>(&text)
        {
            apply_wire_message(&app_for_message, message);
        }
    });
    socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();

    let app_for_close = Rc::clone(app);
    let on_close = Closure::<dyn FnMut(CloseEvent)>::new(move |_event: CloseEvent| {
        let should_reconnect = app_for_close.borrow().connection_epoch == epoch;
        set_status(
            &app_for_close.borrow().document,
            "Disconnected; reconnecting",
        );
        if should_reconnect {
            let retry_app = Rc::clone(&app_for_close);
            let retry = Closure::<dyn FnMut()>::once(move || {
                if retry_app.borrow().connection_epoch == epoch {
                    let _ = connect_subscription(&retry_app);
                }
            });
            if let Some(window) = web_sys::window() {
                let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                    retry.as_ref().unchecked_ref(),
                    750,
                );
                retry.forget();
            }
        }
    });
    socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    on_close.forget();
    state.socket = Some(socket);
    set_status(&state.document, "Connecting");
    Ok(())
}

fn apply_wire_message(app: &Rc<RefCell<ClientApp>>, message: WireMessage) {
    let mut state = app.borrow_mut();
    match message {
        WireMessage::CommandResult(result) => match result.result {
            CommandResult::Data(payload) => match *payload {
                ResponsePayload::Snapshot(snapshot) => {
                    if let Some(reducer) = &mut state.reducer {
                        if reducer.apply_snapshot(*snapshot.clone()).is_err() {
                            state.reducer = new_reducer(*snapshot);
                        }
                    } else {
                        state.reducer = new_reducer(*snapshot);
                    }
                }
                ResponsePayload::Memory(results) => {
                    set_status(
                        &state.document,
                        &format!("Memory query returned {} results", results.len()),
                    );
                }
                _ => {}
            },
            CommandResult::Accepted { .. } => set_status(&state.document, "Command accepted"),
            CommandResult::Rejected(_) => set_status(&state.document, "Command rejected"),
        },
        WireMessage::Event(event) => {
            if let Some(reducer) = &mut state.reducer {
                match reducer.apply_event(&event) {
                    Ok(ReductionOutcome::Gap) | Err(_) => {
                        if let Some(socket) = state.socket.take() {
                            let _ = socket.close();
                        }
                    }
                    Ok(_) => {}
                }
            }
            if matches!(event.event, DaemonEvent::Warning(_) | DaemonEvent::Error(_)) {
                set_status(&state.document, "Agent reported a diagnostic event");
            }
        }
        WireMessage::ServerHello(_) => set_status(&state.document, "Connected"),
        WireMessage::ClientHello(_) | WireMessage::Command(_) => {}
    }
    render_projection(&state);
}

fn new_reducer(snapshot: keith_protocol::SessionSnapshot) -> Option<ProjectionReducer> {
    ProjectionReducer::new(snapshot, VirtualizationConfig::new(5_000, 240, 20).ok()?).ok()
}

fn render_projection(state: &ClientApp) {
    let Some(reducer) = &state.reducer else {
        return;
    };
    let snapshot = reducer.snapshot();
    if let Some(messages) = state.document.get_element_by_id("messages") {
        messages.set_text_content(None);
        let total = snapshot.messages.len();
        let window = reducer.history_window(total.saturating_sub(200), 200);
        for message in window.items {
            let Ok(item) = state.document.create_element("article") else {
                continue;
            };
            item.set_class_name("message");
            let role = match message.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
                MessageRole::System => "system",
            };
            let _ = item.set_attribute("data-role", role);
            let _ = item.set_attribute("aria-label", role);
            item.set_text_content(Some(&message.text));
            let _ = messages.append_child(&item);
        }
    }
    let entries = [
        (
            "sessions",
            format!("Session state: {:?}", snapshot.session.state),
        ),
        ("goals", format!("Goals: {}", snapshot.goals.len())),
        ("plans", format!("Plans: {}", snapshot.plans.len())),
        ("children", format!("Children: {}", snapshot.children.len())),
        ("tools", format!("Tools: {}", snapshot.tools.len())),
        (
            "knowledge",
            "Knowledge is indexed by the agent runtime".into(),
        ),
        (
            "commitments",
            format!("Commitments: {}", snapshot.commitments.len()),
        ),
        (
            "artifacts",
            format!("Deliveries: {}", snapshot.deliveries.len()),
        ),
    ];
    for (surface, text) in entries {
        if let Ok(Some(panel)) = state
            .document
            .query_selector(&format!("[data-panel='{surface}'] .projection"))
        {
            panel.set_text_content(Some(&text));
            panel.set_class_name("projection metric");
        }
    }
}

fn show_surface(document: &Document, route: &str) -> Result<(), JsValue> {
    let panels = document.query_selector_all("[data-panel]")?;
    for index in 0..panels.length() {
        let Some(node) = panels.item(index) else {
            continue;
        };
        let element: Element = node.dyn_into()?;
        let active = element.get_attribute("data-panel").as_deref() == Some(route);
        if active {
            element.remove_attribute("hidden")?;
        } else {
            element.set_attribute("hidden", "")?;
        }
    }
    let buttons = document.query_selector_all("[data-route]")?;
    for index in 0..buttons.length() {
        let Some(node) = buttons.item(index) else {
            continue;
        };
        let element: Element = node.dyn_into()?;
        if element.get_attribute("data-route").as_deref() == Some(route) {
            element.set_attribute("aria-current", "page")?;
        } else {
            element.remove_attribute("aria-current")?;
        }
    }
    Ok(())
}

fn set_status(document: &Document, status: &str) {
    if let Some(element) = document.get_element_by_id("connection-status") {
        element.set_text_content(Some(status));
    }
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

fn browser_entity_id() -> Result<EntityId, JsValue> {
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(js_error)?;
    let identifier = ulid::Ulid::from_parts(browser_millis(), u128::from_be_bytes(random));
    EntityId::parse(identifier.to_string()).map_err(js_error)
}

fn browser_timestamp() -> UtcTimestamp {
    let millis = i64::try_from(browser_millis()).unwrap_or(i64::MAX);
    UtcTimestamp::from_unix_millis(millis)
}

fn browser_millis() -> u64 {
    js_sys::Date::now()
        .round()
        .to_string()
        .parse()
        .unwrap_or_default()
}
