# Feature Spec: Orchestrated threads

> Status: Draft v0.1 · Branch `feat/orchestrated-threads` · Base: `zed-industries/zed@0be6d59`
> Type: experimental fork of Zed. Nothing here is proposed upstream yet.

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
- as an MCP server that Zed itself serves and injects into every ACP session, next to the user's MCP servers.

**Why:** ACP gives an agent no way to open a thread in the client, so the capability has to live in Zed. Injecting it as MCP is the only path that works for every external agent (Claude Code, Codex, Gemini) without changing those agents. It also makes the requirement symmetric: Claude can start a Zed agent child, the Zed agent can start a Claude child, a GPT-based agent can start a Claude child.

Tools:

| Tool | Input | Returns |
| :-- | :-- | :-- |
| `list_agents_and_models` | none | Available agents, providers and models (reuses the existing tool) |
| `start_child` | `title`, `prompt`, `agent?`, `model?`, `worktree: shared \| new`, `subagent_quota` | `child_id`, `log_path` |
| `send_to_child` | `child_id`, `message` | Acknowledgement |
| `await_children` | `child_ids[]`, `timeout?` | For each child: status and **final message only** |
| `child_status` | `child_ids[]` | Status, last activity time and turn count, without content |
| `cancel_child` | `child_id` | Acknowledgement |

### R2. Children on any agent, provider and model

`start_child` accepts any agent and model that `list_agents_and_models` reports, native or external.

**Why:** this is the core goal, multi-model work. Mixing models lets cheap or local models (Ollama) take bulk work, while stronger models take judgment work.

### R3. Parent-child link and results back

A child stores its orchestrator's thread id. The orchestrator receives the child's final message when the child stops, through `await_children`, and can send follow-ups through `send_to_child`.

**Why:** `create_thread` alone launches threads and forgets them. Orchestration needs both directions.

### R3b. Children talk to each other directly

A child can message a sibling (another child of the same orchestrator) without going through the orchestrator. Children get two tools:

| Tool | Input | Returns |
| :-- | :-- | :-- |
| `list_siblings` | none | `child_id`, title, agent and model, and status of each sibling |
| `send_to_sibling` | `child_id`, `message`, `await_reply?` | Acknowledgement, or the sibling's reply when `await_reply` is set |

Rules:
- Each sibling exchange is logged in the R4 files of **both** children, with `direction: from_sibling` or `to_sibling` and the other child's id.
- The orchestrator does not receive sibling messages in its context. `child_status` reports a count of sibling exchanges, and the content stays in the logs.
- `await_reply` has a timeout. It cannot be used if the sibling is itself waiting on the caller, which prevents deadlock.
- `agent.orchestration.max_sibling_messages` caps the messages per pair of children, which prevents loops.

**Why (decision D1):** children working at the same time often need a quick fact from each other ("which file are you touching?", "what did the API return?"). Routing it through the orchestrator would spend its context and add a round trip. The logs and the counter keep the exchange auditable without the orchestrator reading it.

### R4. Exchange log per child (temporary file)

Zed writes one JSONL file per child at `$TMPDIR/zed-orchestration/<orchestrator-id>/<child-id>.jsonl`. Each line is one exchange between orchestrator and child: timestamp, direction (`to_child` or `from_child`), text and status.

**Why:** this is the record of what the orchestrator asked and what it got back. Only the orchestrator-child exchange goes in, never the child's internal tool calls, so the file stays small and auditable. The child's full transcript remains in Zed's own thread storage, which the sidebar opens.

### R5. Context economy for the orchestrator

No tool returns the child's transcript. `await_children` returns only the final message. `child_status` returns only metadata.

**Why:** if the orchestrator read every detail of its children, its context would fill up and it would stop being able to orchestrate long jobs. The full detail stays available to the human in the sidebar, not to the orchestrator.

### R6. Subagent quota granted by the orchestrator

`start_child` carries `subagent_quota` (an integer, 0 by default). The child may start at most that many subagents or children of its own. When the quota is used up, further spawn calls fail with a clear error.

**Why:** each child can fan out on its own, so cost and concurrency would grow without bound. The orchestrator is the one place that sees the whole job, so it is the one that grants capacity.

Enforcement, in layers (decision D3):

| Layer | Applies to | How | Blocks before start? |
| :-- | :-- | :-- | :-- |
| L1. Zed's own tools | Every agent (`spawn_agent`, `start_child`, `create_thread`) | Zed counts and refuses the call. | Yes |
| L2. Claude Code, quota 0 | Claude Code children | Zed starts the child session with `_meta.claudeCode.options.disallowedTools: ["Agent"]`. The adapter merges this list into the SDK options (`claude-agent-acp`, `src/v1/acp-agent.ts`). | Yes: the tool does not exist for the child |
| L3. Claude Code, quota N > 0 | Claude Code children | Zed passes a `PreToolUse` command hook on `Agent`, through `_meta.claudeCode.options.settings`. The hook decrements a counter file that Zed writes per child and denies the call at zero. | Yes |
| L4. Any ACP agent | Codex, Gemini and others | Zed watches the ACP `subagent_update` announcements. When a child goes over quota, it cancels the extra subagent; the adapter sends `capabilities.cancel` while the subagent is live (`claude-agent-acp`, `src/subagent-replay.ts`). | No: it reacts right after the start |
| L5. Instruction | Every agent | The child's starting prompt states the quota. | No: it depends on the model following it |

L1 to L3 are hard limits. L4 is the generic fallback for agents that offer no way to block, and L5 is the first line for all of them.

Verification status: L2 and L4 were checked by reading the adapter's code, not by running it. The passthrough of `hooks` and `settings` that L3 needs is in the same code path as L2, but no command hook has been tested through it yet.

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
| `agent.orchestration.max_concurrent_children` | int | `6` | Global cap on running children |
| `agent.orchestration.max_sibling_messages` | int | `20` | Cap on messages per pair of children (R3b) |
| `agent.orchestration.log_dir` | path | `$TMPDIR/zed-orchestration` | Where the R4 logs go |

**Why:** each behavior that someone may reasonably want different is a setting, not hard-coded. `max_concurrent_children` is a safety cap that sits above the per-child quota of R6.

### R11. Child threads are read-only to the human in V1

A child thread opens in full view, but it has no message editor. This is already the behavior for subagents (`render_message_editor` in `crates/agent_ui/src/conversation_view/thread_view.rs`).

**Why:** it removes the hardest problem from V1: what happens when the human writes to a child while the orchestrator is also talking to it. The human interacts through the orchestrator.

### R12. Cancellation cascades

Cancelling the orchestrator, or closing its thread, cancels all its running children. Cancelling a child cancels that child's own subagents.

**Why:** without it, background children keep spending tokens after their purpose is gone.

## 5. Out of scope in V1

| Item | Why it waits |
| :-- | :-- |
| The human writing directly in a child | Needs rules for turn-taking between human and orchestrator (planned evolution E1). |
| Children nested more than one level in the sidebar | A child's own subagents do not appear in the sidebar. They stay as cards inside the child (D2). |
| Proposing this upstream | Experimental. Decide after V1 works. |

## 6. Planned evolutions

- **E1.** Interact with children directly: a message editor in the child, with the orchestrator notified of human messages and not interrupting them.
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

When a child asks for permission (`session/request_permission`), the request appears in the orchestrator's thread, labelled with the child's title, agent and model. The human's answer goes back to the child. The child's row in the sidebar shows "waiting for confirmation" in the meantime.

**Why (decision D4):** children are read-only in V1 (R11), so the request cannot be answered inside the child. Blocking silently would stall parallel work without anyone noticing. Approving automatically would remove the human from the loop.

## 8. Expected files touched

| Area | Files |
| :-- | :-- |
| Orchestration tools | `crates/agent/src/tools/` (new `start_child`, `send_to_child`, `await_children`, `child_status` and `cancel_child`; `create_thread_tool.rs` refactored to share the start path) |
| MCP server for ACP agents | New module in `crates/agent_servers`, registered through `mcp_servers_for_project` |
| Parent link and quota | `crates/agent/src/thread.rs`, `crates/acp_thread/src/subagents.rs` |
| Sidebar | `crates/agent_ui/src/thread_metadata_store.rs` (admit children), `crates/sidebar/src/sidebar.rs` (nesting, collapse, limit, load more, sort) |
| Settings | `crates/settings_content/src/agent.rs`, default settings JSON |
| Log | New small module that writes the R4 JSONL |

## 9. Acceptance criteria

- **AC1.** From a Claude Code thread, `start_child` with an Ollama model creates a child that appears under that thread in the sidebar.
- **AC2.** From a Zed agent thread, `start_child` with `agent = claude-code` creates a Claude Code child under that thread.
- **AC3.** `await_children` on two children returns two final messages, and no transcript content beyond them.
- **AC4.** `<child-id>.jsonl` contains one line per orchestrator-child exchange, with nothing from the child's internal tool calls.
- **AC5.** A child started with `subagent_quota: 1` gets an error on its second spawn through Zed's tools.
- **AC6.** With `subthreads_visible: 5` and 7 children, the expanded group shows the 5 with the most recent messages, newest first, plus "Load more". "Load more" reveals the other 2.
- **AC7.** The group is expanded when the orchestrator thread is first shown, and a group the user collapsed with the chevron stays collapsed across rebuilds and thread switches.
- **AC8.** With `show_as_subthreads: false`, the children appear as top-level threads.
- **AC9.** Two children with `worktree: new` run at the same time in separate worktrees, and one with `worktree: shared` runs in the orchestrator's worktree.
- **AC10.** Cancelling the orchestrator stops every running child.
- **AC11.** A child thread opens in full view with no message editor.
- **AC12.** Child A calls `send_to_sibling` to child B. The message appears in both children's logs, and the orchestrator's context receives nothing about it.
- **AC13.** Two children that `await_reply` on each other get a deadlock error instead of hanging.
- **AC14.** A Claude Code child started with `subagent_quota: 0` has no `Agent` tool available.
- **AC15.** A child's permission request appears in the orchestrator thread, and the answer given there reaches the child.

## 10. Risks

| Risk | Mitigation |
| :-- | :-- |
| Upstream moves fast in `agent`, `agent_ui` and `sidebar`, so rebases conflict. | Keep new code in new files; touch existing files only at hook points. |
| The external adapter may not announce its internal subagents, so D3(b) fails. | Fall back to D3(a) and document the gap. |
| `$TMPDIR` is cleared on reboot, so the R4 log is lost. | Acceptable in V1; the thread itself persists. E3 makes the log durable. |
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
