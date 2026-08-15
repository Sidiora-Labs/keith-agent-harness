#![forbid(unsafe_code)]

use std::io::{BufRead, BufReader, Read, Write};

use keith_agent_types::UtcTimestamp;
use keith_channel_core::{
    AdapterEvent, AdapterFailure, AdapterFeatures, ChannelAdapter, InboundMessage, OutboundMessage,
    RetryClass, SendReceipt,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "event", content = "payload")]
pub enum NormalizedPlatformEvent {
    Message(Box<InboundMessage>),
    RateLimited { retry_after_ms: u64 },
    Disconnected { safe_reason: String },
}

impl From<NormalizedPlatformEvent> for AdapterEvent {
    fn from(event: NormalizedPlatformEvent) -> Self {
        match event {
            NormalizedPlatformEvent::Message(message) => Self::Inbound(message),
            NormalizedPlatformEvent::RateLimited { retry_after_ms } => {
                Self::RateLimited { retry_after_ms }
            }
            NormalizedPlatformEvent::Disconnected { safe_reason } => {
                Self::Disconnected { safe_reason }
            }
        }
    }
}

pub struct JsonLineAdapter<S> {
    stream: BufReader<S>,
    features: AdapterFeatures,
    max_event_bytes: u64,
}

impl<S: Read + Write> JsonLineAdapter<S> {
    pub fn new(stream: S, features: AdapterFeatures, max_event_bytes: u64) -> Self {
        Self {
            stream: BufReader::new(stream),
            features,
            max_event_bytes,
        }
    }
}

impl<S: Read + Write> ChannelAdapter for JsonLineAdapter<S> {
    fn features(&self) -> AdapterFeatures {
        self.features.clone()
    }

    fn receive(&mut self) -> Result<AdapterEvent, AdapterFailure> {
        let mut bytes = Vec::new();
        let read = self
            .stream
            .by_ref()
            .take(self.max_event_bytes.saturating_add(1))
            .read_until(b'\n', &mut bytes)
            .map_err(|error| io_failure(&error))?;
        if read == 0 {
            return Ok(AdapterEvent::Disconnected {
                safe_reason: "platform stream closed".to_owned(),
            });
        }
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > self.max_event_bytes {
            return Err(AdapterFailure {
                class: RetryClass::Permanent,
                safe_message: "platform event exceeds the configured limit".to_owned(),
                retry_after_ms: None,
            });
        }
        let event = serde_json::from_slice::<NormalizedPlatformEvent>(&bytes).map_err(|_| {
            AdapterFailure {
                class: RetryClass::Permanent,
                safe_message: "malformed platform event".to_owned(),
                retry_after_ms: None,
            }
        })?;
        Ok(event.into())
    }

    fn send(&mut self, message: &OutboundMessage) -> Result<SendReceipt, AdapterFailure> {
        let mut bytes = serde_json::to_vec(message).map_err(|_| AdapterFailure {
            class: RetryClass::Permanent,
            safe_message: "outbound message could not be encoded".to_owned(),
            retry_after_ms: None,
        })?;
        bytes.push(b'\n');
        self.stream
            .get_mut()
            .write_all(&bytes)
            .map_err(|error| io_failure(&error))?;
        self.stream
            .get_mut()
            .flush()
            .map_err(|error| io_failure(&error))?;
        Ok(SendReceipt {
            platform_message_id: message.idempotency_key.clone(),
            accepted_at: UtcTimestamp::now().unwrap_or(UtcTimestamp::UNIX_EPOCH),
            duplicate_possible: true,
        })
    }

    fn reconnect(&mut self) -> Result<(), AdapterFailure> {
        Err(AdapterFailure {
            class: RetryClass::Permanent,
            safe_message: "this stream adapter does not own its transport reconnect".to_owned(),
            retry_after_ms: None,
        })
    }
}

fn io_failure(error: &std::io::Error) -> AdapterFailure {
    AdapterFailure {
        class: RetryClass::Reconnect,
        safe_message: format!("platform transport failed: {}", error.kind()),
        retry_after_ms: None,
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;
    use std::net::Shutdown;
    use std::os::unix::net::UnixStream;
    use std::thread;

    use keith_agent_types::ArtifactId;
    use keith_channel_core::{
        AdapterCapability, GatewayLimits, GatewayQueue, InboundIntent, RoutedInbound,
    };

    use super::*;

    fn features() -> AdapterFeatures {
        AdapterFeatures {
            capabilities: std::collections::BTreeSet::from([
                AdapterCapability::Attachments,
                AdapterCapability::Threads,
                AdapterCapability::Steering,
                AdapterCapability::Cancellation,
            ]),
            max_attachment_bytes: 4,
            requests_per_minute: Some(60),
        }
    }

    fn platform_message(message_id: &str, occurred_at: i64) -> NormalizedPlatformEvent {
        NormalizedPlatformEvent::Message(Box::new(InboundMessage {
            channel: "json".to_owned(),
            external_account: "account".to_owned(),
            conversation: "conversation".to_owned(),
            thread: Some("thread".to_owned()),
            sender: "sender".to_owned(),
            message_id: message_id.to_owned(),
            reply_target: None,
            text: "hello".to_owned(),
            attachments: Vec::new(),
            occurred_at: UtcTimestamp::from_unix_millis(occurred_at),
            intent: InboundIntent::Prompt,
        }))
    }

    #[test]
    fn conformance_covers_malformed_reordered_duplicate_oversized_rate_limit_and_disconnect() {
        let (platform, gateway) = UnixStream::pair().expect("real local platform stream");
        let platform_thread = thread::spawn(move || {
            let mut platform = platform;
            for event in [
                platform_message("first", 20),
                platform_message("second", 10),
                platform_message("first", 20),
                NormalizedPlatformEvent::RateLimited {
                    retry_after_ms: 250,
                },
                NormalizedPlatformEvent::Disconnected {
                    safe_reason: "reconnect requested".to_owned(),
                },
            ] {
                serde_json::to_writer(&mut platform, &event).expect("encode event");
                platform.write_all(b"\n").expect("event delimiter");
            }
            platform.write_all(b"not-json\n").expect("malformed event");
        });
        let mut adapter = JsonLineAdapter::new(gateway, features(), 1_024);
        let session_id = keith_agent_types::SessionId::new();
        let profile_id = keith_agent_types::ProfileId::new();
        let mut queue = GatewayQueue::new(GatewayLimits {
            max_attachment_bytes: 4,
            max_total_attachment_bytes: 4,
            ..GatewayLimits::default()
        })
        .expect("queue");
        for expected in ["first", "second"] {
            let AdapterEvent::Inbound(message) = adapter.receive().expect("inbound") else {
                panic!("message required");
            };
            assert_eq!(message.message_id, expected);
            queue
                .enqueue(RoutedInbound {
                    profile_id: profile_id.clone(),
                    session_id: session_id.clone(),
                    message: *message,
                })
                .expect("queue inbound");
        }
        let AdapterEvent::Inbound(duplicate) = adapter.receive().expect("duplicate inbound") else {
            panic!("message required");
        };
        assert_eq!(
            queue
                .enqueue(RoutedInbound {
                    profile_id,
                    session_id,
                    message: *duplicate,
                })
                .expect("deduplicate"),
            keith_channel_core::EnqueueOutcome::Duplicate
        );
        assert_eq!(
            adapter.receive().expect("rate limit"),
            AdapterEvent::RateLimited {
                retry_after_ms: 250
            }
        );
        assert!(matches!(
            adapter.receive().expect("disconnect"),
            AdapterEvent::Disconnected { .. }
        ));
        assert_eq!(
            adapter.receive().expect_err("malformed denied").class,
            RetryClass::Permanent
        );
        platform_thread.join().expect("platform completes");
    }

    #[test]
    fn conformance_bounds_input_sends_receipts_and_classifies_reconnect() {
        let (mut oversized_platform, oversized_gateway) =
            UnixStream::pair().expect("oversized stream");
        oversized_platform
            .write_all(&[b'x'; 33])
            .expect("oversized bytes");
        oversized_platform
            .shutdown(Shutdown::Write)
            .expect("finish oversized input");
        let mut bounded = JsonLineAdapter::new(oversized_gateway, features(), 32);
        assert_eq!(
            bounded.receive().expect_err("oversized denied").class,
            RetryClass::Permanent
        );

        let (platform, gateway) = UnixStream::pair().expect("outbound stream");
        let platform_thread = thread::spawn(move || {
            let mut platform = BufReader::new(platform);
            let mut outbound = String::new();
            platform
                .read_line(&mut outbound)
                .expect("read outbound message");
            serde_json::from_str::<OutboundMessage>(&outbound).expect("valid outbound message")
        });
        let mut adapter = JsonLineAdapter::new(gateway, features(), 1_024);
        let outbound = OutboundMessage {
            route: keith_channel_core::ReplyRoute {
                channel: "json".to_owned(),
                external_account: "account".to_owned(),
                conversation: "conversation".to_owned(),
                thread: None,
                reply_to_message: None,
            },
            idempotency_key: "delivery".to_owned(),
            text: "reply".to_owned(),
            artifacts: vec![ArtifactId::new()],
        };
        assert!(adapter.send(&outbound).is_ok());
        assert_eq!(
            platform_thread.join().expect("platform completes"),
            outbound
        );
        assert_eq!(
            adapter
                .reconnect()
                .expect_err("reconnect unsupported")
                .class,
            RetryClass::Permanent
        );
    }
}
