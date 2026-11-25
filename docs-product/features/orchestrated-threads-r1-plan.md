# Implementation plan: orchestrated threads, slice R1–R5

> Scope: R1, R2, R3, R4 and R5 of `orchestrated-threads.md`. R3b (sibling messages), R6 (quota) and R13 (permissions) were later slices, and the design did not block them: they are done (see "Beyond R1 to R5" at the end). Status at `bd64c7b`: all steps below are done.

## Decisions

### D-R1. Exposing the tools to external ACP agents: Zed-hosted MCP server behind a stdio bridge

- Reuse the in-process MCP server in `crates/context_server/src/listener.rs`: `McpServer::new`, `add_tool<T: McpServerTool>`, `socket_path()`. It listens on a Unix socket in a 0700 tempdir and is unused today.
- An agent can only use stdio or http MCP servers, so add a hidden mode of the zed binary, `--orchestration-bridge <socket>`, that pipes stdin and stdout to the socket.
- Inject it into every session as `McpServer::Stdio { name: "zed-orchestration", command: current_exe, args: ["--orchestration-bridge", socket], env: [ZED_ORCH_TOKEN=<per-session token>] }`, in two places:
  - v1: `mcp_servers_for_project` (`crates/agent_servers/src/acp/v1.rs` ~10196; call sites ~1408, 1601, 1645);
  - v2: the `SessionContext` builder (`crates/agent_servers/src/acp/v2.rs` ~660-705).
- The registry mints the token, and it maps to the orchestrator `ThreadId`. That is how the server knows who is calling, and R3b and R6 need the same identity.
- Only for local projects, and only when `agent.orchestration.enabled` is on.
- Why not MCP-over-ACP: Zed only negotiates it on the v2 path, which sits behind a beta flag, and only some adapters implement it. It is a later upgrade.

### D-R4. Parent link: a column in the thread metadata

- `ThreadMetadata` gets `parent_thread_id: Option<ThreadId>`.
- A migration appends `ALTER TABLE sidebar_threads ADD COLUMN parent_thread_id BLOB` to `ThreadMetadataDb::MIGRATIONS` in `crates/agent_ui/src/thread_metadata_store.rs`.
- Update the list queries (with and without `delta_clones`), the `save()` upsert, the row deserializer, every `ThreadMetadata { .. }` literal, and the import path that reads another channel's DB (it has no such column).
- Why: children are separate conversations, possibly on another agent or another workspace. Registering them as subagents of the parent `AcpThread` would break ownership. The metadata link also persists across restarts.
- `test_subagent_threads_excluded_from_sidebar_metadata` stays valid and unchanged: orchestrated children are regular conversations and are saved.

## Steps

1. **New crate `crates/orchestration`.** Done in `36ead50`. It depends on gpui and the acp_thread types, not on agent_ui. It contains:
   - tool input and output types with JsonSchema and doc comments: `StartChildInput { title, prompt, agent?, model?, worktree: Shared|New }`, `AwaitChildrenInput { child_ids, timeout_secs? }`, `ChildStatusInput`, `SendToChildInput`, `CancelChildInput`;
   - `ChildStatus` = `Starting|Running|WaitingForConfirmation|Completed|Failed|Cancelled`;
   - `OrchestrationRegistry`, a global: orchestrator `ThreadId` → `Vec<ChildRecord { child_id, title, agent_id, model, status, final_message, turns, last_activity, log_path, watch::Sender<ChildStatus> }>`, plus token → `ThreadId`;
   - `ChildLog`: JSONL at `agent.orchestration.log_dir` (default `$TMPDIR/zed-orchestration/<orchestrator>/<child>.jsonl`), one line per orchestrator↔child exchange (`{ts, direction: to_child|from_child, text, status}`), nothing from the child's internal tool calls;
   - `OrchestrationHost` trait: `start_child`, `send_to_child`, `cancel_child`, `child_status`, `await_children`, `list_available_agents`.

   Also add the `agent.orchestration.enabled` and `log_dir` settings (`crates/settings_content/src/agent.rs`, defaults, Settings UI, docs).
2. **Metadata parent link (D-R4).** Done in `36ead50`. `CreateThreadOptions` gets `parent_thread_id` (`crates/agent_ui/src/agent_panel.rs` ~1044), and the panel stores it in the new thread's metadata.
3. **Child creation.** Done in `36ead50` (host, `create_child_thread`, follow-up, cancel); the model for native agents and later the profile (`10530cf`) are applied there too. In `agent_panel.rs`, extract `create_child_thread(...) -> (ThreadId, Entity<ConversationView>, warning)` from `AgentPanelSiblingHost::create_sibling_thread` (~4968-5120), and have `create_sibling_thread` call it too.
   - Add `AgentPanelOrchestrationHost: OrchestrationHost`. It observes each child's root `AcpThread` (`AcpThreadEvent::Stopped/Error/Refusal/ConnectionLost`, `status()`, `had_error()`), updates the registry, and appends to `ChildLog`.
   - Add `AcpThread::last_assistant_message(cx)`.
   - Follow-up: through the ConversationView/ThreadView send path, so a child that is mid-turn queues the message.
   - Cancel: `ConversationView::cancel_generation`.
   - The model is applied only for the native agent in this slice. For an external agent child, `model` returns a clear "not supported yet" warning.
4. **Native tools** (done in `b1c2733`) in `crates/agent/src/tools/`: `start_child`, `await_children`, `child_status`, `send_to_child` and `cancel_child`, as thin `AgentTool` adapters over the core. Register them in `crates/agent/src/thread.rs` (~2408) and gate them on `agent.orchestration.enabled`. `ThreadEnvironment` gets the orchestration methods, defaulting to "not supported". Keep `create_thread` as it is.
5. **MCP server and bridge (D-R1).** Done in `b1c2733`. `McpServerTool` adapters over the same core, with the server started lazily. Add the hidden bridge mode in `crates/zed/src/main.rs`. `agent_servers` asks the registry for the `acp::McpServer` entry to inject.
6. **Sidebar.** Done: live subagent rows in `b7c55ae`, `5921469`, `0d896ef`, `c449f10`; metadata children in `b1c2733`. While `show_as_subthreads` is on, metadata threads that have `parent_thread_id` leave the top level and render under their parent as indented thread rows. They reuse `collapsed_subthread_groups`, `fully_loaded_subthread_groups`, `subthreads_visible` and the recency order, and they open like any thread. The existing live-subagent path stays.
7. **Restart.** Done in `cbf577d`. Rebuild the registry skeleton from metadata. After a restart, `await_children` returns the persisted last message when there is one.
   - Done: an observer on the thread metadata store adds a record for every unarchived thread with a parent link that no panel holds (`restore_child`). The status is `completed` unless the child's R4 log says `failed` or `cancelled`, or ends with a message nobody answered (then `failed`, with a note that Zed restarted). The last final message, the turn count and the sibling counters come from the log, so after a reboot that clears `$TMPDIR` the child has no final message. `send_to_child` to an unloaded child opens it in the orchestrator's panel without activating it, and parks the message until its agent connects. Limit: a child in another workspace's panel (`worktree: new`) that is already open there is not reachable from this one.
8. **Spec.** Update R1 in `orchestrated-threads.md` with D-R1. Done in the v0.2 spec update (R1 now describes the stdio bridge and the token).

## Risks

- With the v2 beta flag on, the v2 builder checks agent capabilities before adding a stdio server, so inject the entry in both paths.
- Claude Code may ask permission for each MCP call. Accept it in V1; R13 handles permissions later.
- `listener.rs` is untested in this tree. Verify the protocol version and the framing against Claude Code early.
- The `await_children` timeout must stay below the MCP client's own tool timeout.
- `worktree: new` puts the child in another workspace or panel. The metadata link works across panels.

## Tests

- **orchestration crate:** the registry state machine; `await_children` with a timeout and with parallel children; `child_status` returns no content; only the final message comes back; log lines cover only orchestrator↔child exchanges; the log dir override.
- **thread_metadata_store:** the migration; the save/list round-trip with a parent; import from an older DB without the column; the existing exclusion test still passes.
- **agent_panel:** `start_child` creates a linked thread; `worktree: new`; an unknown agent gives an error; send and cancel reach the child.
- **sidebar:** metadata children nest under the parent and are absent from the top level; collapse, limit and order work; with the setting off, the list is flat; live subagents still render.
- **agent tools:** native wiring with a fake host; tools gated off when disabled.
- **agent_servers:** the orchestration server is injected only for local projects with orchestration enabled, in v1 and v2.
- **MCP end to end:** the bridge process does `tools/list` and `tools/call start_child` with a fake host.

## Beyond R1 to R5

Implemented after this slice, with the plan's design unchanged (one registry, one host, one MCP server):

| Requirement | What | Commit |
| :-- | :-- | :-- |
| R14 | Child header before the first prompt, and the `whoami` tool | `d303f89` |
| R15 | `send_to_parent` with the inbox (`pending_messages`) and the urgent path | `d303f89` |
| R16 | Human messages in a child are never logged or forwarded; the orchestrator gets an on-demand summary | `d303f89` |
| R17 | Agent profiles with a `description`; `start_child` takes `profile`; the model and profile persist in the thread metadata | `10530cf` |
| R3b | `list_siblings` and `send_to_sibling` with `await_reply`, deadlock detection and a per-pair cap | `cbf577d` |
| R12 | Cascade on close, archive and delete of the orchestrator, and on `cancel_child` of a child that orchestrates | `cbf577d` |
| R10 | `max_concurrent_children` (app-wide cap) and `max_sibling_messages` | `cbf577d` |
| R3 | Registry rebuild after a restart, opening unloaded children on send, queueing while a child connects, token revocation on delete | `cbf577d` |
| R13 | Permission cards of children in the orchestrator thread | `cbf577d` |
| R6 | Subagent quota in five layers (L1 to L5), `child_status` granted and used, the `quota` log direction | `bd64c7b` |
