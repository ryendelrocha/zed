# Feature Spec: Orchestrated threads

> Status: Draft v0.2 · Branch `feat/orchestrated-threads` · Base: `zed-industries/zed@0be6d59` · Code state described: `bd64c7b`
> Type: experimental fork of Zed. Nothing here is proposed upstream yet.

## Implementation status

Status of each requirement at `bd64c7b`. "Done" means the behavior in the requirement text exists in the code. Commits: `36ead50` registry, log, parent link and panel host; `b1c2733` native tools, MCP server, bridge, sidebar children; `d303f89` child awareness, inbox, human summary; `10530cf` task profiles; `cbf577d` sibling messages, cascade, caps, restart, permission cards; `bd64c7b` subagent quota. Sidebar nesting is `b7c55ae`, `5921469`, `0d896ef` and `c449f10`.

| ID | Status | Commits |
| :-- | :-- | :-- |
| R1 Orchestration tools for every agent | Done | `36ead50`, `b1c2733` (the tools added later are in R3b, R14, R15) |
| R2 Children on any agent, provider and model | Partial | `b1c2733`, `10530cf` |
| R3 Parent-child link and results back | Done | `36ead50`, `b1c2733`; restart rebuild, opening unloaded children and queueing while connecting in `cbf577d` |
| R3b Children talk to each other directly | Done | `cbf577d` |
| R4 Exchange log per child | Done | `36ead50`; new directions in `d303f89`, `cbf577d`, `bd64c7b` |
| R5 Context economy for the orchestrator | Done | `36ead50`, `b1c2733` |
| R6 Subagent quota | Partial | `bd64c7b` |
| R7 Shared or separate worktree | Done | `36ead50`, `b1c2733` |
| R8 Parallel execution | Done | `b1c2733` |
| R9 Children nested in the sidebar | Done | `b7c55ae`, `5921469`, `0d896ef`, `c449f10`, `b1c2733`, `10530cf` |
| R10 Settings | Done | `36ead50`, `d303f89`, `cbf577d` |
| R11 The human can write in a child | Done | `d303f89` (message origins); spec revised in `1fcdad9` |
| R12 Cancellation cascades | Done | `cbf577d` |
| R13 Child permission requests in the orchestrator | Done | `cbf577d` |
| R14 A child knows it is a child | Done | `d303f89`; quota sentence in `bd64c7b`, sibling sentence in `cbf577d` |
| R15 Child-to-orchestrator inbox | Done | `d303f89` |
| R16 Human intervention summary | Done | `d303f89` |
| R17 Task profiles pick the provider | Done | `10530cf` |
| E1 Interact with children directly | Done | `d303f89` (see R11, R16) |
| E2 Durable log outside `$TMPDIR` | Not started | none |

Counts for R1 to R17 and R3b (18 requirements): 16 Done, 2 Partial, 0 Not started.

What is partial:
- **R2:** any agent can be a child, but `model` is applied only to the native Zed agent. For an external agent child, `start_child` ignores `model` and returns a warning.
- **R6:** all five layers exist. L2 and L3 apply only to Claude Code children and only to a session Zed creates (not one it later loads), L1 and L4 count only the child's own root session, and no layer was run against a real Claude Code session (see R6, "Verification status", and section 12).

## 1. Summary

Let one agent thread (the **orchestrator**) start other agent threads (the **children**) on any agent, provider and model Zed supports. Each child is a real Zed thread that appears in the sidebar nested under its orchestrator. The orchestrator gets back only each child's final result, never the child's full transcript. Children run in parallel, in the same worktree or in their own.

The goal is to use several models in one piece of work, with a single orchestrator managing it, and with every delegated conversation visible for governance.

## 2. Problem

| Today | Why it blocks the goal |
| :-- | :-- |
| An external agent (for example Claude Code over ACP) can only start subagents of its own vendor. Claude Code subagents are always Claude models. | Multi-model work is impossible from Claude Code: it cannot hand a task to Ollama, GPT or the Zed agent. |
| ACP has no method for an agent to open a new thread in the client. The agent-to-client methods are only `fs/*`, `terminal/*`, `session/request_permission` and `session/update`. | An external agent cannot make a delegated conversation visible in Zed, except as a subagent card. |
| Subagent threads are excluded from the sidebar on purpose (`crates/agent_ui/src/thread_metadata_store.rs`, test `test_subagent_threads_excluded_from_sidebar_metadata`). | Delegated conversations are only visible inside the parent's transcript, not as a navigable list. There is no governance view. |
| The native `create_thread` tool opens a sibling thread on any agent and model, but "you will NOT receive its output and you cannot interact with it afterwards" (`crates/agent/src/tools/create_thread_tool.rs`). | The thread is visible, but nobody orchestrates it: no result comes back, there is no parent link, and it is not nested. |
| The native `spawn_agent` tool returns the result, but only for native Zed models (`is_native: true`), with depth capped at 1 (`MAX_SUBAGENT_DEPTH`, `crates/agent/src/thread.rs`). | Results come back, but you cannot use Claude Code, Codex or another external agent as a child, and the child is not in the sidebar. |

## 3. What already exists and is reused

The fork extends existing pieces instead of building a parallel system. This keeps the diff small and keeps the fork rebaseable on upstream.

| Existing piece | Where | Reused for |
| :-- | :-- | :-- |
| `create_thread` (any agent and model, optional new worktree) | `crates/agent/src/tools/create_thread_tool.rs` | Starting a child (R2, R7) |
| `list_agents_and_models` | `crates/agent/src/tools/list_agents_and_models_tool.rs` | Picking the provider and model (R2) |
| `spawn_agent` follow-up by `session_id` | `crates/agent/src/tools/spawn_agent_tool.rs` | Pattern for orchestrator-to-child follow-ups (R3) |
| MCP servers passed to every ACP session | `mcp_servers_for_project`, `crates/agent_servers/src/acp/v1.rs` | Exposing the orchestration tools to external agents (R1) |
| Sidebar project groups with collapse | `crates/sidebar/src/sidebar.rs` | Rendering the nested children (R9) |
| `SubagentAssociation` (parent label, cwd, cancel capability) | `crates/acp_thread/src/subagents.rs` | Data model for the parent-child link (R3) |

## 4. Requirements (V1)

Each requirement states what it does and why.

### R1. Orchestration tools for every agent

Zed exposes the orchestration tools in two ways:
- as native tools in the Zed agent;
- as an MCP server that Zed itself serves and injects into every ACP session, next to the user's MCP servers. An agent can only reach an MCP server over stdio or HTTP, so the injected entry is a stdio command: a hidden mode of the Zed binary (`zed --orchestration-bridge <socket>`) that relays to Zed's local socket. A per-session token, passed in the bridge's environment, tells Zed which thread is calling. The entry is added only for local projects and only when `agent.orchestration.enabled` is on. See `orchestrated-threads-how-it-works.md`.

**Why:** ACP gives an agent no way to open a thread in the client, so the capability has to live in Zed. Injecting it as MCP is the only path that works for every external agent (Claude Code, Codex, Gemini) without changing those agents. It also makes the requirement symmetric: Claude can start a Zed agent child, the Zed agent can start a Claude child, a GPT-based agent can start a Claude child.

Tools:

| Tool | Input | Returns |
| :-- | :-- | :-- |
| `list_agents_and_models` | none | Available agents, providers and models (reuses the existing tool) |
| `start_child` | `title`, `prompt`, `agent?`, `model?`, `profile?` (R17), `worktree: shared \| new`, `base_ref?`, `worktree_name?`, `subagent_quota` (R6) | `child_id`, `log_path`, `agent`, `warning?` |
| `send_to_child` | `child_id`, `message` | `queued`: true when the child was mid-turn or its agent was still connecting |
| `await_children` | `child_ids[]`, `timeout_secs?` | For each child: status and **final message only**, plus `pending_messages` (R15) and `human_interventions` (R16) |
| `child_status` | `child_ids[]` | Status, last activity time, turn count, `sibling_messages` (R3b) and `subagent_quota` granted and used (R6), without content; plus `pending_messages` and `human_interventions` |
| `cancel_child` | `child_id` | Acknowledgement |

Tools added by later requirements: `send_to_parent` (R15), `list_siblings` and `send_to_sibling` (R3b), `whoami` (R14). The native tools and the MCP server expose the same set.

### R2. Children on any agent, provider and model

`start_child` accepts any agent and model that `list_agents_and_models` reports, native or external.

Deviation in the code: `model` (and `profile`, R17) is applied only when the child runs on the native Zed agent. For an external agent child, `start_child` still starts the child but ignores `model` and `profile` and returns a `warning` saying so: the child uses the agent's own configuration. A `model` that is not `provider/model-id` is also ignored with a warning. An unknown agent, or an unknown `profile`, is an error.

**Why:** this is the core goal, multi-model work. Mixing models lets cheap or local models (Ollama) take bulk work, while stronger models take judgment work.

### R3. Parent-child link and results back

A child stores its orchestrator's thread id in its thread metadata (`parent_thread_id`), so the link survives a restart. The orchestrator receives the child's final message when the child stops, through `await_children`, and can send follow-ups through `send_to_child`.

Details in the code:
- A follow-up to a child that is mid-turn, or whose agent is still connecting, is queued and delivered when the child can take it (`queued: true`).
- After a restart, Zed rebuilds the registry from the thread metadata and the child's R4 log. Every child comes back `completed`, unless its log says `failed` or `cancelled`, or ends with a message nobody answered (then `failed`, with a note that Zed restarted). The last final message, the turn count and the sibling counters come from the log. `send_to_child` to a child that is not loaded opens it first.
- When a thread is deleted, Zed revokes the session tokens of its agent, so its MCP connection can no longer call the orchestration tools.

**Why:** `create_thread` alone launches threads and forgets them. Orchestration needs both directions.

### R3b. Children talk to each other directly

A child can message a sibling (another child of the same orchestrator) without going through the orchestrator. Children get two tools:

| Tool | Input | Returns |
| :-- | :-- | :-- |
| `list_siblings` | none | `child_id`, title, agent and model, and status of each sibling |
| `send_to_sibling` | `child_id`, `message`, `await_reply?`, `timeout_secs?` | Acknowledgement, or the sibling's reply when `await_reply` is set |

Rules:
- Both tools exist only for a thread that has an orchestrator. Siblings are the other children of the same orchestrator.
- The message reuses the R15 inbox. The receiving sibling gets it as a new message in its thread, prefixed `[from sibling <title>]`, only when it is idle. It never interrupts a running turn; a busy sibling gets it when its turn ends. A sibling that is not loaded (after a restart) is opened first.
- Each sibling exchange is logged in the R4 files of **both** children, with `direction: from_sibling` or `to_sibling` and the other child's id in `sibling`.
- The orchestrator does not receive sibling messages in its context. `child_status` reports `sibling_messages`, a count per child, and the content stays in the logs.
- `await_reply` waits for the sibling's **next `send_to_sibling` addressed to the caller**, up to `timeout_secs` (default 120). The delivered message tells the sibling to answer with that tool. That message is returned to the caller as the reply and is not also added to the caller's thread. On timeout the call returns `timed_out: true` (the message was still delivered). The alternative, taking the sibling's final message of its next turn, was rejected: a busy sibling would answer with the end of a turn that started before the question, and the final message already belongs to the orchestrator (R5).
- Deadlock: `await_reply` fails with an error to the second waiter when the target already waits for the caller, directly or through a chain of siblings. Nothing is sent. The way out is a plain `send_to_sibling` (no `await_reply`), which is the reply that unblocks the first waiter.
- `agent.orchestration.max_sibling_messages` (default 20) caps the messages per pair of children, in both directions, which prevents loops. After the cap the call fails with an error that points to `send_to_parent`.

**Why (decision D1):** children working at the same time often need a quick fact from each other ("which file are you touching?", "what did the API return?"). Routing it through the orchestrator would spend its context and add a round trip. The logs and the counter keep the exchange auditable without the orchestrator reading it.

### R4. Exchange log per child (temporary file)

Zed writes one JSONL file per child at `$TMPDIR/zed-orchestration/<orchestrator-id>/<child-id>.jsonl`. Each line is one exchange between orchestrator and child: timestamp, direction (`to_child` or `from_child`), text and status. Later requirements add directions: `to_parent` (R15), `human_intervention` (R16, the summary only, never the human's messages), `to_sibling` and `from_sibling` with a `sibling` field (R3b), and `quota` (R6). The directory comes from `agent.orchestration.log_dir` (R10).

**Why:** this is the record of what the orchestrator asked and what it got back. Only the orchestrator-child exchange goes in, never the child's internal tool calls, so the file stays small and auditable. The child's full transcript remains in Zed's own thread storage, which the sidebar opens.

### R5. Context economy for the orchestrator

No tool returns the child's transcript. `await_children` returns only the final message. `child_status` returns only metadata.

**Why:** if the orchestrator read every detail of its children, its context would fill up and it would stop being able to orchestrate long jobs. The full detail stays available to the human in the sidebar, not to the orchestrator.

### R6. Subagent quota granted by the orchestrator

`start_child` carries `subagent_quota` (an integer, 0 by default). The child may start at most that many subagents or children of its own. When the quota is used up, further spawn calls fail with a clear error. The quota applies only to orchestrated children: a thread that no orchestrator started has none.

**Why:** each child can fan out on its own, so cost and concurrency would grow without bound. The orchestrator is the one place that sees the whole job, so it is the one that grants capacity.

The quota only counts and blocks. It grants nothing: a child with quota N gets no access it did not have before.

Enforcement, in layers (decision D3):

| Layer | Applies to | How | Blocks before start? |
| :-- | :-- | :-- | :-- |
| L1. Zed's own tools | Any child (`spawn_agent`, `start_child`, `create_thread`) | Zed counts each spawn per child and refuses the call with a "Subagent quota exhausted" error. A start that then fails is refunded. | Yes |
| L2. Claude Code, quota 0 | Claude Code children | Zed starts the child session with `_meta.claudeCode.options.disallowedTools: ["Agent", "Task"]`. The adapter merges this list into the SDK options (`claude-agent-acp`, `src/v1/acp-agent.ts`). Both names are listed because Claude Code renamed `Task` to `Agent` and the adapter treats both as the subagent tool. | Yes: the tool does not exist for the child |
| L3. Claude Code, quota N > 0 | Claude Code children | Zed passes a `PreToolUse` command hook, with matcher `Agent\|Task`, through `_meta.claudeCode.options.settings.hooks`. Zed writes N ticket files in a per-child directory, `<log_dir>/<orchestrator-id>/<child-id>.quota/`. The hook takes one ticket with `rm` (it succeeds for exactly one of any number of concurrent hooks) and allows the call; with none left it writes a message to stderr and exits with code 2, which Claude Code treats as a blocking error. L1 takes tickets from the same directory, so Zed's tools and the hook share one budget. | Yes |
| L4. Any ACP agent | Codex, Gemini and others | Zed watches the ACP `subagent_update` announcements of the child's session. Subagents announced beyond the quota are cancelled when the subagent is running and the agent advertised `capabilities.cancel` for it (`claude-agent-acp`, `src/subagent-replay.ts`). Replayed history is never cancelled. Without that capability, the event is only logged. | No: it reacts right after the start |
| L5. Instruction | Every agent | The child's header states the quota: none (0), one, or N subagents, and that more are refused. | No: it depends on the model following it |

L1 to L3 are hard limits. L4 is the generic fallback for agents that offer no way to block, and L5 is the first line for all of them. L2 and L3 apply only to Claude Code children, only in the session Zed creates for the child (the quota meta is sent in `session/new`), and L1 and L4 look only at the child's own root session.

Why the hook travels in `settings`: the adapter's own `hooks` option accepts JavaScript callbacks only, which cannot cross the ACP boundary. A command hook has to be passed as Claude Code settings.

Reporting: `child_status` returns `subagent_quota: { granted, used }` for each child. `used` is the largest of the counts the layers keep, because one subagent can be seen by more than one layer (a Claude Code `Agent` call passes the hook and is then announced). The R4 log gets entries with `direction: quota`: the grant (`subagent_quota granted: N`), each spawn counted or refused, and each over-quota subagent that Zed cancelled or could not cancel. After a restart, the grant and the use are rebuilt from those log lines.

Verification status: L2 and L4 were checked by reading the adapter's code, and L3 by reading the adapter's code and Claude Code's hook documentation. None was run end to end against a real Claude Code session. Unit tests assert the `_meta` Zed sends for quota 0 and quota N, run the hook command in a shell (N calls allowed, then exit code 2), count spawns of Zed's tools with refund, and, for the ACP v2 connection, cancel and log over-quota subagents. The ACP v1 path calls the same L4 check but has no test of its own.

### R7. Shared or separate worktree

`start_child` takes `worktree: shared | new`. `new` reuses the `create_thread` worktree path, including `base_ref` and `worktree_name`.

**Why:** children work at the same time. A shared worktree is cheaper but needs disjoint files. A new worktree isolates risky or overlapping work. The orchestrator picks per child, because it knows the write set of each task.

### R8. Parallel execution

Children run concurrently. `start_child` returns right away, and `await_children` waits on several children at once.

**Why:** most of the value comes from parallelism. A blocking `start_child` would serialize the work.

### R9. Children nested in the sidebar

When `agent.orchestration.show_as_subthreads` is on:
- each child appears in the sidebar under its orchestrator thread, with a disclosure arrow;
- the group is **expanded by default**, and the user can collapse it with the chevron;
- an expanded group shows the **N most recent children** (N = `agent.orchestration.subthreads_visible`, default 5), followed by a **"Load more"** row;
- children are **sorted by the time of their last message, most recent first**;
- each child row reuses the thread row's status icons (running, waiting for confirmation, error, done).

When the setting is off, children appear as ordinary top-level threads, which is today's `create_thread` behavior.

**Why:** this is the governance view. You can see at a glance what was delegated, to which model and in what state, and open any child's transcript. Collapsing and the visible limit keep a large orchestration from flooding the sidebar. Recency sorting puts active work on top.

### R10. Settings

| Key | Type | Default | Purpose |
| :-- | :-- | :-- | :-- |
| `agent.orchestration.enabled` | bool | `true` | Expose the orchestration tools (R1) |
| `agent.orchestration.show_as_subthreads` | bool | `true` | Nest children under the orchestrator in the sidebar (R9) |
| `agent.orchestration.subthreads_visible` | int | `5` | Children shown before "Load more" (R9) |
| `agent.orchestration.max_concurrent_children` | int | `6` | Global cap on running children, across every orchestrator in the app. A child counts from its start until its turn finishes, fails or is cancelled. `start_child` beyond it fails with an error that states the count |
| `agent.orchestration.max_sibling_messages` | int | `20` | Cap on messages per pair of children (R3b) |
| `agent.orchestration.human_summary_max_chars` | int | `600` | Length cap of the human-intervention summary (R16) |
| `agent.orchestration.log_dir` | path | `null` (uses `zed-orchestration` in the system temporary directory) | Where the R4 logs and the R6 quota tickets go |

All seven keys exist in `assets/settings/default.json` with these defaults, and each has an item in the Settings UI (`crates/settings_ui/src/page_data.rs`).

**Why:** each behavior that someone may reasonably want different is a setting, not hard-coded. `max_concurrent_children` is a safety cap that sits above the per-child quota of R6.

### R11. The human can write in a child thread (revised)

Orchestrated children are ordinary Zed threads, so they keep their message editor and the human can talk to a child directly. (Agent-owned subagents stay read-only, as upstream.) What the orchestrator learns about that conversation is defined by R16. The code tags each message with its origin (human or orchestration) so R16 can tell them apart; no test asserts the editor itself.

**Why (revised after testing):** the original V1 made children read-only to avoid turn-taking problems. In practice, steering a child directly is useful, and R16 gives the orchestrator what it needs to continue without reading the human's messages.

### R12. Cancellation cascades (revised)

Cancelling the orchestrator's running turn does **not** cancel its children: they run in parallel by design, and the human often stops the orchestrator only to talk to it. Children are cancelled, deepest first, in these cases:
- the orchestrator's thread is **closed, archived or deleted** (archive is how Zed closes a thread; delete is `remove_thread`);
- `cancel_child` is called on a child that itself orchestrates: its running children are cancelled too, even when its own turn had already ended. Its siblings are not touched.

Cancelling a child also cancels its own agent-owned subagents, through Zed's existing cancellation of the child's session (the native thread cancels its running subagents, and ACP agents that announce subagents get the cancel for them). Closing the window or workspace is not a trigger.

**Why:** without it, background children keep spending tokens after their purpose is gone. Tying the cascade to the thread's life, and not to its turn, keeps the parallel work alive while the human steers the orchestrator.

## 5. Out of scope in V1

| Item | Why it waits |
| :-- | :-- |
| Children nested more than one level in the sidebar | A child's own subagents do not appear in the sidebar. They stay as cards inside the child (D2). |
| Proposing this upstream | Experimental. Decide after V1 works. |

## 6. Planned evolutions

- **E1.** ~~Interact with children directly.~~ Done earlier than planned: see R11 and R16.
- **E2.** A durable log outside `$TMPDIR`, kept with the thread metadata.

## 7. Decisions

| ID | Decision | Outcome |
| :-- | :-- | :-- |
| D1 | How children talk to each other | **Directly**, sibling to sibling, without going through the orchestrator (R3b). |
| D2 | Sidebar depth | **One level.** A child's own subagents do not appear in the sidebar. |
| D3 | Quota on an external agent's internal subagents | **Layered enforcement** L1 to L5 (R6). |
| D4 | Who approves a child's permission requests | **The orchestrator thread.** The request appears there, and the human decides from one place (R13). |
| D5 | Upstream path | See section 11. |

### R13. Child permission requests surface in the orchestrator

When a child asks for permission (`session/request_permission`), the request appears in the orchestrator's thread as a card, labelled with the child's title, agent and model. The human's answer on the card goes to the child exactly as if it had been given in the child. The child's row in the sidebar shows "waiting for confirmation" in the meantime.

How the cards behave in the code:
- One compact card per pending request, in a block above the orchestrator's own awaiting-confirmation prompts (a divider separates them). The block scrolls above 320 px.
- A card shows the tool and a short rendering of its input (at most 240 characters), and the buttons the child's own request offers: **Allow** (tinted with the success color), "always" when the request offers it, and **Deny**. For a generic request (not a tool call), the buttons are its options, plus cancel.
- An **Open** button shows the child thread in the agent panel of the workspace that holds it, to answer there. It does nothing when the child is no longer open.
- A card disappears when the request is answered, here or in the child.
- Only the orchestrator's direct children are listed, and only requests of each child's own root session. A grandchild's request appears in its own orchestrator (the child), and a child's agent-owned subagents keep their existing surface inside the child.
- Nothing answers on its own: Zed never approves a child's request automatically.

**Why (decision D4):** the human is usually watching the orchestrator, not each child. Blocking silently would stall parallel work without anyone noticing. Approving automatically would remove the human from the loop.

### R14. A child knows it is a child

When Zed starts a child, it puts a fixed header before the child's first prompt: the child is a child conversation started by the orchestrator "‹orchestrator title›"; its final message goes back to the orchestrator automatically; it can message the orchestrator during the task with `send_to_parent`. A `whoami` tool, available to every thread, returns its role (orchestrator, child or both), its parent and its children.

**Why:** in testing, a child asked to talk to its orchestrator answered in its own chat, because nothing told it that it had a parent or how to reach it.

### R15. Child-to-orchestrator messages through an inbox

Children get a `send_to_parent(message, urgent?)` tool, both as a native tool and through the MCP server. A message:
- goes into the orchestrator's inbox and into the child's R4 log, with `direction: to_parent`;
- is returned to the orchestrator as `pending_messages` on its next `await_children` or `child_status` call, then marked delivered;
- with `urgent: true`, is also added to the orchestrator's thread as a new message, but only when the orchestrator is idle. It never interrupts a running turn. If the orchestrator is blocked in `await_children`, an urgent message ends that wait early (the other children keep running), because the orchestrator could not see the message any other way. This goes beyond the first design.

**Why:** the orchestrator must hear from a child mid-task (a blocker, a question) without its context filling up, and without being interrupted (R5). The urgent path covers "I am blocked and need a decision". The same inbox later serves sibling messages (R3b).

### R16. Human intervention in a child reaches the orchestrator as a summary

When the human writes directly in a child (R11), the human's messages are **not** logged and **not** sent to the orchestrator. Instead, the orchestrator receives a **summary of the human intervention**: that a human intervened, what was adjusted, and why.
- The summary is generated **on demand only**: when the orchestrator next calls `await_children` or `child_status`. It is not written eagerly when the child finishes (the first design also did that). It covers every human-driven turn since the previous summary, so hundreds of interactions become one short summary.
- Zed writes it with the existing `agent.thread_summary_model`, capped at `agent.orchestration.human_summary_max_chars` (default 600). When no model is available, it falls back to a minimal note stating how many human turns there were.
- It is logged in the child's R4 log with `direction: human_intervention`, and returned to the orchestrator as `human_interventions` next to the child's status.

**Why:** a human may steer a child through hundreds of turns. The orchestrator does not need the process, only what the human changed and why, so it can continue coherently. Generating the summary on demand means no model call when the orchestrator never asks.

### R17. Task profiles pick the provider

Agent profiles (`agent.profiles`, which already have a name, a tool set and a `default_model`) gain a `description`: when to use the profile. `start_child` gains an optional `profile`. `list_agents_and_models` also returns the profiles with their descriptions, so the orchestrator picks a profile by matching the task to the descriptions. The child's model is resolved in this order: an explicit `model`; the profile's `default_model`; `agent.subagent_model`; the user's default model. The profile's tool set also applies to the child. The child's model and profile are stored in its thread metadata (`run_config`), so the sidebar shows the model and provider icon even when the child is not loaded. The sidebar labels a nested child row "Profile · model" (just the model without a profile). The profile `description` is set in the profile creation and fork form of the agent settings (it is not editable on an existing profile in that form), shown in the profile list, and can also be written in `settings.json`. `start_child` with an unknown `profile` is an error that lists the available ids. For an external agent child, `profile` is ignored with a warning (see R2).

**Why:** today the user must name the model in every request. Reusing profiles adds one field and one parameter, with no new settings section or UI. Profiles cover every provider the native Zed agent supports; routing a profile to an external agent (an `agent` field) is a later step.

## 8. Expected files touched

| Area | Files |
| :-- | :-- |
| Orchestration tools | `crates/agent/src/tools/` (new `start_child`, `send_to_child`, `await_children`, `child_status` and `cancel_child`; `create_thread_tool.rs` refactored to share the start path) |
| MCP server for ACP agents | New module in `crates/agent_servers`, registered through `mcp_servers_for_project` |
| Parent link and quota | `crates/agent/src/thread.rs`, `crates/acp_thread/src/subagents.rs` |
| Sidebar | `crates/agent_ui/src/thread_metadata_store.rs` (admit children), `crates/sidebar/src/sidebar.rs` (nesting, collapse, limit, load more, sort) |
| Settings | `crates/settings_content/src/agent.rs`, default settings JSON |
| Log | New small module that writes the R4 JSONL |

This table is the original plan. The files actually changed are listed one by one in `manual-review.md`.

## 9. Acceptance criteria

Status column: **Met (tests)** means a test in the code asserts the behavior; **Not verified** means no test or manual check covers the full criterion. The status was taken from the tests present at `bd64c7b`; the doc update did not run them. Real Claude Code, Codex or Ollama sessions were not exercised by any test, so criteria that name them are not verified end to end.

| ID | Criterion | Status | Evidence |
| :-- | :-- | :-- | :-- |
| AC1 | From a Claude Code thread, `start_child` with an Ollama model creates a child that appears under that thread in the sidebar. | Not verified | The MCP `start_child` call is tested with a fake host (`crates/orchestration/src/mcp.rs`), and a linked child with a native model is tested in `agent_panel.rs` (`test_orchestration_start_child_creates_linked_thread`); a real Claude Code session and Ollama were not run. |
| AC2 | From a Zed agent thread, `start_child` with `agent = claude-code` creates a Claude Code child under that thread. | Not verified | Panel tests use stub agents only. |
| AC3 | `await_children` on two children returns two final messages, and no transcript content beyond them. | Met (tests) | `test_orchestration_await_children_returns_only_final_messages` |
| AC4 | `<child-id>.jsonl` contains one line per orchestrator-child exchange, with nothing from the child's internal tool calls. | Met (tests) | `test_orchestration_log_contains_only_orchestrator_child_lines` |
| AC5 | A child started with `subagent_quota: 1` gets an error on its second spawn through Zed's tools. | Met (tests) | `test_orchestration_child_starts_children_only_within_its_quota`, `test_native_spawns_are_counted_against_the_subagent_quota` |
| AC6 | With `subthreads_visible: 5` and 7 children, the expanded group shows the 5 with the most recent messages, newest first, plus "Load more". "Load more" reveals the other 2. | Met (tests) | `test_subagent_threads_respect_visible_limit_and_load_more`, `test_metadata_children_respect_limit_load_more_and_message_order` |
| AC7 | The group is expanded when the orchestrator thread is first shown, and a group the user collapsed with the chevron stays collapsed across rebuilds and thread switches. | Met (tests) | `test_subagent_threads_nest_under_parent_expanded_by_default`, `test_collapsed_subagent_group_stays_collapsed_across_thread_switches` |
| AC8 | With `show_as_subthreads: false`, the children appear as top-level threads. | Met (tests) | `test_metadata_children_are_ordinary_threads_when_setting_is_off` |
| AC9 | Two children with `worktree: new` run at the same time in separate worktrees, and one with `worktree: shared` runs in the orchestrator's worktree. | Not verified | Only the failure of `worktree: new` outside a git repository is tested (`test_orchestration_start_child_in_new_worktree_without_git_reports_the_error`). |
| AC10 | Cancelling the orchestrator's running turn leaves its children running. Closing, archiving or deleting the orchestrator thread stops every running child, and `cancel_child` on a child that orchestrates stops that child's running children. | Met (tests) | `test_orchestration_cancelling_the_orchestrator_turn_keeps_children_running`, `..._removing_the_orchestrator_cancels_its_running_children`, `..._archiving_the_orchestrator_cancels_its_running_children`, `..._cancel_child_cancels_the_children_of_a_child_that_orchestrates` |
| AC11 | An orchestrated child thread opens with its message editor, so the human can write in it (R11). The original V1 criterion, "no message editor", is replaced. | Not verified | No test asserts the editor. Human messages typed in a child are tested for R16. |
| AC12 | Child A calls `send_to_sibling` to child B. The message appears in both children's logs, and the orchestrator's context receives nothing about it. | Met (tests) | `test_sibling_message_is_delivered_and_logged_in_both_logs` (registry), `test_orchestration_sibling_message_reaches_an_idle_sibling_and_never_interrupts` (panel) |
| AC13 | Two children that `await_reply` on each other get a deadlock error instead of hanging. | Met (tests) | `test_mutual_await_reply_is_a_deadlock_for_the_second_waiter` |
| AC14 | A Claude Code child started with `subagent_quota: 0` has no `Agent` tool available. | Not verified | A test asserts the `_meta` with `disallowedTools` that Zed sends (`test_quota_zero_meta_disallows_the_claude_code_subagent_tool`); the tool's absence in a real session was not checked. |
| AC15 | A child's permission request appears in the orchestrator thread, and the answer given there reaches the child. | Met (tests) | `test_orchestrated_child_permission_card_allow_resolves_the_child`, `..._deny_resolves_the_child`, `..._answered_in_the_child_removes_the_card` |
| AC16 | R3b: a sibling message waits for an idle thread and never interrupts a running turn; siblings are only the other children of the same orchestrator. | Met (tests) | `test_sibling_message_waits_for_an_idle_thread`, `test_siblings_are_the_other_children_of_the_same_orchestrator` |
| AC17 | R3b: messages per pair of children are capped by `max_sibling_messages` in both directions, and `await_reply` returns the sibling's reply or times out with `timed_out: true`. | Met (tests) | `test_sibling_messages_per_pair_are_capped`, `test_await_reply_returns_the_siblings_next_message_to_the_caller`, `test_await_reply_times_out_and_stops_blocking` |
| AC18 | R10, R3: `start_child` beyond `max_concurrent_children` fails with the count; the registry is rebuilt from metadata and the log after a restart, and `send_to_child` opens an unloaded child; deleting a thread revokes its session tokens. | Met (tests) | `test_orchestration_start_child_stops_at_the_concurrency_cap`, `test_orchestration_registry_is_rebuilt_and_an_unloaded_child_is_opened_by_send_to_child`, `test_session_tokens_are_revoked_with_their_thread` |
| AC19 | R6 L1: Zed's tools count spawns per child, refuse the one over the quota, and give a spawn back when the start fails. `child_status` reports granted and used. | Met (tests) | `test_consume_refuses_beyond_the_quota_with_a_clear_error`, `test_refund_gives_the_spawn_back`, `test_child_status_reports_the_quota_used_and_granted` |
| AC20 | R6 L2 and L3: for a Claude Code child, quota 0 sends `disallowedTools` for `Agent` and `Task`; quota N sends a `PreToolUse` command hook and N tickets; the hook allows N calls and then exits with code 2. | Not verified | The `_meta` and the hook command run in a shell are tested (`test_quota_n_meta_carries_a_pre_tool_use_command_hook_and_tickets`, `test_hook_command_allows_n_calls_then_blocks_with_exit_code_2`); Claude Code was not run with them. |
| AC21 | R6 L4: with the ACP v2 connection, a subagent announced beyond the quota is cancelled once when the agent advertised `capabilities.cancel`, and only logged otherwise. | Met (tests) for v2; Not verified for v1 | `v2_subagents_over_the_quota_are_cancelled_and_logged`, `v2_subagents_over_the_quota_are_not_cancelled_without_the_capability`; the v1 path has no test. |
| AC22 | R6 L5: the child header states the quota. | Met (tests) | `test_child_header_states_the_quota`, `test_orchestration_header_states_the_quota_and_child_status_reports_it` |
| AC23 | R13: one card per waiting child, in the orchestrator only; a grandchild's request shows only in its own orchestrator. | Met (tests) | `test_two_orchestrated_children_waiting_show_one_card_each`, `test_grandchild_permission_shows_only_in_its_own_orchestrator` |
| AC24 | R14 and R15: a child gets the header and `whoami`; `send_to_parent` reaches the inbox, comes back once as `pending_messages`, and an urgent message is delivered as a new message only when the orchestrator is idle and wakes a pending `await_children`. | Met (tests) | `test_orchestration_child_gets_a_header_before_its_first_prompt`, `test_orchestration_whoami_and_send_to_parent_through_the_host`, `test_orchestration_urgent_message_waits_for_an_idle_orchestrator`, `test_urgent_message_wakes_await_children` |
| AC25 | R16: a human's messages in a child are neither logged nor forwarded; the orchestrator gets one summary on demand, capped at `human_summary_max_chars`, with a minimal note when no model is set. | Met (tests) | `test_orchestration_human_message_is_neither_logged_nor_forwarded`, `test_orchestration_human_turns_become_one_summary_on_demand`, `test_orchestration_summary_respects_the_char_cap`, `test_orchestration_summary_without_a_model_is_a_minimal_note` |
| AC26 | R17: the child's model resolves as explicit `model`, profile `default_model`, `agent.subagent_model`, user default; the profile's tool set applies; an unknown profile lists the valid ids; a profile is ignored with a warning for an external agent. | Met (tests) | `test_orchestration_child_model_resolution_order`, `test_orchestration_child_gets_the_profile_and_its_tool_set`, `test_orchestration_unknown_profile_lists_the_available_ids`, `test_orchestration_external_agent_ignores_the_profile_with_a_warning` |

## 10. Risks

| Risk | Mitigation |
| :-- | :-- |
| Upstream moves fast in `agent`, `agent_ui` and `sidebar`, so rebases conflict. | Keep new code in new files; touch existing files only at hook points. |
| The external adapter may not announce its internal subagents, so L4 sees nothing. | L1 and L5 still apply, and L2 and L3 for Claude Code. The gap is documented in section 12. |
| `$TMPDIR` is cleared on reboot, so the R4 log is lost. | Acceptable in V1; the thread itself persists. E2 makes the log durable. |
| Many parallel children on paid models drive up cost. | `subagent_quota`, `max_concurrent_children` and status visible in the sidebar. |
| Sibling messages loop or deadlock. | `max_sibling_messages`, `await_reply` timeout and mutual-wait detection (R3b). |

## 11. Upstream path

Source: `CONTRIBUTING.md` and `docs/src/development/feature-process.md` in the Zed repository, read on 2026-11-24.

### What Zed requires

- **Discuss first.** Without a GitHub issue that staff confirmed, a feature starts as a GitHub Discussion, not a PR. Feature PRs that skip this "have a very low merge rate".
- **One thing per PR**, with tests, built and tested locally, and with screenshots or video for visual changes.
- **No giant changes.** "Features whose complexity outweighs their benefit" and "giant refactorings" are typically declined.
- **At most three open PRs** per contributor.
- **Contributor License Agreement** signed (zed.dev/cla).
- **AI policy.** LLM-assisted code is accepted with a human who understands it. Contributions from autonomous agents are not. PR descriptions and replies to maintainers must come from the human. A machine translation goes in a quote block, followed by the original text.

### Existing demand (open discussions, none answered by staff yet)

| Discussion | Upvotes | Overlap with this spec |
| :-- | :-- | :-- |
| #49452 Surface Claude Code subagent/team activity in the agent panel | 29 | R9 (children visible) |
| #61001 Agent Orchestration Layer | 5 | R1, R2, R3 |
| #62933 Cross-session agent collaboration: messaging and context handoff | 5 | R3b |
| #63207 RFC: Autonomous Multi Agent Delegation and Collaboration via ACP | 4 | R1, R6 |

### Proposed sequence

1. **Discussion.** Comment on #49452 and #61001 with this proposal in the feature-process format, written by the owner, and link the fork with a video.
2. **PRs, one per piece, only after staff interest:**
   - **PR 1.** Nested children in the sidebar, with the settings of R9 and R10 (small, and the most requested).
   - **PR 2.** Parent link and results back for `create_thread` (R3).
   - **PR 3.** Orchestration tools exposed to external agents through MCP (R1).
   - **PR 4.** Sibling messaging (R3b) and quota (R6).
3. **Meanwhile, the fork is used internally** and rebased on upstream.

## 12. Known limitations

Open issues of the implementation at `bd64c7b`. Each one is stated in the code, in a code comment, in a test or in a commit message, or follows directly from the code path named.

| Limitation | Where it comes from |
| :-- | :-- |
| Closing a window or workspace does not cancel children. Only closing, archiving or deleting the orchestrator's thread, and `cancel_child` on a child that orchestrates, cascade (R12). | R12 text and the cascade hooks in `crates/agent_ui/src/agent_panel.rs`. |
| The R4 logs live in `$TMPDIR` by default, which is cleared on reboot. After a reboot a restored child has no final message, no turn count, no sibling counters and no quota use. | Plan step 7, `restore_child` in `crates/orchestration/src/registry.rs`, and risk 4 of section 10. E2 is not started. |
| After a restart, a child started with `worktree: new` that lives in another workspace's agent panel cannot be opened from the orchestrator's panel by `send_to_child` when that panel already holds it. | The error text in `open_child` in `crates/agent_ui/src/orchestration_host.rs`, and plan step 7. |
| A Claude Code child that is resumed or loaded later does not get the quota `_meta` again (L2 and L3), because it is sent only in `session/new`. | `quota_session_meta` is called only from the `new_session` paths in `crates/agent_servers/src/acp/v1.rs` and `v2.rs`. |
| L1 and L4 count only the child's own root session. Subagents of a subagent are not counted by L4, and L1 counts the calls of the child's own root session only. | `enforce_subagent_quota` and `count_toward_subagent_quota` resolve the owning child from the root session id. |
| The ACP v1 path of L4 has no test, and none of L2 to L4 was run against a real Claude Code session (R6, "Verification status"). | Tests in `crates/agent_servers/src/acp/v2.rs` and `crates/orchestration/src/quota.rs` only. |
| `model` is not supported for an external agent child, and `profile` is ignored for one (R2, R17). Both return a warning. | `crates/agent_ui/src/orchestration_host.rs` and its tests. |
| The stdio bridge exists only on Unix (`#[cfg(unix)]` in `crates/orchestration/src/bridge.rs`). | `bridge.rs`. |
| Claude Code may ask the human for permission on each `mcp__zed-orchestration__*` call. | Plan, "Risks": accepted in V1. |
| The session token sits in the environment of the bridge process. A model with shell access on the same machine could read it (for example with `ps eww`). It only lets that caller act on its own orchestrator's children. | `orchestrated-threads-how-it-works.md`, and `bridge.rs`. |
| Orchestrated children nest one level in the sidebar (D2). | Section 5. |
