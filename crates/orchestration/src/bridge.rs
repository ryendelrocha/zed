//! The hidden `zed --orchestration-bridge <socket>` mode.
//!
//! External ACP agents can only reach an MCP server over stdio or HTTP, so
//! Zed registers itself as a stdio MCP server for them. This process is that
//! server's stdio side: it relays stdin and stdout to the Unix socket of the
//! MCP server that runs inside the Zed app.
//!
//! Framing is newline-delimited JSON-RPC on both sides (MCP's stdio
//! transport, and `context_server::listener`), so the relay copies bytes and
//! never parses them. The one thing it adds is the first line it writes to
//! the socket: an [`IDENTIFY_METHOD`] notification carrying the per-session
//! token from [`BRIDGE_TOKEN_ENV`].

use context_server::listener::IDENTIFY_METHOD;

/// Environment variable that holds the per-session token. It is set in the
/// MCP server entry Zed gives to the agent, never in a tool argument or a
/// prompt, so the model does not see it.
pub const BRIDGE_TOKEN_ENV: &str = "ZED_ORCH_TOKEN";

/// The first line the bridge sends.
pub fn identify_line(token: &str) -> String {
    let mut line = serde_json::json!({
        "jsonrpc": "2.0",
        "method": IDENTIFY_METHOD,
        "params": { "token": token },
    })
    .to_string();
    line.push('\n');
    line
}

/// Runs the bridge on the process's stdin and stdout until either side
/// closes. Errors go to stderr: stdout belongs to the MCP protocol.
#[cfg(unix)]
pub fn run_bridge(socket: &str) -> anyhow::Result<()> {
    use anyhow::Context as _;

    let token = std::env::var(BRIDGE_TOKEN_ENV)
        .ok()
        .filter(|token| !token.is_empty())
        .with_context(|| format!("{BRIDGE_TOKEN_ENV} is not set"))?;
    relay(socket, &token, std::io::stdin(), std::io::stdout().lock())
}

/// Connects to `socket`, identifies with `token`, and copies bytes between
/// `input`/`output` and the socket.
#[cfg(unix)]
pub fn relay(
    socket: &str,
    token: &str,
    mut input: impl std::io::Read + Send + 'static,
    mut output: impl std::io::Write,
) -> anyhow::Result<()> {
    use anyhow::Context as _;
    use std::io::{Read as _, Write as _};

    let mut socket_writer = net::UnixStream::connect(socket)
        .with_context(|| format!("connecting to the orchestration socket {socket}"))?;
    let mut socket_reader = socket_writer.try_clone()?;
    socket_writer.write_all(identify_line(token).as_bytes())?;
    socket_writer.flush()?;

    // input -> socket. When the agent closes its output, close our write side
    // so the server sees the end of the session.
    let uplink = std::thread::spawn(move || -> std::io::Result<()> {
        let mut buffer = [0u8; 8192];
        loop {
            let read = input.read(&mut buffer)?;
            if read == 0 {
                socket_writer.shutdown(std::net::Shutdown::Write).ok();
                return Ok(());
            }
            socket_writer.write_all(&buffer[..read])?;
            socket_writer.flush()?;
        }
    });

    // socket -> output, flushed per chunk so replies are not held back.
    let mut buffer = [0u8; 8192];
    loop {
        let read = socket_reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read])?;
        output.flush()?;
    }
    // The server closed the connection: nothing left to relay. The uplink
    // thread may still be blocked on its input, so do not join it.
    drop(uplink);
    Ok(())
}

#[cfg(not(unix))]
pub fn run_bridge(_socket: &str) -> anyhow::Result<()> {
    anyhow::bail!("the orchestration bridge is only available on Unix")
}
