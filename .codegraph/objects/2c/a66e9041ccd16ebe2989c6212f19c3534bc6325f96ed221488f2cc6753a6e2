#![forbid(unsafe_code)]

use std::io::{Read, Write};

use keith_composio::proxy_mcp_once;

const MAX_REQUEST_BYTES: u64 = 1_048_576;

fn main() {
    if run().is_err() {
        std::process::exit(1);
    }
}

fn run() -> Result<(), ()> {
    let mut arguments = std::env::args().skip(1);
    let endpoint = arguments.next().ok_or(())?;
    let timeout_ms = arguments.next().ok_or(())?.parse::<u64>().map_err(|_| ())?;
    let max_response_bytes = arguments
        .next()
        .ok_or(())?
        .parse::<usize>()
        .map_err(|_| ())?;
    if arguments.next().is_some() {
        return Err(());
    }
    let api_key = std::env::var("KEITH_COMPOSIO_MCP_API_KEY").map_err(|_| ())?;
    let mut request = Vec::new();
    std::io::stdin()
        .take(MAX_REQUEST_BYTES.saturating_add(1))
        .read_to_end(&mut request)
        .map_err(|_| ())?;
    if request.is_empty() || u64::try_from(request.len()).unwrap_or(u64::MAX) > MAX_REQUEST_BYTES {
        return Err(());
    }
    while request.last().is_some_and(u8::is_ascii_whitespace) {
        request.pop();
    }
    let response = proxy_mcp_once(
        &endpoint,
        &api_key,
        &request,
        timeout_ms,
        max_response_bytes,
    )
    .map_err(|_| ())?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&response).map_err(|_| ())?;
    stdout.write_all(b"\n").map_err(|_| ())?;
    stdout.flush().map_err(|_| ())
}
