#![forbid(unsafe_code)]

use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::path::Path;

use keith_framing::{FrameError, LengthDelimitedCodec};
use keith_protocol::{WireFormat, WireMessage, decode, encode};
use thiserror::Error;
use tungstenite::{Message, WebSocket};

pub trait AgentTransport {
    /// # Errors
    ///
    /// Returns a transport, framing, or protocol error when the message cannot be sent.
    fn send(&mut self, message: &WireMessage) -> Result<(), ConnectionError>;

    /// # Errors
    ///
    /// Returns a transport, framing, or protocol error when a message cannot be received.
    fn receive(&mut self) -> Result<WireMessage, ConnectionError>;
}

#[cfg(unix)]
/// # Errors
///
/// Returns an I/O error when the local endpoint cannot be bound or restricted to its owner.
pub fn bind_permissioned_local(path: &Path) -> Result<UnixListener, ConnectionError> {
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

#[cfg(unix)]
/// # Errors
///
/// Returns an I/O error when the permission-restricted local endpoint cannot be connected.
pub fn connect_local(path: &Path) -> Result<UnixStream, ConnectionError> {
    UnixStream::connect(path).map_err(ConnectionError::from)
}

pub struct FramedTransport<S> {
    stream: S,
    format: WireFormat,
    framing: LengthDelimitedCodec,
}

impl<S> FramedTransport<S> {
    pub fn new(stream: S, format: WireFormat) -> Self {
        Self {
            stream,
            format,
            framing: LengthDelimitedCodec::default(),
        }
    }

    pub const fn with_framing(
        stream: S,
        format: WireFormat,
        framing: LengthDelimitedCodec,
    ) -> Self {
        Self {
            stream,
            format,
            framing,
        }
    }

    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S: Read + Write> AgentTransport for FramedTransport<S> {
    fn send(&mut self, message: &WireMessage) -> Result<(), ConnectionError> {
        let payload = encode(self.format, message)?;
        self.framing.write_frame(&mut self.stream, &payload)?;
        Ok(())
    }

    fn receive(&mut self) -> Result<WireMessage, ConnectionError> {
        let payload = self
            .framing
            .read_frame(&mut self.stream)?
            .ok_or(ConnectionError::Closed)?;
        decode(self.format, &payload).map_err(ConnectionError::from)
    }
}

pub struct StdioTransport<R, W> {
    reader: R,
    writer: W,
    format: WireFormat,
    framing: LengthDelimitedCodec,
}

impl<R, W> StdioTransport<R, W> {
    pub fn new(reader: R, writer: W, format: WireFormat) -> Self {
        Self {
            reader,
            writer,
            format,
            framing: LengthDelimitedCodec::default(),
        }
    }
}

impl<R: Read, W: Write> AgentTransport for StdioTransport<R, W> {
    fn send(&mut self, message: &WireMessage) -> Result<(), ConnectionError> {
        let payload = encode(self.format, message)?;
        self.framing.write_frame(&mut self.writer, &payload)?;
        Ok(())
    }

    fn receive(&mut self) -> Result<WireMessage, ConnectionError> {
        let payload = self
            .framing
            .read_frame(&mut self.reader)?
            .ok_or(ConnectionError::Closed)?;
        decode(self.format, &payload).map_err(ConnectionError::from)
    }
}

pub struct WebSocketTransport<S> {
    socket: WebSocket<S>,
    format: WireFormat,
    max_message_bytes: usize,
}

impl<S> WebSocketTransport<S> {
    pub const fn new(socket: WebSocket<S>, format: WireFormat, max_message_bytes: usize) -> Self {
        Self {
            socket,
            format,
            max_message_bytes,
        }
    }

    pub fn into_inner(self) -> WebSocket<S> {
        self.socket
    }
}

impl<S: Read + Write> AgentTransport for WebSocketTransport<S> {
    fn send(&mut self, message: &WireMessage) -> Result<(), ConnectionError> {
        let payload = encode(self.format, message)?;
        if payload.len() > self.max_message_bytes {
            return Err(ConnectionError::MessageTooLarge {
                length: payload.len(),
                limit: self.max_message_bytes,
            });
        }
        let message = match self.format {
            WireFormat::Json => Message::Text(
                String::from_utf8(payload)
                    .map_err(|_| ConnectionError::InvalidUtf8)?
                    .into(),
            ),
            WireFormat::Binary => Message::Binary(payload.into()),
        };
        self.socket.send(message)?;
        Ok(())
    }

    fn receive(&mut self) -> Result<WireMessage, ConnectionError> {
        loop {
            let message = self.socket.read()?;
            let payload: &[u8] = match (&self.format, &message) {
                (WireFormat::Json, Message::Text(text)) => text.as_bytes(),
                (WireFormat::Binary, Message::Binary(bytes)) => bytes.as_ref(),
                (_, Message::Close(_)) => return Err(ConnectionError::Closed),
                (_, Message::Ping(_) | Message::Pong(_)) => continue,
                _ => return Err(ConnectionError::UnexpectedWebSocketMessage),
            };
            if payload.len() > self.max_message_bytes {
                return Err(ConnectionError::MessageTooLarge {
                    length: payload.len(),
                    limit: self.max_message_bytes,
                });
            }
            return decode(self.format, payload).map_err(ConnectionError::from);
        }
    }
}

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("local transport I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error(transparent)]
    Protocol(#[from] keith_protocol::ProtocolError),
    #[error("WebSocket transport failed: {0}")]
    WebSocket(#[from] tungstenite::Error),
    #[error("connection closed before a complete message")]
    Closed,
    #[error("message length {length} exceeds limit {limit}")]
    MessageTooLarge { length: usize, limit: usize },
    #[error("JSON payload was not UTF-8")]
    InvalidUtf8,
    #[error("WebSocket message kind did not match the negotiated wire format")]
    UnexpectedWebSocketMessage,
}

impl ConnectionError {
    pub fn is_interrupted(&self) -> bool {
        match self {
            Self::Io(error) | Self::Frame(FrameError::Io(error)) => {
                error.kind() == std::io::ErrorKind::Interrupted
            }
            Self::Frame(FrameError::Truncated { source, .. }) => {
                source.kind() == std::io::ErrorKind::Interrupted
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::net::{TcpListener, TcpStream};
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    use std::thread;

    use keith_agent_types::{
        CURRENT_PROTOCOL_VERSION, ClientId, CommandId, EntityId, ProtocolVersion, UtcTimestamp,
    };
    use keith_protocol::{
        ClientCommand, ClientHello, CommandEnvelope, CommandResult, CommandResultEnvelope, Feature,
        ResponsePayload, SessionFilter, WireMessage, negotiate,
    };

    use super::*;

    const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

    fn client_hello() -> ClientHello {
        ClientHello {
            protocol: CURRENT_PROTOCOL_VERSION,
            client_id: ClientId::new(),
            client_name: "transport-conformance".into(),
            client_version: "1.0.0".into(),
            supported_features: BTreeSet::from([
                Feature::SessionLifecycle,
                Feature::FramedJson,
                Feature::LocalBinary,
                Feature::Stdio,
                Feature::WebSocket,
            ]),
            resume: None,
        }
    }

    fn client_journey(transport: &mut impl AgentTransport) {
        let hello = client_hello();
        let client_id = hello.client_id.clone();
        transport.send(&WireMessage::ClientHello(hello)).unwrap();
        let WireMessage::ServerHello(server) = transport.receive().unwrap() else {
            panic!("server hello required");
        };
        assert_eq!(server.protocol, CURRENT_PROTOCOL_VERSION);

        let command_id = CommandId::new();
        transport
            .send(&WireMessage::Command(CommandEnvelope {
                protocol: server.protocol,
                command_id: command_id.clone(),
                client_id,
                sent_at: UtcTimestamp::UNIX_EPOCH,
                session_id: None,
                command: ClientCommand::ListSessions(SessionFilter::default()),
            }))
            .unwrap();
        let WireMessage::CommandResult(result) = transport.receive().unwrap() else {
            panic!("command result required");
        };
        assert_eq!(result.command_id, command_id);
        assert_eq!(
            result.result,
            CommandResult::Data(Box::new(ResponsePayload::Sessions(Vec::new())))
        );
    }

    fn server_journey(transport: &mut impl AgentTransport) {
        let WireMessage::ClientHello(client) = transport.receive().unwrap() else {
            panic!("client hello required");
        };
        let features = client.supported_features.clone();
        let server = negotiate(
            &client,
            CURRENT_PROTOCOL_VERSION,
            EntityId::new(),
            &features,
        )
        .unwrap();
        transport.send(&WireMessage::ServerHello(server)).unwrap();

        let WireMessage::Command(command) = transport.receive().unwrap() else {
            panic!("command required");
        };
        assert!(matches!(command.command, ClientCommand::ListSessions(_)));
        transport
            .send(&WireMessage::CommandResult(CommandResultEnvelope {
                protocol: CURRENT_PROTOCOL_VERSION,
                command_id: command.command_id,
                completed_at: UtcTimestamp::UNIX_EPOCH,
                result: CommandResult::Data(Box::new(ResponsePayload::Sessions(Vec::new()))),
            }))
            .unwrap();
    }

    #[cfg(unix)]
    fn run_unix_journey(format: WireFormat) {
        let (client_stream, server_stream) = UnixStream::pair().unwrap();
        let server = thread::spawn(move || {
            server_journey(&mut FramedTransport::new(server_stream, format));
        });
        client_journey(&mut FramedTransport::new(client_stream, format));
        server.join().unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn framed_json_conformance_over_permissionable_local_stream() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent.sock");
        let listener = bind_permissioned_local(&path).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            server_journey(&mut FramedTransport::new(stream, WireFormat::Json));
        });
        let stream = connect_local(&path).unwrap();
        client_journey(&mut FramedTransport::new(stream, WireFormat::Json));
        server.join().unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn binary_conformance_over_permissionable_local_stream() {
        run_unix_journey(WireFormat::Binary);
    }

    #[test]
    fn framed_stdio_conformance_over_real_os_pipes() {
        let (client_reader, server_writer) = os_pipe::pipe().unwrap();
        let (server_reader, client_writer) = os_pipe::pipe().unwrap();
        let server = thread::spawn(move || {
            server_journey(&mut StdioTransport::new(
                server_reader,
                server_writer,
                WireFormat::Json,
            ));
        });
        client_journey(&mut StdioTransport::new(
            client_reader,
            client_writer,
            WireFormat::Json,
        ));
        server.join().unwrap();
    }

    #[test]
    fn websocket_conformance_over_loopback_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let socket = tungstenite::accept(stream).unwrap();
            server_journey(&mut WebSocketTransport::new(
                socket,
                WireFormat::Json,
                MAX_MESSAGE_BYTES,
            ));
        });

        let stream = TcpStream::connect(address).unwrap();
        let request = format!("ws://{address}/agent");
        let (socket, _) = tungstenite::client(request, stream).unwrap();
        client_journey(&mut WebSocketTransport::new(
            socket,
            WireFormat::Json,
            MAX_MESSAGE_BYTES,
        ));
        server.join().unwrap();
    }

    #[test]
    fn transport_preserves_explicit_major_mismatch() {
        let client = ClientHello {
            protocol: ProtocolVersion::new(2, 0),
            ..client_hello()
        };
        let error = negotiate(
            &client,
            CURRENT_PROTOCOL_VERSION,
            EntityId::new(),
            &BTreeSet::new(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            keith_protocol::ProtocolError::MajorMismatch { .. }
        ));
    }
}
