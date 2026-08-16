#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod client;
#[cfg(not(target_arch = "wasm32"))]
mod security;
#[cfg(not(target_arch = "wasm32"))]
mod server;

#[cfg(not(target_arch = "wasm32"))]
pub use server::{
    CredentialKeySource, OpenAiCompatibilityConfig, ServerArguments, ServerError, WebServer,
    WebServerConfig,
};

use std::fmt::Write as _;

use keith_protocol::{ProfileSummary, SessionSummary};
use keith_provider_catalog::provider_options_html;
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
    let initial_workspace = profiles
        .iter()
        .find(|profile| profile.id.to_string() == initial_profile)
        .map(|profile| profile.workspace_id.to_string())
        .unwrap_or_default();
    let mut panels = String::new();
    for surface in Surface::ALL {
        panels.push_str(&surface_panel(surface, &initial_profile, csrf));
    }
    format!(
        r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="keith-csrf" content="{csrf}"><title>Keith Agent</title><link rel="stylesheet" href="/assets/app.css"></head>
<body><a class="skip-link" href="#conversation">Skip to conversation</a><div id="app" data-profile="{initial_profile}" data-workspace="{initial_workspace}" data-session="{initial_session}">
<aside class="app-sidebar" aria-label="Keith Agent navigation"><div class="brand-row"><h1>Keith Agent</h1><p>Local</p></div>
<button id="new-session" type="button">New chat</button>
<nav aria-label="Workspace">{navigation}</nav>
<section id="session-drawer" aria-label="Session picker"><h2>Recent conversations</h2><div class="session-list">{session_list}</div></section>
<p class="sidebar-footnote">Private workspace on this device</p></aside>
<section class="app-shell"><header><div><p class="eyebrow">Current workspace</p><p class="header-title">Assistant</p></div><div class="status-group"><p id="connection-status" role="status" aria-live="polite">Connection not started</p><p id="presence-status" role="status" aria-live="polite">No authoritative activity</p></div></header>
<main class="workspace"><section id="conversation" aria-labelledby="conversation-heading" tabindex="-1"><h2 id="conversation-heading">Conversation</h2>
<div id="messages" role="log" aria-live="polite" aria-relevant="additions text" aria-atomic="false"></div>
<div class="composer-dock"><form id="composer"><label class="visually-hidden" for="prompt">Message</label><textarea id="prompt" aria-describedby="prompt-help" maxlength="65536" rows="1" placeholder="Message Keith Agent"></textarea>
<button type="submit">Send</button><p id="prompt-help">Enter sends a prompt. Use the controls for steering and session actions.</p></form>
<section class="command-bar" aria-label="Conversation controls"><button type="button" data-command="steer">Steer</button><button type="button" data-command="cancel">Cancel</button><button type="button" data-command="retry">Retry</button><button type="button" data-command="branch">Branch</button><button type="button" data-command="resume">Resume</button><button type="button" data-command="list-sessions">Refresh</button></section></div></section>
<aside id="surface" aria-label="Details">{panels}</aside></main></section>
</div><script type="module" src="/assets/bootstrap.js"></script></body></html>"##,
        csrf = escape_html(csrf),
        initial_workspace = escape_html(&initial_workspace),
    )
}

pub const APP_CSS: &str = r#"
:root { color-scheme: dark; font-family: Inter, ui-sans-serif, system-ui, sans-serif; background: #0d0d0d; color: #ececec; line-height: 1.5; }
* { box-sizing: border-box; }
body { min-height: 100vh; margin: 0; background: #0d0d0d; color: #ececec; }
button, input, textarea, select { min-block-size: 2.75rem; appearance: none; font: inherit; color: inherit; background: #242424; padding: .65rem .8rem; }
button { cursor: pointer; border-radius: .65rem; }
button:hover, button[aria-current="page"] { background: #303030; }
button:focus-visible, input:focus-visible, textarea:focus-visible, select:focus-visible { outline: .2rem solid #d7d7d7; outline-offset: .15rem; }
button:disabled { cursor: not-allowed; opacity: .65; }
.skip-link { position: absolute; inset-inline-start: .5rem; inset-block-start: -5rem; background: #ececec; color: #0d0d0d; padding: .65rem; z-index: 20; }
.skip-link:focus { inset-block-start: .5rem; }
#app { min-height: 100vh; display: grid; grid-template-columns: 17rem minmax(0, 1fr); }
.app-sidebar { min-inline-size: 0; display: flex; flex-direction: column; gap: 1rem; padding: .8rem; background: #151515; }
.brand-row { display: flex; align-items: center; justify-content: space-between; padding: .35rem .45rem; }
.brand-row h1 { margin: 0; font-size: 1.05rem; letter-spacing: -.015em; }
.brand-row p, .sidebar-footnote, .eyebrow { margin: 0; color: #a6a6a6; font-size: .78rem; }
.sidebar-footnote { margin-block-start: auto; padding: .7rem .45rem; }
nav { display: grid; gap: .2rem; }
nav button { width: 100%; text-align: start; background: transparent; }
.app-shell { min-inline-size: 0; display: flex; flex-direction: column; background: #0d0d0d; }
header { min-height: 4rem; display: flex; align-items: center; justify-content: space-between; gap: 1rem; padding: .7rem 1.25rem; background: #0d0d0d; }
.header-title { margin: 0; font-weight: 600; }
.status-group { max-width: 36rem; text-align: end; overflow-wrap: anywhere; color: #a6a6a6; font-size: .78rem; }
.status-group p { margin: 0; }
.workspace { min-height: 0; flex: 1; display: grid; grid-template-columns: minmax(24rem, 1fr) minmax(16rem, 20rem); gap: .75rem; padding: 0 .75rem .75rem; }
#conversation, #surface, .surface-panel { min-inline-size: 0; }
#conversation { min-height: calc(100vh - 4.75rem); display: flex; flex-direction: column; background: #0d0d0d; }
#conversation h2 { position: absolute; inline-size: 1px; block-size: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
#messages { width: min(100%, 52rem); min-height: 50vh; flex: 1; margin-inline: auto; padding: 2rem 1rem 9rem; overflow-y: auto; }
.message { width: fit-content; max-width: 88%; padding: .8rem 1rem; margin-block: .7rem; background: #242424; border-radius: 1.15rem; white-space: pre-wrap; overflow-wrap: anywhere; }
.message[data-role="assistant"] { width: 100%; max-width: 100%; padding-inline: 0; background: #0d0d0d; border-radius: 0; }
.composer-dock { position: sticky; inset-block-end: 0; width: min(100%, 54rem); margin-inline: auto; padding: .5rem 1rem 1rem; background: #0d0d0d; }
#composer { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: end; gap: .5rem; padding: .55rem; background: #2b2b2b; border-radius: 1.6rem; }
#composer textarea { width: 100%; max-height: 12rem; resize: vertical; background: transparent; padding: .65rem .8rem; }
#composer button { min-inline-size: 4.5rem; background: #ececec; color: #171717; border-radius: 1.15rem; }
#prompt-help { grid-column: 1 / -1; margin: 0 .8rem .15rem; color: #a6a6a6; font-size: .72rem; text-align: center; }
.visually-hidden { position: absolute; inline-size: 1px; block-size: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
.command-bar { display: flex; flex-wrap: wrap; justify-content: center; gap: .25rem; padding-block-start: .4rem; }
.command-bar button { min-block-size: 2rem; padding: .3rem .55rem; background: transparent; color: #a6a6a6; font-size: .75rem; }
#surface { padding: 1rem; overflow-y: auto; background: #181818; border-radius: .8rem; }
.surface-panel h2 { margin-block-start: 0; font-size: 1rem; }
.surface-panel form { display: grid; gap: .55rem; }
textarea, input, select { width: 100%; background: #242424; border-radius: .55rem; }
#session-drawer { min-height: 0; display: flex; flex-direction: column; overflow: hidden; }
#session-drawer h2 { margin: .25rem .45rem; color: #a6a6a6; font-size: .78rem; font-weight: 500; }
.session-list { display: grid; gap: .15rem; overflow-y: auto; }
.session { width: 100%; text-align: start; background: transparent; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.metric { background: #242424; padding: .65rem; margin-block: .4rem; border-radius: .55rem; }
.login { max-width: 32rem; margin: 10vh auto; padding: 1.25rem; background: #181818; border-radius: .8rem; }
@media (max-width: 64rem) { #app { grid-template-columns: 13rem minmax(0, 1fr); } .workspace { grid-template-columns: 1fr; } #surface { max-height: 35vh; } }
@media (max-width: 48rem) { #app { display: block; } .app-sidebar { position: static; padding: .5rem; } .brand-row, .sidebar-footnote, #session-drawer { display: none; } nav { display: flex; overflow-x: auto; } nav button { width: auto; min-inline-size: max-content; } header { align-items: stretch; flex-direction: column; } .status-group { text-align: start; } .workspace { padding: 0; } #conversation { min-height: auto; } #messages { min-height: 45vh; padding-block-start: 1rem; } #surface { margin: .5rem; } }
@media (max-width: 30rem) { header { padding: .65rem; } #messages, .composer-dock, #surface, .login { padding-inline: .65rem; } .command-bar button { font-size: .7rem; } }
@media (min-width: 90rem) { .workspace { grid-template-columns: minmax(42rem, 1fr) 22rem; } }
@media (prefers-reduced-motion: reduce) { *, *::before, *::after { scroll-behavior: auto !important; transition-duration: 0s !important; animation-duration: 0s !important; } }
@media (prefers-contrast: more) { :root, body, .app-shell, header, #conversation, .composer-dock, .message[data-role="assistant"] { background: #000; color: #fff; } .app-sidebar, #surface, .login { background: #151515; } button, input, textarea, select, .message, .metric, #composer { background: #252525; } }
"#;

fn surface_panel(surface: Surface, profile: &str, csrf: &str) -> String {
    let hidden = if surface == Surface::Sessions {
        ""
    } else {
        " hidden"
    };
    let provider_options = provider_options_html();
    let content = match surface {
        Surface::Models => format!(
            r#"<form id="model-form"><label for="model-provider">Provider</label><select id="model-provider" name="provider">{provider_options}</select><label for="model-name">Model</label><input id="model-name" name="model" value="gpt-4.1-mini" required maxlength="256"><button type="submit">Select model</button></form>"#
        ),
        Surface::Confirmations => r#"<div class="projection" aria-live="polite"></div><form id="confirmation-form"><label for="confirmation-id">Confirmation identifier</label><input id="confirmation-id" name="confirmation" required maxlength="64"><label for="confirmation-decision">Decision</label><select id="confirmation-decision" name="decision"><option value="allow_once">Allow once</option><option value="deny">Deny</option></select><button type="submit">Resolve confirmation</button></form>"#.to_owned(),
        Surface::Goals => domain_form("goal", "New goal", "Goal objective"),
        Surface::Children => domain_form("child", "New child", "Delegated objective"),
        Surface::Artifacts => domain_form("export", "Session export", "Type export to confirm"),
        Surface::Memory => domain_form("memory", "Memory query", "Search personal memory"),
        Surface::Schedules => domain_form("schedule", "Schedule", "Prompt to schedule"),
        Surface::Channels => domain_form("channel", "Channel delivery", "Message to deliver"),
        Surface::Refinement => domain_form("refinement", "Refinement", "Requested refinement"),
        Surface::Settings => format!(
            r#"<form id="credential-form" method="post" action="/api/profiles/{profile}/credentials"><input type="hidden" name="csrf" value="{csrf}"><label for="provider">Provider</label><select id="provider" name="provider">{provider_options}</select><label for="credential-name">Reference name</label><input id="credential-name" name="name" value="default" required maxlength="128"><label for="credential-secret">Credential</label><input id="credential-secret" name="secret" type="password" autocomplete="new-password" required maxlength="65536"><button type="button" id="save-credential">Save credential</button></form>"#,
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
        for provider in keith_provider_catalog::BUILTIN_PROVIDERS {
            assert!(html.contains(&format!("value=\"{}\"", provider.id)));
            assert!(html.contains(&format!(
                "data-default-model=\"{}\"",
                provider.default_model
            )));
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
        for domain in ["goal", "child", "schedule", "memory", "export"] {
            assert!(html.contains(&format!("data-kind=\"{domain}\"")));
        }
        for accessible_path in [
            "class=\"skip-link\"",
            "class=\"app-sidebar\"",
            "class=\"composer-dock\"",
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
            "max-width: 64rem",
            "max-width: 48rem",
            "max-width: 30rem",
            "min-width: 90rem",
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
