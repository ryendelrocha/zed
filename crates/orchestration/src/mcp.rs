//! The orchestration tools as an MCP server, for external ACP agents.
//!
//! One [`McpServer`] per app listens on a Unix socket inside a 0700 tempdir
//! (no network listener). Each agent session reaches it through the hidden
//! `zed --orchestration-bridge` mode (see [`crate::bridge`]), registered in
//! the session's MCP servers with [`OrchestrationMcpEntry`].
//!
//! # Who is calling
//!
//! The bridge's first message on a connection carries a per-session token.
//! The server hands it to every tool as [`McpCaller`], and each tool resolves
//! it with [`OrchestrationRegistry::resolve_caller`] to the orchestrator
//! thread and the host that owns it. The token only says which orchestrator
//! is calling: it carries no provider credential, and a token that was never
//! minted is rejected.
//!
//! Tokens are minted per ACP session rather than per Zed thread, because the
//! code that creates sessions knows the session id and not the thread. The
//! host maps session to thread when a tool is called.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
};

use anyhow::{Context as _, Result};
use context_server::{
    listener::{McpCaller, McpServer, McpServerTool, ToolResponse},
    types::{
        Implementation, InitializeParams, InitializeResponse, ProtocolVersion, ServerCapabilities,
        ToolAnnotations, ToolResponseContent, ToolsCapabilities, requests::Initialize,
    },
};
use gpui::{App, AsyncApp, Global, Task};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings::SettingsStore;
use util::ResultExt as _;

use crate::{
    AvailableAgents, AwaitChildrenInput, AwaitChildrenOutput, BRIDGE_TOKEN_ENV, CancelChildInput,
    CancelChildOutput, ChildStatusInput, ChildStatusOutput, OrchestrationHost,
    OrchestrationRegistry, SendToChildInput, SendToChildOutput, StartChildInput, StartChildOutput,
    ThreadId,
};

/// Name of the MCP server in the agent's session.
pub const MCP_SERVER_NAME: &str = "zed-orchestration";

/// Whether `agent.orchestration.enabled` is on.
pub fn orchestration_enabled(cx: &App) -> bool {
    cx.try_global::<SettingsStore>().is_some_and(|store| {
        store
            .merged_settings()
            .agent
            .as_ref()
            .and_then(|agent| agent.orchestration.as_ref())
            .and_then(|orchestration| orchestration.enabled)
            .unwrap_or(false)
    })
}

/// The MCP server entry to add to an agent session so the session can use the
/// orchestration tools.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrchestrationMcpEntry {
    pub name: String,
    /// The Zed executable.
    pub command: PathBuf,
    /// `--orchestration-bridge <socket>`.
    pub args: Vec<String>,
    /// Holds the session token in [`BRIDGE_TOKEN_ENV`].
    pub env: HashMap<String, String>,
    token: String,
}

impl OrchestrationMcpEntry {
    pub fn new(command: PathBuf, socket: &Path, token: String) -> Self {
        Self {
            name: MCP_SERVER_NAME.to_string(),
            command,
            args: vec![
                "--orchestration-bridge".to_string(),
                socket.to_string_lossy().into_owned(),
            ],
            env: HashMap::from([(BRIDGE_TOKEN_ENV.to_string(), token.clone())]),
            token,
        }
    }

    /// Ties this entry's token to the session it was given to. Call it for an
    /// entry made with [`mcp_entry_for_new_session`], once the session id
    /// exists. It is not needed for [`mcp_entry_for_session`].
    pub fn bind_session(&self, session_id: &str, cx: &mut App) {
        OrchestrationRegistry::global_or_init(cx).update(cx, |registry, _| {
            registry.bind_session_token(&self.token, session_id);
        });
    }
}

/// The entry for a session that is about to be created, or `None` when the
/// session must not get the tools: orchestration is off, the project is not
/// local, or the MCP server could not start. After the session exists, call
/// [`OrchestrationMcpEntry::bind_session`].
pub fn mcp_entry_for_new_session(is_local: bool, cx: &mut App) -> Option<OrchestrationMcpEntry> {
    mcp_entry(is_local, cx, |registry| {
        registry.mint_pending_session_token()
    })
}

/// The entry for a session that already exists (load or resume). The same
/// session always gets the same token.
pub fn mcp_entry_for_session(
    is_local: bool,
    session_id: &str,
    cx: &mut App,
) -> Option<OrchestrationMcpEntry> {
    mcp_entry(is_local, cx, |registry| {
        registry.token_for_session(session_id)
    })
}

fn mcp_entry(
    is_local: bool,
    cx: &mut App,
    mint: impl FnOnce(&mut OrchestrationRegistry) -> String,
) -> Option<OrchestrationMcpEntry> {
    // The bridge relies on Unix sockets.
    if !cfg!(unix) || !is_local || !orchestration_enabled(cx) {
        return None;
    }
    let socket = ensure_mcp_server(cx)?;
    let command = std::env::current_exe()
        .context("finding the zed executable")
        .log_err()?;
    let token = OrchestrationRegistry::global_or_init(cx).update(cx, |registry, _| mint(registry));
    Some(OrchestrationMcpEntry::new(command, &socket, token))
}

struct GlobalOrchestrationMcpServer {
    socket_path: PathBuf,
    _server: McpServer,
}

impl Global for GlobalOrchestrationMcpServer {}

/// Starts the app's orchestration MCP server the first time it is needed and
/// returns the path of its socket. Later calls return the same path. Returns
/// `None`, after logging, when the server cannot start.
pub fn ensure_mcp_server(cx: &mut App) -> Option<PathBuf> {
    if let Some(server) = cx.try_global::<GlobalOrchestrationMcpServer>() {
        return Some(server.socket_path.clone());
    }
    let mut server = McpServer::start(cx)
        .context("starting the orchestration MCP server")
        .log_err()?;
    register_orchestration_tools(&mut server);
    let socket_path = server.socket_path().to_path_buf();
    cx.set_global(GlobalOrchestrationMcpServer {
        socket_path: socket_path.clone(),
        _server: server,
    });
    Some(socket_path)
}

/// Adds the `initialize` handler and the six tools to `server`.
pub fn register_orchestration_tools(server: &mut McpServer) {
    server.handle_request::<Initialize>(|params: InitializeParams, _cx| {
        // Answer with the client's version when we know it, otherwise with
        // the latest one, and let the client decide whether it can continue.
        let version = match params.protocol_version.0.as_str() {
            version @ ("2024-11-05" | "2025-03-26" | "2025-06-18" | "2025-11-25") => {
                version.to_string()
            }
            _ => context_server::types::LATEST_PROTOCOL_VERSION.to_string(),
        };
        Task::ready(Ok(InitializeResponse {
            protocol_version: ProtocolVersion(version),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapabilities {
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            server_info: Implementation {
                name: MCP_SERVER_NAME.to_string(),
                title: None,
                version: "1".to_string(),
                description: None,
            },
            meta: None,
        }))
    });
    server.add_tool(StartChildTool);
    server.add_tool(AwaitChildrenTool);
    server.add_tool(ChildStatusTool);
    server.add_tool(SendToChildTool);
    server.add_tool(CancelChildTool);
    server.add_tool(ListAgentsAndModelsTool);
}

/// Resolves the caller and runs `f` against its host.
async fn with_host<T: 'static>(
    caller: &McpCaller,
    cx: &mut AsyncApp,
    f: impl FnOnce(ThreadId, Rc<dyn OrchestrationHost>, &mut App) -> Task<Result<T>>,
) -> Result<T> {
    let token = caller
        .token
        .as_deref()
        .context("This connection did not identify itself, so it cannot use orchestration.")?;
    let task = cx.update(|cx| {
        let registry =
            OrchestrationRegistry::try_global(cx).context("Orchestration is not available.")?;
        let (orchestrator, host) = registry
            .read(cx)
            .resolve_caller(token, cx)
            .context("Unknown orchestration token: this session is not an orchestrator.")?;
        anyhow::Ok(f(orchestrator, host, cx))
    })?;
    task.await
}

fn text_response<T: Serialize>(output: T) -> Result<ToolResponse<T>> {
    let text = serde_json::to_string(&output)?;
    Ok(ToolResponse {
        content: vec![ToolResponseContent::Text { text }],
        structured_content: output,
    })
}

fn annotations(read_only: bool, destructive: bool) -> ToolAnnotations {
    ToolAnnotations {
        title: None,
        read_only_hint: Some(read_only),
        destructive_hint: Some(destructive),
        idempotent_hint: None,
        open_world_hint: Some(false),
    }
}

#[derive(Clone)]
pub struct StartChildTool;

impl McpServerTool for StartChildTool {
    type Input = StartChildInput;
    type Output = StartChildOutput;
    const NAME: &'static str = "start_child";

    fn annotations(&self) -> ToolAnnotations {
        annotations(false, false)
    }

    async fn run(
        &self,
        input: Self::Input,
        caller: &McpCaller,
        cx: &mut AsyncApp,
    ) -> Result<ToolResponse<Self::Output>> {
        let output = with_host(caller, cx, |orchestrator, host, cx| {
            host.start_child(orchestrator, input, cx)
        })
        .await?;
        text_response(output)
    }
}

#[derive(Clone)]
pub struct AwaitChildrenTool;

impl McpServerTool for AwaitChildrenTool {
    type Input = AwaitChildrenInput;
    type Output = AwaitChildrenOutput;
    const NAME: &'static str = "await_children";

    fn annotations(&self) -> ToolAnnotations {
        annotations(true, false)
    }

    async fn run(
        &self,
        input: Self::Input,
        caller: &McpCaller,
        cx: &mut AsyncApp,
    ) -> Result<ToolResponse<Self::Output>> {
        let output = with_host(caller, cx, |orchestrator, host, cx| {
            host.await_children(orchestrator, input, cx)
        })
        .await?;
        text_response(output)
    }
}

#[derive(Clone)]
pub struct ChildStatusTool;

impl McpServerTool for ChildStatusTool {
    type Input = ChildStatusInput;
    type Output = ChildStatusOutput;
    const NAME: &'static str = "child_status";

    fn annotations(&self) -> ToolAnnotations {
        annotations(true, false)
    }

    async fn run(
        &self,
        input: Self::Input,
        caller: &McpCaller,
        cx: &mut AsyncApp,
    ) -> Result<ToolResponse<Self::Output>> {
        let output = with_host(caller, cx, |orchestrator, host, cx| {
            Task::ready(host.child_status(orchestrator, input, cx))
        })
        .await?;
        text_response(output)
    }
}

#[derive(Clone)]
pub struct SendToChildTool;

impl McpServerTool for SendToChildTool {
    type Input = SendToChildInput;
    type Output = SendToChildOutput;
    const NAME: &'static str = "send_to_child";

    fn annotations(&self) -> ToolAnnotations {
        annotations(false, false)
    }

    async fn run(
        &self,
        input: Self::Input,
        caller: &McpCaller,
        cx: &mut AsyncApp,
    ) -> Result<ToolResponse<Self::Output>> {
        let output = with_host(caller, cx, |orchestrator, host, cx| {
            host.send_to_child(orchestrator, input, cx)
        })
        .await?;
        text_response(output)
    }
}

#[derive(Clone)]
pub struct CancelChildTool;

impl McpServerTool for CancelChildTool {
    type Input = CancelChildInput;
    type Output = CancelChildOutput;
    const NAME: &'static str = "cancel_child";

    fn annotations(&self) -> ToolAnnotations {
        annotations(false, true)
    }

    async fn run(
        &self,
        input: Self::Input,
        caller: &McpCaller,
        cx: &mut AsyncApp,
    ) -> Result<ToolResponse<Self::Output>> {
        let output = with_host(caller, cx, |orchestrator, host, cx| {
            host.cancel_child(orchestrator, input, cx)
        })
        .await?;
        text_response(output)
    }
}

/// Lists the agents a child can run on, with their models.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ListAgentsAndModelsInput {}

#[derive(Clone)]
pub struct ListAgentsAndModelsTool;

impl McpServerTool for ListAgentsAndModelsTool {
    type Input = ListAgentsAndModelsInput;
    type Output = AvailableAgents;
    const NAME: &'static str = "list_agents_and_models";

    fn annotations(&self) -> ToolAnnotations {
        annotations(true, false)
    }

    async fn run(
        &self,
        _input: Self::Input,
        caller: &McpCaller,
        cx: &mut AsyncApp,
    ) -> Result<ToolResponse<Self::Output>> {
        let output = with_host(caller, cx, |_, host, cx| {
            Task::ready(host.list_available_agents(cx))
        })
        .await?;
        text_response(output)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        cell::RefCell,
        io::{BufRead as _, BufReader, Write as _},
        os::unix::net::UnixStream,
    };

    use futures::{StreamExt as _, channel::mpsc};
    use gpui::{BorrowAppContext as _, TestAppContext};
    use serde_json::{Value, json};

    use super::*;
    use crate::{AvailableAgent, ChildStatus, relay};

    /// A host that records who called it.
    struct FakeHost {
        sessions: Vec<(String, ThreadId)>,
        started: Rc<RefCell<Vec<(ThreadId, String)>>>,
    }

    impl OrchestrationHost for FakeHost {
        fn start_child(
            &self,
            orchestrator: ThreadId,
            input: StartChildInput,
            _cx: &mut App,
        ) -> Task<Result<StartChildOutput>> {
            self.started
                .borrow_mut()
                .push((orchestrator, input.title.clone()));
            Task::ready(Ok(StartChildOutput {
                child_id: ThreadId::new().to_key_string(),
                log_path: "/tmp/log.jsonl".to_string(),
                agent: input.agent.unwrap_or_else(|| "zed-agent".to_string()),
                warning: None,
            }))
        }

        fn send_to_child(
            &self,
            _orchestrator: ThreadId,
            _input: SendToChildInput,
            _cx: &mut App,
        ) -> Task<Result<SendToChildOutput>> {
            Task::ready(Ok(SendToChildOutput { queued: true }))
        }

        fn cancel_child(
            &self,
            _orchestrator: ThreadId,
            _input: CancelChildInput,
            _cx: &mut App,
        ) -> Task<Result<CancelChildOutput>> {
            Task::ready(Ok(CancelChildOutput {
                status: ChildStatus::Cancelled,
            }))
        }

        fn list_available_agents(&self, _cx: &mut App) -> Result<AvailableAgents> {
            Ok(AvailableAgents {
                agents: vec![AvailableAgent {
                    id: "zed-agent".to_string(),
                    name: "Zed Agent".into(),
                    is_native: true,
                    models: Vec::new(),
                }],
            })
        }

        fn resolve_orchestrator(&self, session_id: &str, _cx: &App) -> Option<ThreadId> {
            self.sessions
                .iter()
                .find(|(session, _)| session == session_id)
                .map(|(_, thread)| *thread)
        }

        fn owns_thread(&self, thread_id: ThreadId, _cx: &App) -> bool {
            self.sessions.iter().any(|(_, thread)| *thread == thread_id)
        }
    }

    /// The agent's side of a bridge: it writes what an agent would write to
    /// the bridge's stdin, and reads the bridge's stdout.
    struct BridgeClient {
        to_bridge: UnixStream,
        from_bridge: mpsc::UnboundedReceiver<String>,
        next_id: u64,
    }

    impl BridgeClient {
        fn connect(socket: &Path, token: &str) -> Self {
            let (to_bridge, bridge_stdin) = UnixStream::pair().unwrap();
            let (bridge_stdout, from_bridge_stream) = UnixStream::pair().unwrap();
            let socket = socket.to_string_lossy().into_owned();
            let token = token.to_string();
            std::thread::spawn(move || {
                relay(&socket, &token, bridge_stdin, bridge_stdout).ok();
            });
            let (lines_tx, from_bridge) = mpsc::unbounded();
            std::thread::spawn(move || {
                for line in BufReader::new(from_bridge_stream).lines() {
                    let Ok(line) = line else { return };
                    if lines_tx.unbounded_send(line).is_err() {
                        return;
                    }
                }
            });
            Self {
                to_bridge,
                from_bridge,
                next_id: 0,
            }
        }

        fn notify(&mut self, method: &str) {
            let line = json!({"jsonrpc": "2.0", "method": method});
            writeln!(self.to_bridge, "{line}").unwrap();
        }

        async fn request(&mut self, method: &str, params: Value) -> Value {
            self.next_id += 1;
            let id = self.next_id;
            let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
            writeln!(self.to_bridge, "{line}").unwrap();
            let response = self.from_bridge.next().await.expect("the bridge closed");
            let response: Value = serde_json::from_str(&response).unwrap();
            assert_eq!(response["id"], json!(id), "{response}");
            response
        }

        async fn initialize(&mut self) -> Value {
            let response = self
                .request(
                    "initialize",
                    json!({
                        "protocolVersion": "2025-06-18",
                        "capabilities": {"roots": {"listChanged": true}, "elicitation": {}},
                        "clientInfo": {"name": "claude-code", "version": "2.0.0"},
                    }),
                )
                .await;
            self.notify("notifications/initialized");
            response
        }
    }

    struct Setup {
        socket: PathBuf,
        registry: gpui::Entity<OrchestrationRegistry>,
        started: Rc<RefCell<Vec<(ThreadId, String)>>>,
        orchestrators: Vec<(String, ThreadId)>,
    }

    /// Starts the MCP server with a fake host that owns two orchestrators,
    /// each with its own session token.
    fn setup(cx: &mut TestAppContext) -> (Setup, Vec<String>) {
        cx.executor().allow_parking();
        cx.update(|cx| {
            let registry = OrchestrationRegistry::global_or_init(cx);
            let started = Rc::new(RefCell::new(Vec::new()));
            let orchestrators = vec![
                ("session-a".to_string(), ThreadId::new()),
                ("session-b".to_string(), ThreadId::new()),
            ];
            registry.update(cx, |registry, _| {
                registry.register_host(Rc::new(FakeHost {
                    sessions: orchestrators.clone(),
                    started: started.clone(),
                }));
            });
            // Session a is created through the "new session" path, b through
            // the "load session" path.
            let (token_a, token_b) = registry.update(cx, |registry, _| {
                let token_a = registry.mint_pending_session_token();
                registry.bind_session_token(&token_a, "session-a");
                (token_a, registry.token_for_session("session-b"))
            });
            let socket = ensure_mcp_server(cx).expect("server starts");
            (
                Setup {
                    socket,
                    registry,
                    started,
                    orchestrators,
                },
                vec![token_a, token_b],
            )
        })
    }

    #[gpui::test]
    async fn test_bridge_lists_the_six_tools_with_schemas(cx: &mut TestAppContext) {
        let (setup, tokens) = setup(cx);
        let mut client = BridgeClient::connect(&setup.socket, &tokens[0]);

        let response = client.initialize().await;
        assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(response["result"]["serverInfo"]["name"], MCP_SERVER_NAME);
        assert!(response["result"]["capabilities"]["tools"].is_object());

        let response = client.request("tools/list", json!({})).await;
        let tools = response["result"]["tools"].as_array().expect("tools");
        let mut names: Vec<_> = tools
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "await_children",
                "cancel_child",
                "child_status",
                "list_agents_and_models",
                "send_to_child",
                "start_child",
            ]
        );
        for tool in tools {
            assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
            assert!(
                tool["description"]
                    .as_str()
                    .is_some_and(|text| !text.is_empty()),
                "{tool}"
            );
        }
        let start_child = tools
            .iter()
            .find(|tool| tool["name"] == "start_child")
            .unwrap();
        let required = start_child["inputSchema"]["required"].as_array().unwrap();
        assert!(required.contains(&json!("title")) && required.contains(&json!("prompt")));
        assert!(start_child["inputSchema"]["properties"]["worktree"].is_object());
    }

    #[gpui::test]
    async fn test_tokens_resolve_to_their_own_orchestrator(cx: &mut TestAppContext) {
        let (setup, tokens) = setup(cx);
        let mut client_a = BridgeClient::connect(&setup.socket, &tokens[0]);
        let mut client_b = BridgeClient::connect(&setup.socket, &tokens[1]);
        client_a.initialize().await;
        client_b.initialize().await;

        let call = |title: &str| json!({"name": "start_child", "arguments": {"title": title, "prompt": "do it"}});
        let response = client_a.request("tools/call", call("from a")).await;
        assert_eq!(response["result"]["isError"], false, "{response}");
        assert!(response["result"]["structuredContent"]["child_id"].is_string());
        let response = client_b.request("tools/call", call("from b")).await;
        assert_eq!(response["result"]["isError"], false, "{response}");

        let started = setup.started.borrow().clone();
        assert_eq!(
            started,
            vec![
                (setup.orchestrators[0].1, "from a".to_string()),
                (setup.orchestrators[1].1, "from b".to_string()),
            ]
        );

        // The read-only tools work through the registry.
        let response = client_a
            .request(
                "tools/call",
                json!({"name": "child_status", "arguments": {}}),
            )
            .await;
        assert_eq!(response["result"]["isError"], false, "{response}");
        let response = client_a
            .request(
                "tools/call",
                json!({"name": "list_agents_and_models", "arguments": {}}),
            )
            .await;
        assert_eq!(
            response["result"]["structuredContent"]["agents"][0]["id"],
            "zed-agent"
        );
        let response = client_a
            .request(
                "tools/call",
                json!({"name": "await_children", "arguments": {"timeout_secs": 1}}),
            )
            .await;
        assert_eq!(response["result"]["isError"], false, "{response}");
    }

    #[gpui::test]
    async fn test_unknown_token_is_rejected(cx: &mut TestAppContext) {
        let (setup, tokens) = setup(cx);
        let mut stranger = BridgeClient::connect(&setup.socket, "not-a-minted-token");
        stranger.initialize().await;
        let response = stranger
            .request(
                "tools/call",
                json!({"name": "start_child", "arguments": {"title": "x", "prompt": "y"}}),
            )
            .await;
        assert_eq!(response["result"]["isError"], true, "{response}");
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Unknown orchestration token")
        );

        // A pending token that never got a session, and a revoked orchestrator,
        // are rejected too.
        let pending = setup
            .registry
            .update(cx, |registry, _| registry.mint_pending_session_token());
        let mut unbound = BridgeClient::connect(&setup.socket, &pending);
        unbound.initialize().await;
        let response = unbound
            .request(
                "tools/call",
                json!({"name": "start_child", "arguments": {"title": "x", "prompt": "y"}}),
            )
            .await;
        assert_eq!(response["result"]["isError"], true, "{response}");
        assert!(setup.started.borrow().is_empty());

        // The valid token still works on the same server.
        let mut client = BridgeClient::connect(&setup.socket, &tokens[0]);
        client.initialize().await;
        let response = client
            .request(
                "tools/call",
                json!({"name": "start_child", "arguments": {"title": "x", "prompt": "y"}}),
            )
            .await;
        assert_eq!(response["result"]["isError"], false, "{response}");
    }

    #[gpui::test]
    async fn test_connection_without_identification_is_rejected(cx: &mut TestAppContext) {
        let (setup, _) = setup(cx);
        // Talk to the socket directly, skipping the bridge's identification.
        let mut stream = UnixStream::connect(&setup.socket).unwrap();
        let (lines_tx, mut lines) = mpsc::unbounded();
        let reader = stream.try_clone().unwrap();
        std::thread::spawn(move || {
            for line in BufReader::new(reader).lines().map_while(Result::ok) {
                lines_tx.unbounded_send(line).ok();
            }
        });
        let request = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {
            "name": "start_child", "arguments": {"title": "x", "prompt": "y"}}});
        writeln!(stream, "{request}").unwrap();
        let response: Value = serde_json::from_str(&lines.next().await.unwrap()).unwrap();
        assert_eq!(response["result"]["isError"], true, "{response}");
        assert!(setup.started.borrow().is_empty());
    }

    #[gpui::test]
    fn test_session_tokens_are_stable_per_session(cx: &mut TestAppContext) {
        let registry = cx.update(OrchestrationRegistry::global_or_init);
        registry.update(cx, |registry, _| {
            let pending = registry.mint_pending_session_token();
            registry.bind_session_token(&pending, "session-1");
            // Loading the session again gives the same token.
            assert_eq!(registry.token_for_session("session-1"), pending);
            let other = registry.token_for_session("session-2");
            assert_ne!(other, pending);
            assert_eq!(registry.token_for_session("session-2"), other);
        });
    }

    #[gpui::test]
    fn test_entry_shape_and_gating(cx: &mut TestAppContext) {
        let entry = OrchestrationMcpEntry::new(
            PathBuf::from("/Applications/Zed.app/Contents/MacOS/zed"),
            Path::new("/tmp/zed-mcp1/mcp.sock"),
            "secret".to_string(),
        );
        assert_eq!(entry.name, "zed-orchestration");
        assert_eq!(
            entry.args,
            ["--orchestration-bridge", "/tmp/zed-mcp1/mcp.sock"]
        );
        assert_eq!(
            entry.env.get(BRIDGE_TOKEN_ENV).map(String::as_str),
            Some("secret")
        );

        // No settings store means orchestration is off: no entry.
        cx.update(|cx| {
            assert!(mcp_entry_for_new_session(true, cx).is_none());
            assert!(mcp_entry_for_session(true, "s", cx).is_none());
        });
    }

    #[gpui::test]
    fn test_entry_only_for_local_projects_with_orchestration_enabled(cx: &mut TestAppContext) {
        let set_enabled = |enabled: bool, cx: &mut TestAppContext| {
            cx.update(|cx| {
                cx.update_global::<SettingsStore, _>(|store, cx| {
                    store.update_user_settings(cx, |settings| {
                        settings
                            .agent
                            .get_or_insert_default()
                            .orchestration
                            .get_or_insert_default()
                            .enabled = Some(enabled);
                    });
                });
            });
        };
        cx.update(|cx| {
            let store = SettingsStore::test(cx);
            cx.set_global(store);
        });

        set_enabled(false, cx);
        cx.update(|cx| {
            assert!(!orchestration_enabled(cx));
            assert!(mcp_entry_for_new_session(true, cx).is_none());
        });

        set_enabled(true, cx);
        cx.update(|cx| {
            assert!(orchestration_enabled(cx));
            // Remote project: no entry.
            assert!(mcp_entry_for_new_session(false, cx).is_none());
            assert!(mcp_entry_for_session(false, "s", cx).is_none());

            let entry = mcp_entry_for_session(true, "s", cx).expect("entry");
            assert_eq!(entry.args[0], "--orchestration-bridge");
            assert!(Path::new(&entry.args[1]).ends_with("mcp.sock"));
            // The same session gets the same entry every time.
            assert_eq!(mcp_entry_for_session(true, "s", cx), Some(entry.clone()));

            // A new session gets a fresh token, and binding it makes it
            // the session's token.
            let new_entry = mcp_entry_for_new_session(true, cx).expect("entry");
            assert_ne!(new_entry.env, entry.env);
            new_entry.bind_session("s2", cx);
            assert_eq!(mcp_entry_for_session(true, "s2", cx), Some(new_entry));
        });
    }
}
