#![forbid(unsafe_code)]

use std::fs;
use std::io::{self, IsTerminal};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use keith_agent_tui::{
    Accessibility, AgentCommandDispatcher, AgentConnectionClient, AppAction, DispatchEvent, TuiApp,
    TuiArguments, render,
};
use keith_protocol::WireMessage;
use signal_hook::consts::{SIGINT, SIGTERM};

enum LoopExit {
    Quit,
    ExternalEditor,
}

fn run() -> Result<(), String> {
    let Some(arguments) = TuiArguments::parse(std::env::args_os())? else {
        return Ok(());
    };
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("agent-tui requires an interactive terminal".into());
    }
    let shutdown = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&shutdown))
        .map_err(|error| format!("failed to register SIGTERM: {error}"))?;
    signal_hook::flag::register(SIGINT, Arc::clone(&shutdown))
        .map_err(|error| format!("failed to register SIGINT: {error}"))?;
    let accessibility = Accessibility {
        color_mode: arguments.color_mode,
        reduced_motion: arguments.reduced_motion,
    };
    let mut app = TuiApp::new(accessibility);
    let client = AgentConnectionClient::connect(
        arguments.mode,
        app.client_id.clone(),
        None,
        arguments.startup_timeout,
    )
    .map_err(|error| error.to_string())?;
    let mut dispatcher = AgentCommandDispatcher::new(client, arguments.startup_timeout);
    app.connected = true;
    app.list_sessions();
    if let Some(session_id) = arguments.session_id {
        app.attach(session_id);
    }

    loop {
        let exit =
            ratatui::run(|terminal| event_loop(terminal, &mut app, &mut dispatcher, &shutdown))
                .map_err(|error| error.to_string())?;
        match exit {
            LoopExit::Quit => return Ok(()),
            LoopExit::ExternalEditor => edit_composer(&mut app)?,
        }
    }
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut TuiApp,
    dispatcher: &mut AgentCommandDispatcher,
    shutdown: &AtomicBool,
) -> io::Result<LoopExit> {
    while !shutdown.load(Ordering::Acquire) && !app.quit {
        while let Some(event) = dispatcher.try_next() {
            match event {
                DispatchEvent::Message(message) => {
                    let message = *message;
                    if matches!(message, WireMessage::CommandResult(_)) {
                        app.command_finished();
                    }
                    app.apply_wire_message(message);
                }
                DispatchEvent::CommandFailed(error) => {
                    app.command_finished();
                    app.report_command_failure(error);
                }
                DispatchEvent::Reconnecting => app.report_reconnecting(),
                DispatchEvent::Reconnected => {
                    app.report_reconnected();
                    app.resume_attached_session();
                }
                DispatchEvent::ReconnectFailed(error) => app.report_reconnect_failure(error),
            }
        }
        while let Some(command) = app.next_command() {
            let envelope = app.command_envelope(command);
            match dispatcher.dispatch(envelope) {
                Ok(()) => app.command_dispatched(),
                Err(error) => app.report_command_failure(error),
            }
        }
        terminal.draw(|frame| render(frame, app))?;
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match app.handle_key(key) {
                AppAction::Quit => return Ok(LoopExit::Quit),
                AppAction::OpenExternalEditor => return Ok(LoopExit::ExternalEditor),
                AppAction::None | AppAction::Redraw => {}
            },
            Event::Paste(content) => {
                app.handle_paste(&content);
            }
            Event::Resize(_, _)
            | Event::FocusGained
            | Event::FocusLost
            | Event::Mouse(_)
            | Event::Key(_) => {}
        }
    }
    Ok(LoopExit::Quit)
}

fn edit_composer(app: &mut TuiApp) -> Result<(), String> {
    let editor = std::env::var_os("VISUAL")
        .or_else(|| std::env::var_os("EDITOR"))
        .ok_or_else(|| "VISUAL or EDITOR must name an external editor".to_owned())?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("message.txt");
    fs::write(&path, &app.composer).map_err(|error| error.to_string())?;
    let status = Command::new(editor)
        .arg(&path)
        .status()
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(format!("external editor exited with {status}"));
    }
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    app.replace_composer(content);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
