#![forbid(unsafe_code)]

#[cfg(any(target_arch = "wasm32", test))]
mod client;
#[cfg(not(target_arch = "wasm32"))]
mod security;
#[cfg(not(target_arch = "wasm32"))]
mod server;

#[cfg(not(target_arch = "wasm32"))]
pub use server::{
    CredentialKeySource, OpenAiCompatibilityConfig, PlatformCompatibilityConfig, ServerArguments,
    ServerError, WebServer, WebServerConfig,
};

use keith_protocol::{ProfileSummary, SessionSummary};
use std::fmt::Write as _;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebAssets {
    pub script: String,
    pub styles: Vec<String>,
}

pub fn login_page(stylesheet: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en" data-theme="light"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Welcome to Keith</title><link rel="stylesheet" href="{}"></head>
<body><main class="login-shell"><p class="login-kicker">Your personal intelligence</p><h1>Welcome back</h1><p>Sign in to continue with Keith.</p>
<form method="post" action="/auth/session"><label for="password">Password</label>
<input id="password" name="password" type="password" autocomplete="current-password" required maxlength="4096">
<button type="submit">Continue</button></form></main></body></html>"#,
        escape_html(stylesheet)
    )
}

pub fn shell_page(
    csrf: &str,
    profiles: &[ProfileSummary],
    sessions: &[SessionSummary],
    assets: &WebAssets,
) -> String {
    let profiles = escape_html(&serde_json::to_string(profiles).unwrap_or_else(|_| "[]".into()));
    let sessions = escape_html(&serde_json::to_string(sessions).unwrap_or_else(|_| "[]".into()));
    let styles = assets
        .styles
        .iter()
        .fold(String::new(), |mut html, stylesheet| {
            let _ = write!(
                html,
                "<link rel=\"stylesheet\" href=\"{}\">",
                escape_html(stylesheet)
            );
            html
        });
    format!(
        r#"<!doctype html>
<html lang="en" data-theme="light"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Keith</title>{styles}</head>
<body><div id="keith-app" data-csrf="{csrf}" data-profiles="{profiles}" data-sessions="{sessions}"></div>
<script type="module" src="{script}"></script></body></html>"#,
        csrf = escape_html(csrf),
        script = escape_html(&assets.script),
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

    fn assets() -> WebAssets {
        WebAssets {
            script: "/assets/ui/assets/keith-test.js".into(),
            styles: vec!["/assets/ui/assets/keith-test.css".into()],
        }
    }

    #[test]
    fn authenticated_shell_is_a_secret_safe_solid_mount() {
        let html = shell_page("csrf-proof", &[], &[], &assets());
        assert!(html.contains("id=\"keith-app\""));
        assert!(html.contains("data-profiles=\"[]\""));
        assert!(html.contains("data-sessions=\"[]\""));
        assert!(html.contains("/assets/ui/assets/keith-test.js"));
        assert!(html.contains("/assets/ui/assets/keith-test.css"));
        assert_eq!(html.matches("<script").count(), 1);
        for forbidden in [
            "localStorage",
            "sessionStorage",
            "access secret",
            "control plane",
            "confirmation identifier",
            "data-panel",
        ] {
            assert!(!html.contains(forbidden));
        }
    }

    #[test]
    fn untrusted_bootstrap_data_and_asset_paths_are_escaped() {
        let profile = keith_agent_types::ProfileId::new();
        let session = SessionSummary {
            session_id: keith_agent_types::SessionId::new(),
            root_tree_id: keith_agent_types::RootTreeId::new(),
            profile_id: profile,
            title: Some("</div><script>bad()</script>".into()),
            state: keith_protocol::SessionState::Ready,
            updated_at: keith_agent_types::UtcTimestamp::UNIX_EPOCH,
        };
        let html = shell_page("\" onmouseover=\"bad", &[], &[session], &assets());
        assert!(!html.contains("<script>bad()"));
        assert!(!html.contains("onmouseover=\"bad"));
        assert!(html.contains("&lt;/div&gt;"));
        assert!(html.contains("&quot;"));
    }

    #[test]
    fn login_uses_the_production_token_stylesheet() {
        let html = login_page("/assets/ui/assets/keith-test.css");
        assert!(html.contains("class=\"login-shell\""));
        assert!(html.contains("Your personal intelligence"));
        assert!(html.contains("type=\"password\""));
        assert!(html.contains("/assets/ui/assets/keith-test.css"));
    }
}
