use ::serde::{Deserialize, Serialize};
use anyhow::{Context as _, Result};
use collections::HashMap;
use futures::AsyncReadExt;
use futures::stream::StreamExt;
use futures::{
    AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, FutureExt,
    channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded},
    io::BufReader,
    select_biased,
};
use gpui::{App, AppContext, AsyncApp, Task};
use net::async_net::{UnixListener, UnixStream};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::{json, value::RawValue};
use std::{
    any::TypeId,
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};
use util::ResultExt;

use crate::{
    client::{CspResult, RequestId, Response},
    types::{
        CallToolParams, CallToolResponse, ListToolsResponse, Request, Tool, ToolAnnotations,
        ToolResponseContent,
        requests::{CallTool, ListTools},
    },
};

/// Method of the notification a client sends first on a connection to say
/// who it is. Its `params` are `{"token": "<opaque string>"}`. The server
/// does not interpret the token: it hands it to tools through [`McpCaller`].
/// Only the first identification on a connection counts.
pub const IDENTIFY_METHOD: &str = "zed/identify";

/// Who is on the other end of the connection a tool call came from.
#[derive(Clone, Debug, Default)]
pub struct McpCaller {
    /// The token from the connection's [`IDENTIFY_METHOD`] notification, if
    /// the client sent one.
    pub token: Option<String>,
}

#[derive(Deserialize)]
struct IdentifyParams {
    token: String,
}

pub struct McpServer {
    socket_path: PathBuf,
    tools: Rc<RefCell<HashMap<&'static str, RegisteredTool>>>,
    handlers: Rc<RefCell<HashMap<&'static str, RequestHandler>>>,
    _server_task: Task<()>,
}

struct RegisteredTool {
    tool: Tool,
    handler: ToolHandler,
}

type ToolHandler = Box<
    dyn Fn(
        Option<serde_json::Value>,
        McpCaller,
        &mut AsyncApp,
    ) -> Task<Result<ToolResponse<serde_json::Value>>>,
>;
type RequestHandler = Box<dyn Fn(RequestId, Option<Box<RawValue>>, &App) -> Task<String>>;

impl McpServer {
    pub fn new(cx: &AsyncApp) -> Task<Result<Self>> {
        let task = cx.background_spawn(async move { Self::bind() });

        cx.spawn(async move |cx| {
            let (temp_dir, socket_path, listener) = task.await?;
            Ok(Self::serve(temp_dir, socket_path, listener, cx))
        })
    }

    /// Like [`Self::new`], but binds the socket right away, so the caller
    /// knows [`Self::socket_path`] without waiting. Creating the tempdir and
    /// binding are two quick syscalls.
    pub fn start(cx: &mut App) -> Result<Self> {
        let (temp_dir, socket_path, listener) = Self::bind()?;
        let tools = Rc::new(RefCell::new(HashMap::default()));
        let handlers = Rc::new(RefCell::new(HashMap::default()));
        let server_task = cx.spawn({
            let tools = tools.clone();
            let handlers = handlers.clone();
            async move |cx| Self::accept_loop(listener, temp_dir, tools, handlers, cx).await
        });
        Ok(Self {
            socket_path,
            _server_task: server_task,
            tools,
            handlers,
        })
    }

    /// Creates the socket in a private (0700) tempdir. Only the user that runs
    /// Zed can connect to it.
    fn bind() -> Result<(tempfile::TempDir, PathBuf, UnixListener)> {
        let temp_dir = tempfile::Builder::new().prefix("zed-mcp").tempdir()?;
        let socket_path = temp_dir.path().join("mcp.sock");
        let listener = UnixListener::bind(&socket_path).context("creating mcp socket")?;
        Ok((temp_dir, socket_path, listener))
    }

    fn serve(
        temp_dir: tempfile::TempDir,
        socket_path: PathBuf,
        listener: UnixListener,
        cx: &mut AsyncApp,
    ) -> Self {
        let tools = Rc::new(RefCell::new(HashMap::default()));
        let handlers = Rc::new(RefCell::new(HashMap::default()));
        let server_task = cx.spawn({
            let tools = tools.clone();
            let handlers = handlers.clone();
            async move |cx| Self::accept_loop(listener, temp_dir, tools, handlers, cx).await
        });
        Self {
            socket_path,
            _server_task: server_task,
            tools,
            handlers,
        }
    }

    async fn accept_loop(
        listener: UnixListener,
        temp_dir: tempfile::TempDir,
        tools: Rc<RefCell<HashMap<&'static str, RegisteredTool>>>,
        handlers: Rc<RefCell<HashMap<&'static str, RequestHandler>>>,
        cx: &mut AsyncApp,
    ) {
        while let Ok((stream, _)) = listener.accept().await {
            Self::serve_connection(stream, tools.clone(), handlers.clone(), cx);
        }
        drop(temp_dir)
    }

    pub fn add_tool<T: McpServerTool + Clone + 'static>(&mut self, tool: T) {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let mut generator = settings.into_generator();

        let input_schema = generator.root_schema_for::<T::Input>();

        let description = input_schema
            .get("description")
            .and_then(|desc| desc.as_str())
            .map(|desc| desc.to_string());
        debug_assert!(
            description.is_some(),
            "Input schema struct must include a doc comment for the tool description"
        );

        let registered_tool = RegisteredTool {
            tool: Tool {
                name: T::NAME.into(),
                title: None,
                description,
                input_schema: input_schema.into(),
                output_schema: if TypeId::of::<T::Output>() == TypeId::of::<()>() {
                    None
                } else {
                    Some(generator.root_schema_for::<T::Output>().into())
                },
                annotations: Some(tool.annotations()),
            },
            handler: Box::new({
                move |input_value, caller, cx| {
                    let input = match input_value {
                        Some(input) => serde_json::from_value(input),
                        // A client may omit `arguments` for a tool without
                        // required fields.
                        None => serde_json::from_value(json!({}))
                            .or_else(|_| serde_json::from_value(serde_json::Value::Null)),
                    };

                    let tool = tool.clone();
                    match input {
                        Ok(input) => cx.spawn(async move |cx| {
                            let output = tool.run(input, &caller, cx).await?;

                            Ok(ToolResponse {
                                content: output.content,
                                structured_content: serde_json::to_value(output.structured_content)
                                    .unwrap_or_default(),
                            })
                        }),
                        Err(err) => Task::ready(Err(err.into())),
                    }
                }
            }),
        };

        self.tools.borrow_mut().insert(T::NAME, registered_tool);
    }

    pub fn handle_request<R: Request>(
        &mut self,
        f: impl Fn(R::Params, &App) -> Task<Result<R::Response>> + 'static,
    ) {
        let f = Box::new(f);
        self.handlers.borrow_mut().insert(
            R::METHOD,
            Box::new(move |req_id, opt_params, cx| {
                let result = match opt_params {
                    Some(params) => serde_json::from_str(params.get()),
                    None => serde_json::from_value(serde_json::Value::Null),
                };

                let params: R::Params = match result {
                    Ok(params) => params,
                    Err(e) => {
                        return Task::ready(
                            serde_json::to_string(&Response::<R::Response> {
                                jsonrpc: "2.0",
                                id: req_id,
                                value: CspResult::Error(Some(crate::client::Error {
                                    message: format!("{e}"),
                                    code: -32700,
                                })),
                            })
                            .unwrap(),
                        );
                    }
                };
                let task = f(params, cx);
                cx.background_spawn(async move {
                    match task.await {
                        Ok(result) => serde_json::to_string(&Response {
                            jsonrpc: "2.0",
                            id: req_id,
                            value: CspResult::Ok(Some(result)),
                        })
                        .unwrap(),
                        Err(e) => serde_json::to_string(&Response {
                            jsonrpc: "2.0",
                            id: req_id,
                            value: CspResult::Error::<R::Response>(Some(crate::client::Error {
                                message: format!("{e}"),
                                code: -32603,
                            })),
                        })
                        .unwrap(),
                    }
                })
            }),
        );
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    fn serve_connection(
        stream: UnixStream,
        tools: Rc<RefCell<HashMap<&'static str, RegisteredTool>>>,
        handlers: Rc<RefCell<HashMap<&'static str, RequestHandler>>>,
        cx: &mut AsyncApp,
    ) {
        let (read, write) = stream.split();
        let (incoming_tx, mut incoming_rx) = unbounded();
        let (outgoing_tx, outgoing_rx) = unbounded();

        cx.background_spawn(Self::handle_io(outgoing_rx, incoming_tx, write, read))
            .detach();

        cx.spawn(async move |cx| {
            let mut caller = McpCaller::default();
            while let Some(request) = incoming_rx.next().await {
                if request.method == IDENTIFY_METHOD {
                    if caller.token.is_none() {
                        caller.token = request
                            .params
                            .as_ref()
                            .and_then(|params| {
                                serde_json::from_str::<IdentifyParams>(params.get()).ok()
                            })
                            .map(|params| params.token);
                    }
                    continue;
                }
                let Some(request_id) = request.id.clone() else {
                    continue;
                };

                if request.method == CallTool::METHOD {
                    Self::handle_call_tool(
                        request_id,
                        request.params,
                        &caller,
                        &tools,
                        &outgoing_tx,
                        cx,
                    )
                    .await;
                } else if request.method == ListTools::METHOD {
                    Self::handle_list_tools(request_id, &tools, &outgoing_tx);
                } else if let Some(handler) = handlers.borrow().get(&request.method.as_ref()) {
                    let outgoing_tx = outgoing_tx.clone();

                    let task = cx.update(|cx| handler(request_id, request.params, cx));
                    cx.spawn(async move |_| {
                        let response = task.await;
                        outgoing_tx.unbounded_send(response).ok();
                    })
                    .detach();
                } else {
                    Self::send_err(
                        request_id,
                        format!("unhandled method {}", request.method),
                        &outgoing_tx,
                    );
                }
            }
        })
        .detach();
    }

    fn handle_list_tools(
        request_id: RequestId,
        tools: &Rc<RefCell<HashMap<&'static str, RegisteredTool>>>,
        outgoing_tx: &UnboundedSender<String>,
    ) {
        let response = ListToolsResponse {
            tools: tools.borrow().values().map(|t| t.tool.clone()).collect(),
            next_cursor: None,
            meta: None,
        };

        outgoing_tx
            .unbounded_send(
                serde_json::to_string(&Response {
                    jsonrpc: "2.0",
                    id: request_id,
                    value: CspResult::Ok(Some(response)),
                })
                .unwrap_or_default(),
            )
            .ok();
    }

    async fn handle_call_tool(
        request_id: RequestId,
        params: Option<Box<RawValue>>,
        caller: &McpCaller,
        tools: &Rc<RefCell<HashMap<&'static str, RegisteredTool>>>,
        outgoing_tx: &UnboundedSender<String>,
        cx: &mut AsyncApp,
    ) {
        let result: Result<CallToolParams, serde_json::Error> = match params.as_ref() {
            Some(params) => serde_json::from_str(params.get()),
            None => serde_json::from_value(serde_json::Value::Null),
        };

        match result {
            Ok(params) => {
                if let Some(tool) = tools.borrow().get(&params.name.as_ref()) {
                    let outgoing_tx = outgoing_tx.clone();

                    let task = (tool.handler)(params.arguments, caller.clone(), cx);
                    cx.spawn(async move |_| {
                        let response = match task.await {
                            Ok(result) => CallToolResponse {
                                content: result.content,
                                is_error: Some(false),
                                meta: None,
                                structured_content: if result.structured_content.is_null() {
                                    None
                                } else {
                                    Some(result.structured_content)
                                },
                            },
                            Err(err) => CallToolResponse {
                                content: vec![ToolResponseContent::Text {
                                    text: err.to_string(),
                                }],
                                is_error: Some(true),
                                meta: None,
                                structured_content: None,
                            },
                        };

                        outgoing_tx
                            .unbounded_send(
                                serde_json::to_string(&Response {
                                    jsonrpc: "2.0",
                                    id: request_id,
                                    value: CspResult::Ok(Some(response)),
                                })
                                .unwrap_or_default(),
                            )
                            .ok();
                    })
                    .detach();
                } else {
                    Self::send_err(
                        request_id,
                        format!("Tool not found: {}", params.name),
                        outgoing_tx,
                    );
                }
            }
            Err(err) => {
                Self::send_err(request_id, err.to_string(), outgoing_tx);
            }
        }
    }

    fn send_err(
        request_id: RequestId,
        message: impl Into<String>,
        outgoing_tx: &UnboundedSender<String>,
    ) {
        outgoing_tx
            .unbounded_send(
                serde_json::to_string(&Response::<()> {
                    jsonrpc: "2.0",
                    id: request_id,
                    value: CspResult::Error(Some(crate::client::Error {
                        message: message.into(),
                        code: -32601,
                    })),
                })
                .unwrap(),
            )
            .ok();
    }

    async fn handle_io(
        mut outgoing_rx: UnboundedReceiver<String>,
        incoming_tx: UnboundedSender<RawRequest>,
        mut outgoing_bytes: impl Unpin + AsyncWrite,
        incoming_bytes: impl Unpin + AsyncRead,
    ) -> Result<()> {
        let mut output_reader = BufReader::new(incoming_bytes);
        let mut incoming_line = String::new();
        loop {
            select_biased! {
                message = outgoing_rx.next().fuse() => {
                    if let Some(message) = message {
                        log::trace!("send: {message}");
                        outgoing_bytes.write_all(message.as_bytes()).await?;
                        outgoing_bytes.write_all(&[b'\n']).await?;
                    } else {
                        break;
                    }
                }
                bytes_read = output_reader.read_line(&mut incoming_line).fuse() => {
                    if bytes_read? == 0 {
                        break
                    }
                    log::trace!("recv: {incoming_line}");
                    match serde_json::from_str(&incoming_line) {
                        Ok(message) => {
                            incoming_tx.unbounded_send(message).log_err();
                        }
                        Err(error) => {
                            outgoing_bytes.write_all(serde_json::to_string(&json!({
                                "jsonrpc": "2.0",
                                "error": json!({
                                    "code": -32603,
                                    "message": format!("Failed to parse: {error}"),
                                }),
                            }))?.as_bytes()).await?;
                            outgoing_bytes.write_all(&[b'\n']).await?;
                            log::error!("failed to parse incoming message: {error}. Raw: {incoming_line}");
                        }
                    }
                    incoming_line.clear();
                }
            }
        }
        Ok(())
    }
}

pub trait McpServerTool {
    type Input: DeserializeOwned + JsonSchema;
    type Output: Serialize + JsonSchema;

    const NAME: &'static str;

    fn annotations(&self) -> ToolAnnotations {
        ToolAnnotations {
            title: None,
            read_only_hint: None,
            destructive_hint: None,
            idempotent_hint: None,
            open_world_hint: None,
        }
    }

    /// `caller` identifies the connection the call arrived on.
    fn run(
        &self,
        input: Self::Input,
        caller: &McpCaller,
        cx: &mut AsyncApp,
    ) -> impl Future<Output = Result<ToolResponse<Self::Output>>>;
}

#[derive(Debug)]
pub struct ToolResponse<T> {
    pub content: Vec<ToolResponseContent>,
    pub structured_content: T,
}

#[derive(Debug, Serialize, Deserialize)]
struct RawRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<RequestId>,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Box<serde_json::value::RawValue>>,
}
