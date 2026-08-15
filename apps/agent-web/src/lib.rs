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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Surface {
    Sessions,
    Goals,
    Plans,
    Children,
    Tools,
    Memory,
    Knowledge,
    Schedules,
    Commitments,
    Channels,
    Settings,
    Artifacts,
    Refinement,
}

impl Surface {
    pub const ALL: [Self; 13] = [
        Self::Sessions,
        Self::Goals,
        Self::Plans,
        Self::Children,
        Self::Tools,
        Self::Memory,
        Self::Knowledge,
        Self::Schedules,
        Self::Commitments,
        Self::Channels,
        Self::Settings,
        Self::Artifacts,
        Self::Refinement,
    ];

    pub const fn route(self) -> &'static str {
        match self {
            Self::Sessions => "sessions",
            Self::Goals => "goals",
            Self::Plans => "plans",
            Self::Children => "children",
            Self::Tools => "tools",
            Self::Memory => "memory",
            Self::Knowledge => "knowledge",
            Self::Schedules => "schedules",
            Self::Commitments => "commitments",
            Self::Channels => "channels",
            Self::Settings => "settings",
            Self::Artifacts => "artifacts",
            Self::Refinement => "refinement",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Sessions => "Sessions",
            Self::Goals => "Goals",
            Self::Plans => "Plans",
            Self::Children => "Children",
            Self::Tools => "Tools",
            Self::Memory => "Memory",
            Self::Knowledge => "Knowledge",
            Self::Schedules => "Schedules",
            Self::Commitments => "Commitments",
            Self::Channels => "Channels",
            Self::Settings => "Settings",
            Self::Artifacts => "Artifacts",
            Self::Refinement => "Refinement",
        }
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
            "<button type=\"button\" data-route=\"{}\">{}</button>",
            surface.route(),
            surface.label()
        );
    }
    let mut session_list = String::new();
    for session in sessions {
        let title = session.title.as_deref().unwrap_or("Untitled session");
        let _ = write!(
            session_list,
            "<button type=\"button\" class=\"session\" data-session=\"{}\" data-profile=\"{}\">{}</button>",
            session.session_id,
            session.profile_id,
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
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="keith-csrf" content="{csrf}"><title>Keith Agent</title><link rel="stylesheet" href="/assets/app.css"></head>
<body><div id="app" data-profile="{initial_profile}" data-session="{initial_session}">
<header><h1>Keith Agent</h1><p id="connection-status" role="status">Connecting</p></header>
<nav aria-label="Workspace">{navigation}</nav>
<main class="workspace"><section id="conversation" aria-label="Conversation">
<div id="messages" role="log" aria-live="polite"></div>
<form id="composer"><label for="prompt">Message</label><textarea id="prompt" maxlength="65536" rows="3"></textarea>
<button type="submit">Send</button></form></section>
<aside id="surface" aria-label="Details">{panels}</aside></main>
<section id="session-drawer" aria-label="Session picker"><h2>Sessions</h2>{session_list}</section>
</div><script type="module" src="/assets/bootstrap.js"></script></body></html>"#,
        csrf = escape_html(csrf),
    )
}

pub const APP_CSS: &str = r#"
:root { color-scheme: light dark; font-family: system-ui, sans-serif; background: #101820; color: #ecf2f8; }
* { box-sizing: border-box; }
body { margin: 0; background: #101820; color: #ecf2f8; }
button, input, textarea, select { font: inherit; color: inherit; background: #243747; padding: .65rem .8rem; }
button { cursor: pointer; }
button:hover, button[aria-current="page"] { background: #31556b; }
button:focus-visible, input:focus-visible, textarea:focus-visible, select:focus-visible { outline: .2rem solid #f2c14e; outline-offset: .15rem; }
header { display: flex; align-items: baseline; justify-content: space-between; padding: .8rem 1rem; background: #172631; }
header h1 { margin: 0; font-size: 1.15rem; }
nav { display: flex; gap: .25rem; overflow-x: auto; padding: .5rem; background: #1d303e; }
.workspace { display: grid; grid-template-columns: minmax(20rem, 2fr) minmax(18rem, 1fr); min-height: 65vh; gap: .5rem; padding: .5rem; }
#conversation, #surface, #session-drawer, .login { background: #172631; padding: 1rem; }
#messages { min-height: 45vh; max-height: 60vh; overflow-y: auto; }
.message { padding: .75rem; margin-block: .35rem; background: #203442; white-space: pre-wrap; overflow-wrap: anywhere; }
.message[data-role="assistant"] { background: #29495a; }
form { display: grid; gap: .55rem; }
textarea, input, select { width: 100%; background: #0f222e; }
#session-drawer { display: flex; gap: .5rem; align-items: center; overflow-x: auto; }
#session-drawer h2 { font-size: 1rem; }
.surface-panel[hidden] { display: none; }
.metric { background: #203442; padding: .65rem; margin-block: .4rem; }
.login { max-width: 32rem; margin: 10vh auto; }
@media (max-width: 48rem) { .workspace { grid-template-columns: 1fr; } #messages { min-height: 35vh; } nav { position: sticky; top: 0; } }
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
        "<section class=\"surface-panel\" data-panel=\"{}\"{hidden}><h2>{}</h2>{content}</section>",
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
        assert_eq!(html.matches("id=\"conversation\"").count(), 1);
        for surface in Surface::ALL {
            assert!(html.contains(&format!("data-route=\"{}\"", surface.route())));
            assert!(html.contains(&format!("data-panel=\"{}\"", surface.route())));
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
