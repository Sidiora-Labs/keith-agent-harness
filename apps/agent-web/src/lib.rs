#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod client;
#[cfg(not(target_arch = "wasm32"))]
mod security;
#[cfg(not(target_arch = "wasm32"))]
mod server;

#[cfg(not(target_arch = "wasm32"))]
pub use server::{ServerArguments, ServerError, WebServer, WebServerConfig};

use std::fmt::Write as _;

use keith_protocol::{ProfileSummary, SessionSummary};
use keith_ui_model::{ClientParity, OperatorCommand, OperatorSurface};

pub use keith_ui_model::OperatorSurface as Surface;

pub fn client_parity() -> ClientParity {
    ClientParity {
        surfaces: OperatorSurface::ALL.into_iter().collect(),
        commands: OperatorCommand::ALL.into_iter().collect(),
    }
}

pub fn login_page() -> &'static str {
    r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Keith Agent sign in</title><link rel="stylesheet" href="/assets/app.css"></head>
<body><main class="login"><h1>Keith Agent</h1><p>Sign in to the local control plane.</p>
<form method="post" action="/auth/session"><label for="password">Access secret</label>
<input id="password" name="password" type="password" autocomplete="current-password" required maxlength="4096">
<button type="submit">Sign in</button></form></main></body></html>"#
}

pub fn shell_page(csrf: &str, profiles: &[ProfileSummary], sessions: &[SessionSummary]) -> String {
    let mut navigation = String::new();
    for surface in Surface::ALL {
        let _ = write!(
            navigation,
            "<button id=\"nav-{}\" type=\"button\" data-route=\"{}\" aria-controls=\"panel-{}\">{}</button>",
            surface.route(),
            surface.route(),
            surface.route(),
            surface.label()
        );
    }
    let mut session_list = String::new();
    for session in sessions {
        let title = session.title.as_deref().unwrap_or("Untitled session");
        let _ = write!(
            session_list,
            "<button type=\"button\" class=\"session\" data-session=\"{}\" data-profile=\"{}\" aria-label=\"Open session {}\">{}</button>",
            session.session_id,
            session.profile_id,
            escape_html(title),
            escape_html(title)
        );
    }
    if session_list.is_empty() {
        session_list.push_str("<p data-empty-sessions>No sessions are available.</p>");
    }
    let initial_profile = sessions
        .first()
        .map(|session| session.profile_id.to_string())
        .or_else(|| profiles.first().map(|profile| profile.id.to_string()))
        .unwrap_or_default();
    let initial_session = sessions
        .first()
        .map(|session| session.session_id.to_string())
        .unwrap_or_default();
    let mut panels = String::new();
    for surface in Surface::ALL {
        panels.push_str(&surface_panel(surface, &initial_profile, csrf));
    }
    format!(
        r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="keith-csrf" content="{csrf}"><title>Keith Agent</title><link rel="stylesheet" href="/assets/app.css"></head>
<body><a class="skip-link" href="#conversation">Skip to conversation</a><div id="app" data-profile="{initial_profile}" data-session="{initial_session}">
<header><h1>Keith Agent</h1><div class="status-group"><p id="connection-status" role="status" aria-live="polite">Connection not started</p><p id="presence-status" role="status" aria-live="polite">No authoritative activity</p></div></header>
<nav aria-label="Workspace">{navigation}</nav>
<main class="workspace"><section id="conversation" aria-labelledby="conversation-heading" tabindex="-1"><h2 id="conversation-heading">Conversation</h2>
<div id="messages" role="log" aria-live="polite" aria-relevant="additions text" aria-atomic="false"></div>
<form id="composer"><label for="prompt">Message</label><p id="prompt-help">Enter sends a prompt. Use the controls for steering and session actions.</p><textarea id="prompt" aria-describedby="prompt-help" maxlength="65536" rows="3"></textarea>
<button type="submit">Send</button></form></section>
<section class="command-bar" aria-label="Conversation controls"><button type="button" data-command="steer">Steer</button><button type="button" data-command="cancel">Cancel</button><button type="button" data-command="retry">Retry</button><button type="button" data-command="branch">Branch</button><button type="button" data-command="resume">Resume</button><button type="button" data-command="list-sessions">Refresh sessions</button></section>
<aside id="surface" aria-label="Details">{panels}</aside></main>
<section id="session-drawer" aria-label="Session picker"><h2>Sessions</h2>{session_list}</section>
</div><script type="module" src="/assets/bootstrap.js"></script></body></html>"##,
        csrf = escape_html(csrf),
    )
}

pub const APP_CSS: &str = r#"
:root { color-scheme: light dark; font-family: system-ui, sans-serif; background: #101820; color: #ecf2f8; line-height: 1.5; }
* { box-sizing: border-box; }
body { margin: 0; background: #101820; color: #ecf2f8; }
button, input, textarea, select { min-block-size: 2.75rem; font: inherit; color: inherit; background: #243747; padding: .65rem .8rem; }
button { cursor: pointer; }
button:hover, button[aria-current="page"] { background: #31556b; }
button:focus-visible, input:focus-visible, textarea:focus-visible, select:focus-visible { outline: .2rem solid #f2c14e; outline-offset: .15rem; }
button:disabled { cursor: not-allowed; opacity: .65; }
.skip-link { position: absolute; inset-inline-start: .5rem; inset-block-start: -5rem; background: #f2c14e; color: #101820; padding: .65rem; z-index: 10; }
.skip-link:focus { inset-block-start: .5rem; }
header { display: flex; align-items: start; justify-content: space-between; gap: 1rem; padding: .8rem 1rem; background: #172631; }
header h1 { margin: 0; font-size: 1.15rem; }
.status-group { text-align: end; overflow-wrap: anywhere; }
.status-group p { margin: 0; }
nav { display: flex; gap: .25rem; overflow-x: auto; padding: .5rem; background: #1d303e; }
.workspace { display: grid; grid-template-columns: minmax(20rem, 2fr) minmax(12rem, 1fr); min-height: 65vh; gap: .5rem; padding: .5rem; }
#conversation, #surface, #session-drawer, .login { background: #172631; padding: 1rem; }
#conversation, #surface, .surface-panel { min-inline-size: 0; }
#conversation h2 { margin-block-start: 0; }
#messages { min-height: 45vh; max-height: 60vh; overflow-y: auto; }
.message { padding: .75rem; margin-block: .35rem; background: #203442; white-space: pre-wrap; overflow-wrap: anywhere; }
.message[data-role="assistant"] { background: #29495a; }
form { display: grid; gap: .55rem; }
textarea, input, select { width: 100%; background: #0f222e; }
.command-bar { display: grid; grid-template-columns: repeat(auto-fit, minmax(8rem, 1fr)); gap: .35rem; background: #1d303e; padding: .5rem; }
#session-drawer { display: flex; gap: .5rem; align-items: center; overflow-x: auto; }
#session-drawer h2 { font-size: 1rem; }
.surface-panel[hidden] { display: none; }
.metric { background: #203442; padding: .65rem; margin-block: .4rem; }
.login { max-width: 32rem; margin: 10vh auto; }
@media (max-width: 48rem) { header { align-items: stretch; flex-direction: column; } .status-group { text-align: start; } .workspace { grid-template-columns: 1fr; } #messages { min-height: 35vh; } nav { position: sticky; top: 0; } }
@media (max-width: 30rem) { #conversation, #surface, #session-drawer, .login { padding: .65rem; } .command-bar { grid-template-columns: 1fr 1fr; } nav button { min-inline-size: max-content; } }
@media (min-width: 80rem) { .workspace { grid-template-columns: minmax(36rem, 3fr) minmax(24rem, 1fr); } }
@media (prefers-reduced-motion: reduce) { *, *::before, *::after { scroll-behavior: auto !important; transition-duration: 0s !important; animation-duration: 0s !important; } }
@media (prefers-contrast: more) { :root { background: #000; color: #fff; } body, header, nav, #conversation, #surface, #session-drawer { background: #000; } button, input, textarea, select, .message, .metric { background: #1b1b1b; } }
"#;

fn surface_panel(surface: Surface, profile: &str, csrf: &str) -> String {
    let hidden = if surface == Surface::Sessions {
        ""
    } else {
        " hidden"
    };
    let content = match surface {
        Surface::Models => r#"<form id="model-form"><label for="model-provider">Provider</label><input id="model-provider" name="provider" required maxlength="128"><label for="model-name">Model</label><input id="model-name" name="model" required maxlength="256"><button type="submit">Select model</button></form>"#.to_owned(),
        Surface::Confirmations => r#"<div class="projection" aria-live="polite"></div><form id="confirmation-form"><label for="confirmation-id">Confirmation identifier</label><input id="confirmation-id" name="confirmation" required maxlength="64"><label for="confirmation-decision">Decision</label><select id="confirmation-decision" name="decision"><option value="allow_once">Allow once</option><option value="deny">Deny</option></select><button type="submit">Resolve confirmation</button></form>"#.to_owned(),
        Surface::Memory => domain_form("memory", "Memory update", "Remembered information"),
        Surface::Schedules => domain_form("schedule", "Schedule", "Prompt to schedule"),
        Surface::Channels => domain_form("channel", "Channel delivery", "Message to deliver"),
        Surface::Refinement => domain_form("refinement", "Refinement", "Requested refinement"),
        Surface::Settings => format!(
            r#"<form id="credential-form" method="post" action="/api/profiles/{profile}/credentials"><input type="hidden" name="csrf" value="{csrf}"><label for="provider">Provider</label><input id="provider" name="provider" required maxlength="128"><label for="credential-name">Reference name</label><input id="credential-name" name="name" required maxlength="128"><label for="credential-secret">Credential</label><input id="credential-secret" name="secret" type="password" autocomplete="new-password" required maxlength="65536"><button type="button" id="save-credential">Save credential</button></form>"#,
            profile = escape_html(profile),
            csrf = escape_html(csrf)
        ),
        _ => "<div class=\"projection\" aria-live=\"polite\"></div>".to_owned(),
    };
    format!(
        "<section id=\"panel-{}\" class=\"surface-panel\" data-panel=\"{}\" aria-labelledby=\"nav-{}\" tabindex=\"-1\"{hidden}><h2>{}</h2>{content}</section>",
        surface.route(),
        surface.route(),
        surface.route(),
        surface.label()
    )
}

fn domain_form(kind: &str, label: &str, placeholder: &str) -> String {
    format!(
        "<form class=\"domain-command\" data-kind=\"{kind}\"><label>{label}<textarea name=\"value\" maxlength=\"65536\" placeholder=\"{placeholder}\" required></textarea></label><button type=\"button\">Apply</button></form>"
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_keeps_conversation_mounted_and_obeys_visual_and_secret_rules() {
        let html = shell_page("csrf-proof", &[], &[]);
        assert!(client_parity().is_full());
        assert_eq!(html.matches("id=\"conversation\"").count(), 1);
        for surface in Surface::ALL {
            assert!(html.contains(&format!("data-route=\"{}\"", surface.route())));
            assert!(html.contains(&format!("data-panel=\"{}\"", surface.route())));
            assert!(html.contains(&format!("aria-controls=\"panel-{}\"", surface.route())));
            assert!(html.contains(&format!("aria-labelledby=\"nav-{}\"", surface.route())));
        }
        for command in [
            "steer",
            "cancel",
            "retry",
            "branch",
            "resume",
            "list-sessions",
        ] {
            assert!(html.contains(&format!("data-command=\"{command}\"")));
        }
        for accessible_path in [
            "class=\"skip-link\"",
            "role=\"log\"",
            "role=\"status\"",
            "aria-live=\"polite\"",
            "aria-relevant=\"additions text\"",
            "aria-describedby=\"prompt-help\"",
            "tabindex=\"-1\"",
            "id=\"model-form\"",
            "id=\"confirmation-form\"",
        ] {
            assert!(html.contains(accessible_path));
        }
        assert!(html.contains("type=\"password\""));
        assert!(!html.contains("localStorage"));
        assert!(!html.contains("sessionStorage"));
        for denied in [
            "border:",
            "border-width",
            "border-style",
            "box-shadow",
            "glow",
            "gradient",
            "purple",
        ] {
            assert!(!APP_CSS.to_ascii_lowercase().contains(denied));
        }
        for responsive_or_accessible in [
            "min-block-size: 2.75rem",
            ":focus-visible",
            "max-width: 48rem",
            "max-width: 30rem",
            "min-width: 80rem",
            "prefers-reduced-motion: reduce",
            "prefers-contrast: more",
            "inset-inline-start",
            "overflow-wrap: anywhere",
        ] {
            assert!(APP_CSS.contains(responsive_or_accessible));
        }
        assert!(html.is_ascii());
    }

    #[test]
    fn untrusted_session_titles_are_escaped() {
        let profile = keith_agent_types::ProfileId::new();
        let session = SessionSummary {
            session_id: keith_agent_types::SessionId::new(),
            root_tree_id: keith_agent_types::RootTreeId::new(),
            profile_id: profile,
            title: Some("</button><script>bad()</script>".into()),
            state: keith_protocol::SessionState::Ready,
            updated_at: keith_agent_types::UtcTimestamp::UNIX_EPOCH,
        };
        let html = shell_page("proof", &[], &[session]);
        assert!(!html.contains("<script>bad()"));
        assert!(html.contains("&lt;/button&gt;"));
    }
}
