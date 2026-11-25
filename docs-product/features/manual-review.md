# Manual review

This file lists every file changed for orchestrated threads, so a human reviewer can review each one individually before anything is proposed upstream to Zed. A checkmark in the "Manual review" column means a human reviewed that file; an empty cell means it has not been reviewed yet. The fork owner fills this column, nobody else.

Base commit: `0be6d59`. Last commit covered: `10530cf` (inclusive). Requirements are defined in [orchestrated-threads.md](orchestrated-threads.md) (R1 to R17).

## Files

| File | Change | Requirements | Manual review |
| :-- | :-- | :-- | :-- |
| [assets/settings/default.json](../../assets/settings/default.json) | Adds the `agent.orchestration` defaults (incl. `human_summary_max_chars`), profile descriptions and the new tools in `write`. | R9, R10, R16, R17 | |
| [crates/agent/src/tool_permissions.rs](../../crates/agent/src/tool_permissions.rs) | Adds the new `orchestration` field (with `human_summary_max_chars`) to a test `AgentSettings` literal so it compiles. | R10, R16 | |
| [crates/agent_settings/src/agent_settings.rs](../../crates/agent_settings/src/agent_settings.rs) | Adds `OrchestrationSettings` (incl. `human_summary_max_chars`) to `AgentSettings` and fills it from the settings content. | R10, R16 | |
| [crates/agent_ui/src/agent_ui.rs](../../crates/agent_ui/src/agent_ui.rs) | Re-exports `SubthreadInfo` and `model_icon_parts`, declares `orchestration_host`, updates test settings and the stub hook. | R9, R10 | |
| [crates/agent_ui/src/conversation_view.rs](../../crates/agent_ui/src/conversation_view.rs) | Adds `SubthreadInfo` (with model, thread id), `subthreads()`, `model_icon_parts`, `load_error_message` and `send_follow_up`. | R3, R9, R17 | |
| [crates/settings_content/src/agent.rs](../../crates/settings_content/src/agent.rs) | Adds `OrchestrationSettingsContent` (five optional fields), the profile `description` and the new tool names. | R10, R16, R17 | |
| [crates/settings_ui/src/page_data.rs](../../crates/settings_ui/src/page_data.rs) | Adds five Settings UI items for the orchestration settings; `general_section` grows from 8 to 13 items. | R10, R16 | |
| [crates/sidebar/Cargo.toml](../../crates/sidebar/Cargo.toml) | Adds the `watch` workspace dependency, used only by the new sidebar tests. | R9 | |
| [crates/sidebar/src/sidebar.rs](../../crates/sidebar/src/sidebar.rs) | Adds subthread rows, metadata children, rename, model label and persisted provider icon, collapse state and the active-row logic. | R9, R10, R17 | |
| [crates/sidebar/src/sidebar_tests.rs](../../crates/sidebar/src/sidebar_tests.rs) | Adds tests for nesting, limits, status, collapse, stable order, metadata children, rename, model label and provider icon. | R9, R17 | |
| [crates/sidebar/src/subthreads.rs](../../crates/sidebar/src/subthreads.rs) | New module that collects live and metadata children per parent, picks the visible ones and inserts the nested rows. | R9 | |
| [crates/ui/src/components/ai/thread_item.rs](../../crates/ui/src/components/ai/thread_item.rs) | Adds `indented()`, a right-aligned `disclosure()` chevron and a `model()` label to `ThreadItem`. | R9, R17 | |
| [docs/src/reference/all-settings.md](../../docs/src/reference/all-settings.md) | Documents the five `agent.orchestration` settings and the profile `description` in the settings reference. | R10, R16, R17 | |
| [Cargo.lock](../../Cargo.lock) | Adds the `orchestration` package and its dependencies, `orchestration` for agent, agent_servers and zed, and `watch` for sidebar. | R1, R2, R3, R4, R5 | |
| [Cargo.toml](../../Cargo.toml) | Adds `crates/orchestration` to the workspace members and the workspace dependencies. | R1, R2, R3, R4, R5 | |
| [README.md](../../README.md) | Adds the two-line AGENTS.md mandatory review notice at the top; remove it manually after review. | R1, R2, R3, R4, R5 | |
| [crates/agent_ui/Cargo.toml](../../crates/agent_ui/Cargo.toml) | Adds the `orchestration` workspace dependency. | R1, R3 | |
| [crates/agent_ui/src/agent_panel.rs](../../crates/agent_ui/src/agent_panel.rs) | Registers the host, extracts `create_child_thread`, links children, applies profile and model, persists the run config. | R1, R2, R3, R9, R17 | |
| [crates/agent_ui/src/conversation_view/thread_view.rs](../../crates/agent_ui/src/conversation_view/thread_view.rs) | Adds `send_follow_up`, `active_model`, and message origins that record human and orchestration turns. | R3, R16, R17 | |
| [crates/agent_ui/src/orchestration_host.rs](../../crates/agent_ui/src/orchestration_host.rs) | New `AgentPanelOrchestrationHost`: child tracking, child header, inbox delivery, human summaries and profile resolution. | R1, R2, R3, R4, R5, R14, R15, R16, R17 | |
| [crates/agent_ui/src/test_support.rs](../../crates/agent_ui/src/test_support.rs) | Adds a thread-local override so tests can make `Agent::Stub` use a scripted connection. | R1 | |
| [crates/agent_ui/src/thread_import.rs](../../crates/agent_ui/src/thread_import.rs) | Sets `parent_thread_id: None` and a default `run_config` on imported thread metadata. | R3, R17 | |
| [crates/agent_ui/src/thread_metadata_store.rs](../../crates/agent_ui/src/thread_metadata_store.rs) | Adds persisted `parent_thread_id` and `run_config` (migrations, queries, pending maps) and the stable `interacted_at` rule. | R3, R9, R17 | |
| [crates/agent_ui/src/threads_archive_view.rs](../../crates/agent_ui/src/threads_archive_view.rs) | Sets `parent_thread_id: None` and a default `run_config` on restored thread metadata. | R3, R17 | |
| [crates/orchestration/Cargo.toml](../../crates/orchestration/Cargo.toml) | New crate manifest with its dependencies (incl. `context_server`, `net`, `util`) and test dependencies. | R1 | |
| [crates/orchestration/LICENSE-GPL](../../crates/orchestration/LICENSE-GPL) | New license link for the crate, pointing to the repository `LICENSE-GPL`. | R1 | |
| [crates/orchestration/src/child_log.rs](../../crates/orchestration/src/child_log.rs) | New `ChildLog`: JSONL exchange log with `to_parent` and `human_intervention` entries, plus `log_dir` resolution. | R4, R15, R16 | |
| [crates/orchestration/src/orchestration.rs](../../crates/orchestration/src/orchestration.rs) | New shared types, the `OrchestrationHost` trait, child header, inbox, whoami, human-summary and profile types. | R1, R2, R3, R5, R14, R15, R16, R17 | |
| [crates/orchestration/src/registry.rs](../../crates/orchestration/src/registry.rs) | New global `OrchestrationRegistry`: records, tokens, status machine, `await_children`, inbox, urgent flush, human summaries. | R1, R3, R4, R5, R14, R15, R16 | |
| [crates/acp_thread/src/acp_thread.rs](../../crates/acp_thread/src/acp_thread.rs) | Adds `last_message_at`, replay tracking, `last_assistant_message` and `rename_subagent` to `AcpThread`, with a test. | R5, R9 | |
| [crates/agent/src/agent.rs](../../crates/agent/src/agent.rs) | Marks native history replay and restores `updated_at`; adds `available_profiles` and `selection_to_model_string`. | R9, R17 | |
| [crates/agent/src/thread.rs](../../crates/agent/src/thread.rs) | Adds `updated_at()`, registers and gates the orchestration tools for non-subagent threads, and `profiles` on the agent list. | R1, R9, R14, R15, R17 | |
| [crates/agent_servers/src/acp.rs](../../crates/agent_servers/src/acp.rs) | Marks session updates as replayed history and adds Zed's orchestration MCP server to the session's MCP servers. | R1, R9 | |
| [crates/agent_servers/src/acp/v1.rs](../../crates/agent_servers/src/acp/v1.rs) | Passes the replaying flag to the thread; adds the orchestration MCP entry to new, loaded and resumed sessions, with tests. | R1, R9 | |
| [docs-product/features/orchestrated-threads-how-it-works.md](orchestrated-threads-how-it-works.md) | New explainer of how an agent opens a child conversation; the token note now says it is not part of the conversation. | R1, R3 | |
| [docs-product/features/orchestrated-threads-r1-plan.md](orchestrated-threads-r1-plan.md) | New implementation plan for the R1 to R5 slice, with decisions, steps, risks and tests. | R1, R2, R3, R4, R5 | |
| [docs-product/features/orchestrated-threads.md](orchestrated-threads.md) | New feature spec with requirements R1 to R17, decisions, acceptance criteria and the upstream path; R11 revised. | R1 to R17 | |
| [crates/agent/Cargo.toml](../../crates/agent/Cargo.toml) | Adds the `orchestration` workspace dependency. | R1 | |
| [crates/agent/src/tools.rs](../../crates/agent/src/tools.rs) | Declares and registers the seven orchestration tools and gates them on `agent.orchestration.enabled`. | R1, R14, R15 | |
| [crates/agent/src/tools/await_children_tool.rs](../../crates/agent/src/tools/await_children_tool.rs) | New native tool that waits for children and returns final messages, pending messages and human summaries. | R1, R5, R15, R16 | |
| [crates/agent/src/tools/cancel_child_tool.rs](../../crates/agent/src/tools/cancel_child_tool.rs) | New native tool that cancels a child through the host. | R1 | |
| [crates/agent/src/tools/child_status_tool.rs](../../crates/agent/src/tools/child_status_tool.rs) | New native tool that reports child status, pending messages and human-intervention summaries. | R1, R5, R15, R16 | |
| [crates/agent/src/tools/list_agents_and_models_tool.rs](../../crates/agent/src/tools/list_agents_and_models_tool.rs) | Tool text now covers `start_child` and the agent profiles returned with their descriptions. | R2, R17 | |
| [crates/agent/src/tools/orchestration_common.rs](../../crates/agent/src/tools/orchestration_common.rs) | New shared plumbing of the tools: names, availability gating, host lookup and error output, with tests. | R1, R15 | |
| [crates/agent/src/tools/send_to_child_tool.rs](../../crates/agent/src/tools/send_to_child_tool.rs) | New native tool that sends a follow-up to a child; queued while the child is mid-turn. | R1, R3 | |
| [crates/agent/src/tools/send_to_parent_tool.rs](../../crates/agent/src/tools/send_to_parent_tool.rs) | New native tool for a child to message its orchestrator, with an `urgent` flag. | R15 | |
| [crates/agent/src/tools/start_child_tool.rs](../../crates/agent/src/tools/start_child_tool.rs) | New native tool that starts a child, with agent, `profile`, model and worktree options. | R1, R2, R17 | |
| [crates/agent/src/tools/whoami_tool.rs](../../crates/agent/src/tools/whoami_tool.rs) | New native tool that returns the thread's role, its parent and its children. | R14 | |
| [crates/agent_servers/Cargo.toml](../../crates/agent_servers/Cargo.toml) | Adds the `orchestration` workspace dependency. | R1 | |
| [crates/agent_servers/src/acp/v2.rs](../../crates/agent_servers/src/acp/v2.rs) | Adds the orchestration MCP entry to new and loaded ACP v2 sessions when the agent supports stdio MCP, with a test. | R1 | |
| [crates/agent_settings/src/agent_profile.rs](../../crates/agent_settings/src/agent_profile.rs) | Adds an optional `description` to agent profiles and to profile creation, with a test. | R17 | |
| [crates/agent_ui/src/agent_configuration/manage_profiles_modal.rs](../../crates/agent_ui/src/agent_configuration/manage_profiles_modal.rs) | Adds a description field to profile creation and shows the description in the profile list. | R17 | |
| [crates/agent_ui/src/agent_configuration/tool_picker.rs](../../crates/agent_ui/src/agent_configuration/tool_picker.rs) | Passes the profile's `description` when the tool picker rebuilds a profile. | R17 | |
| [crates/agent_ui/src/conversation_view/message_queue.rs](../../crates/agent_ui/src/conversation_view/message_queue.rs) | Adds `MessageOrigin` (human or orchestration) to queued messages. | R16 | |
| [crates/agent_ui/src/profile_selector.rs](../../crates/agent_ui/src/profile_selector.rs) | Sets `description: None` where a profile is built, after the new field was added. | R17 | |
| [crates/context_server/src/listener.rs](../../crates/context_server/src/listener.rs) | Adds an identify handshake and `McpCaller` so MCP tools know the caller's token; adds `McpServer::start`. | R1 | |
| [crates/orchestration/src/bridge.rs](../../crates/orchestration/src/bridge.rs) | New stdio-to-socket relay for the `zed --orchestration-bridge` mode; sends the session token first. | R1 | |
| [crates/orchestration/src/mcp.rs](../../crates/orchestration/src/mcp.rs) | New MCP server with the orchestration tools and per-session token entries, for external ACP agents, with tests. | R1, R2, R3, R4, R5, R14, R15, R16, R17 | |
| [crates/settings_ui/src/pages/tool_permissions_setup.rs](../../crates/settings_ui/src/pages/tool_permissions_setup.rs) | Adds the seven orchestration tool names to the test list of tools without permission settings. | R1 | |
| [crates/zed/Cargo.toml](../../crates/zed/Cargo.toml) | Adds the `orchestration` workspace dependency. | R1 | |
| [crates/zed/src/main.rs](../../crates/zed/src/main.rs) | Adds the hidden `--orchestration-bridge <socket>` argument that runs the bridge and exits. | R1 | |
| [docs-product/features/manual-review.md](manual-review.md) | This file: the review checklist of every changed file, with details by requirement. | R1 to R17 | |

## Details by requirement

Sections are grouped by the requirement they mainly serve. A file that serves several requirements appears once, in its main section, with a cross-reference line elsewhere.

### R9 and R10: subthreads in the sidebar

Commits `b7c55ae`, `5921469`, `0d896ef`, `c449f10` (plus the settings parts of `36ead50`). Subagent threads of a parent are shown nested under it in the sidebar, controlled by `agent.orchestration.*` settings.

#### [`crates/ui/src/components/ai/thread_item.rs`](../../crates/ui/src/components/ai/thread_item.rs)

- Scope: the shared UI component that draws a sidebar row; nesting and the collapse control live here.
- What changed: new fields `indented` and `disclosure` (private struct `ThreadItemDisclosure { is_open, on_toggle }`); new builders `indented(bool)` and `disclosure(is_open, on_toggle)`. The chevron is an `IconButton` at the right end (`ChevronUp` when open, `ChevronDown` when collapsed); the row gets `pl_5` when indented.
- Why: the toggle must not trigger the row's own click, so the button calls `cx.stop_propagation()` and its container swallows left mouse down. Added as optional builders so existing callers are unchanged.
- Review focus:
  - Click and mouse-down propagation: the chevron must not open the thread.
  - Default behavior of other `ThreadItem` users (defaults are `indented: false`, `disclosure: None`).

Changed again in `b1c2733`: new optional field `model` and builder `model(...)`. The label sits in the metadata line, followed by a dot separator when project, paths, worktree, diff stats or a timestamp follow; `has_metadata` is also true when only a model exists. Why: nested child rows show which model runs the child (R17 feeds the label from `10530cf`).
- Review focus:
  - The separator condition: no dangling dot when the model is the only item.
  - Rows without a model render as before.

#### [`crates/sidebar/src/subthreads.rs`](../../crates/sidebar/src/subthreads.rs)

- Scope: all the pure logic of the nested rows, kept out of `sidebar.rs`.
- What changed: new module with `live_subthreads_by_parent` (reads `ConversationView::subthreads` of every conversation in the open agent panels), `SubthreadEntry`, `VisibleSubthreads`, `select_visible_subthreads` (most recent `last_message_at` first, threads without a message last), `SubthreadLayout` and `insert_subthread_entries`.
- Why: a separate module keeps the edit in `sidebar.rs` small and rebaseable. Only live subagents of loaded conversations are listed (module doc), and only direct children.
- Review focus:
  - Only parents with an open workspace and no draft get children (`ThreadEntryWorkspace::Open`, `draft.is_none()`).
  - `Load more` is shown only when hidden rows remain; collapsed parents add no rows but still record their count.

Changed again in `b1c2733`: the module now also lists children that exist only in thread metadata (linked through `parent_thread_id`, not loaded as live subagents). `SubthreadLayout` gains `nest_children`; `child_info` turns a child `ThreadEntry` into a `SubthreadInfo` (title, status, `updated_at` as last message, provider icon, model, thread id); `insert_subthread_entries` splits the group into top-level entries and children, adds live infos only when the parent's workspace is open, and emits metadata children as `ListEntry::Thread` rows with `nested = true` (live ones stay `ListEntry::Subthread`). A thread is nested only when it has a session id and its parent is a non-draft root of the same group. Why: after a restart children are no longer live subagents but remain linked threads (R3, R9).
- Review focus:
  - Still one level only: a child that has a parent of its own is never a root.
  - A child whose parent is archived or not in the group stays top-level (`test_metadata_child_of_archived_parent_stays_top_level`).
  - Live and metadata children share one ordering and one visible limit.

#### [`crates/sidebar/src/sidebar.rs`](../../crates/sidebar/src/sidebar.rs)

- Scope: the sidebar list itself: new row kinds, state, rendering, input handling.
- What changed: `ListEntry::Subthread` and `ListEntry::LoadMoreSubthreads`; `Contents.subthread_counts`; `EntryShape` variants; sidebar fields `collapsed_subthread_groups` and `fully_loaded_subthread_groups`; `update_entries` reads `AgentSettings::orchestration` and calls `insert_subthread_entries` for both project-group loops; `viewed_subthread`, `toggle_subthreads`, `load_more_subthreads`, `open_subthread`, `render_subthread`, `render_load_more_subthreads`; `show_root_thread` makes opening a thread show the root, not the last viewed subagent; the parent row gets the `disclosure` chevron.
- Why: groups are expanded by default (`c449f10`), so state tracks what the user collapsed. While a subagent is shown, its row is active and the parent's is not. Many existing `match` arms only gained the two new variants.
- Review focus:
  - `show_root_thread` calls `navigate_to_subthread` with the root session; confirm it does not disturb thread activation or focus.
  - `visible: orchestration.subthreads_visible.max(1)` (a value of 0 never hides everything).
  - Active-row logic in the render path, and the new arms in keyboard navigation and project-header lookup.

Changed again in `b1c2733`: `RenameTarget::Subthread(ThreadId, SessionId)` (so `RenameTarget` is no longer `Copy`), `apply_subthread_rename`, which finds the subagent in the parent's open conversation and calls `AcpThread::rename_subagent`; subthread rows get a hover pencil button and a right-click "Rename Title" menu; `ThreadEntry` gains `model: Option<ThreadModel>` and `nested`; nested thread rows are indented and show the model; `Contents` carries `nest_children` from `show_as_subthreads`. Why: metadata children appear as nested rows, and the user can rename a child and see its model and provider icon (R9).
- Review focus:
  - The rename path of a live subagent: it only works while the parent conversation is loaded; metadata children use the thread rename (`test_rename_metadata_child_from_sidebar`).
  - `RenameTarget` losing `Copy`: every use site now clones or borrows.

Changed again in `10530cf` (R17): `persisted_thread_model` rebuilds the model name and provider icon from `ThreadMetadata.run_config` through the language model registry when the thread is not loaded; nested rows label `profile · model` (`child_model_label`, `profile_display_name`); `info.model` no longer erases an already known model (`or_else`). Why: a child's model and provider stay visible after a restart.
- Review focus:
  - A provider missing from the registry falls back to the model id and no icon.
  - The profile name falls back to the profile id when the profile no longer exists.

#### [`crates/sidebar/src/sidebar_tests.rs`](../../crates/sidebar/src/sidebar_tests.rs)

- Scope: test coverage for R9 behavior and the stable-order rule.
- What changed: helpers (`set_orchestration_settings`, `add_subagent`, `touch_subagent`, `setup_parent_with_subagents`, `thread_order`, `setup_two_threads_older_and_newer`, `find_subthread_entry`) and tests for: setting off, nested and expanded by default, ordering by last message, visible limit and load more, one level only, row status, opening a row, parent click not toggling, rapid switching, active row, live added/updated subagents, ACP-announced subagents, collapse persistence, opening an older thread not moving it, sending moving it to the top, replaying a subagent history not reordering siblings.
- Why: covers the sidebar behavior of R9 and the sort-order rule of `c449f10`.
- Review focus:
  - That the tests assert behavior, not implementation details of the entry list.
  - The large size of the diff is almost entirely additive.

Changed again in `b1c2733` and `10530cf`: new helpers `save_orchestration_thread_metadata`, `minutes`, `save_parent_with_metadata_children`, and tests for metadata children nesting and leaving the top level, limit, load more and message order, setting off, archived parent, renaming a metadata child and a live subagent, the model on a live subagent row, the persisted model and icon of an unloaded child, `test_child_model_label_puts_the_profile_before_the_model` and the child's own provider icon and model.
- Review focus:
  - The assertions describe visible behavior (rows, labels, order), not entry-list internals.

#### [`crates/sidebar/Cargo.toml`](../../crates/sidebar/Cargo.toml)

- Scope: test dependency.
- What changed: adds `watch.workspace = true` after the dev-dependencies block.
- Why: `sidebar_tests.rs` uses `watch::Receiver::constant(...)` for prompt capabilities.
- Review focus:
  - The line sits after the `[dev-dependencies]` entries; confirm it is meant to be test-only and in the right table.

#### [`crates/agent_ui/src/conversation_view.rs`](../../crates/agent_ui/src/conversation_view.rs)

- Scope: exposes the data the sidebar needs, and the entry points the orchestration host needs (see also R1 to R5).
- What changed: `SubthreadInfo` (session id, title, status, `last_message_at`, icon) and `ConversationView::subthreads` (direct children only; status derives from pending tool call, error, then thread status). From `36ead50`: `load_error_message`, `send_follow_up` (delegates to the root thread view), and `parent_thread_id: None` added to two test metadata literals.
- Why: the sidebar cannot reach into conversation internals, so a small read-only API is added.
- Review focus:
  - Status mapping order (waiting for confirmation, then error, then running or completed).
  - Title fallback chain: association title, thread title, `DEFAULT_THREAD_TITLE`.

Changed again in `b1c2733`: `SubthreadInfo` gains `model` and `thread_id` (`None` for a live subagent); a subagent's icon now comes from its own view, and for the native agent from the provider of its model (`model_icon_parts`, also exported). In `10530cf`: `run_config: Default::default()` in two test metadata literals. Why: rows show the provider icon and model of each child (R9, R17).
- Review focus:
  - The provider icon applies only when the thread is a native one; external agents keep the agent icon.

#### [`crates/agent_ui/src/agent_ui.rs`](../../crates/agent_ui/src/agent_ui.rs)

- Scope: crate wiring.
- What changed: `pub use ... SubthreadInfo`; `mod orchestration_host;`; `Agent::Stub` now builds its server through `test_support::stub_agent_server()`; test settings get an `orchestration: OrchestrationSettings` value.
- Why: sidebar needs the type exported; the stub hook serves the orchestration tests (R1).
- Review focus:
  - The `Stub` server path is test-only code; confirm it is behind the same cfg as before.

Changed again in `b1c2733` and `d303f89`: `model_icon_parts` is re-exported; the test settings literal gains `human_summary_max_chars: 600`.

#### [`crates/agent_settings/src/agent_settings.rs`](../../crates/agent_settings/src/agent_settings.rs)

- Scope: the resolved (runtime) settings type.
- What changed: `pub orchestration: OrchestrationSettings` on `AgentSettings`; new `OrchestrationSettings { show_as_subthreads: bool, subthreads_visible: usize, enabled: bool }`; `from_settings` reads all three with `unwrap()`.
- Why: defaults come from `default.json`, so every field is present after merge.
- Review focus:
  - `unwrap()` relies on `default.json` defining every key; check it does.
  - `log_dir` is intentionally not in this struct (a path is not `Copy`); `orchestration_log_dir` reads it from the merged settings.

Changed again in `d303f89` (R16): `OrchestrationSettings.human_summary_max_chars` (usize), read with `unwrap()` like its siblings.
- Review focus:
  - `default.json` defines `human_summary_max_chars`, so the `unwrap()` holds.

#### [`crates/settings_content/src/agent.rs`](../../crates/settings_content/src/agent.rs)

- Scope: the user-facing settings schema.
- What changed: `OrchestrationSettingsContent { show_as_subthreads, subthreads_visible, enabled, log_dir }` (all `Option`) and `AgentSettingsContent.orchestration`.
- Why: follows the existing pattern of optional content structs with documented defaults; derives `JsonSchema` so the schema and settings file completions update.
- Review focus:
  - Doc comments match the defaults in `default.json`.
  - Naming of the group (`orchestration`) is a proposal that upstream may want to rename.

Changed again in `d303f89` (R16): `OrchestrationSettingsContent.human_summary_max_chars: Option<usize>`, and the seven orchestration tool names in the profile tools schema list. In `10530cf` (R17): `AgentProfileSettingsContent.description: Option<String>`.
- Review focus:
  - Doc comments match the default (600) and the profile description semantics.

#### [`assets/settings/default.json`](../../assets/settings/default.json)

- Scope: shipped defaults.
- What changed: a new `"orchestration"` object with `show_as_subthreads: true`, `subthreads_visible: 5`, `enabled: true`, `log_dir: null`.
- Why: required by the `unwrap()` reads above; behavior is on by default per R10.
- Review focus:
  - Default `true` for the sidebar nesting and the orchestration tools changes what every user sees.
  - See also R1 to R5 (`enabled`, `log_dir`).

Changed again in `b1c2733`: the five tools of `b1c2733` are enabled in the `write` profile. `d303f89` (R16): `human_summary_max_chars: 600`, the comment lists `send_to_parent` and `whoami`, and both are enabled in `write`. `10530cf` (R17): a `description` on the `write`, `ask` and `minimal` profiles.
- Review focus:
  - The built-in descriptions are the text an orchestrator reads when picking a profile.
  - `ask` and `minimal` get no orchestration tools by default.

#### [`crates/settings_ui/src/page_data.rs`](../../crates/settings_ui/src/page_data.rs)

- Scope: Settings Editor entries for the new settings.
- What changed: four `SettingItem`s ("Show Subagent Threads In Sidebar", "Visible Subagent Threads", "Orchestration Tools", "Orchestration Log Directory"); array size in `general_section` changed from 8 to 12. The log directory item has text-field metadata (placeholder, confirm and clear buttons).
- Why: Zed keeps the Settings Editor in sync with `default.json`.
- Review focus:
  - Array length matches the item count.
  - `json_path` strings match the settings keys exactly.

Changed again in `d303f89` (R16): a fifth item, "Human Intervention Summary Length" (`agent.orchestration.human_summary_max_chars`); the array in `general_section` goes from 12 to 13 items.
- Review focus:
  - Array length matches the item count; `json_path` matches the key.

#### [`crates/agent/src/tool_permissions.rs`](../../crates/agent/src/tool_permissions.rs)

- Scope: test fixture only.
- What changed: adds `orchestration: OrchestrationSettings { true, 5, true }` to a test `AgentSettings` literal.
- Why: the struct gained a field; no production code changed.
- Review focus:
  - Test-only; confirm nothing outside the test module changed.

Changed again in `d303f89`: the test literal also sets `human_summary_max_chars: 600`.

#### [`docs/src/reference/all-settings.md`](../../docs/src/reference/all-settings.md)

- Scope: user documentation for the settings.
- What changed: four new sections with anchors `agent-orchestration-show-as-subthreads`, `-subthreads-visible`, `-enabled`, `-log-dir`, each with description, setting key, default and a `settings.json` example.
- Why: Zed documents every setting in the reference.
- Review focus:
  - Wording matches the Settings UI descriptions and the defaults.
  - Links to the Threads Sidebar docs page resolve.

Changed again in `d303f89` (R16): section "Human Intervention Summary Length" (`agent.orchestration.human_summary_max_chars`, default `600`, behavior and example). In `10530cf` (R17): section "Profile Description" (`agent.profiles.<id>.description`), how the child's model is resolved, and a `settings.json` example.
- Review focus:
  - The model resolution order text matches `resolve_child_model`.

### R1–R5: orchestration core (groundwork)

Commit `36ead50`. The new `orchestration` crate holds the UI-independent parts (types, registry, log, host trait); the agent panel implements the host. No tool is exposed to agents yet in this commit; this is groundwork for R1 to R5. The tools arrive in `b1c2733`, described in the next section.

#### [`Cargo.toml`](../../Cargo.toml) and [`Cargo.lock`](../../Cargo.lock)

- Scope: workspace registration of the new crate.
- What changed: `crates/orchestration` added to `members` and `orchestration = { path = "crates/orchestration" }` to workspace dependencies. `Cargo.lock` gains the `orchestration` package (anyhow, chrono, collections, futures, gpui, log, schemars, serde, serde_json, settings, sqlez, tempfile, uuid, watch) and `watch` for the sidebar crate (from `0d896ef`).
- Why: mechanical consequence of the new crate and dependencies.
- Review focus:
  - `Cargo.lock` was edited with the manifest changes; regenerate to confirm it is consistent.

Changed again in `b1c2733`: `orchestration` added to the dependencies of `agent`, `agent_servers` and `zed`, and `context_server`, `net` and `util` to the `orchestration` package.

#### [`README.md`](../../README.md)

- Scope: AGENTS.md hard rule for any change to source files.
- What changed: two lines added at the top (`> [!IMPORTANT]` and `> Remove this line to confirm you've reviewed this PR before submitting.`).
- Why: AGENTS.md requires these lines whenever source files are modified, and forbids tooling from removing them.
- Review focus:
  - Remove them manually only after reviewing every file in this table; never by script or agent.

#### [`crates/orchestration/Cargo.toml`](../../crates/orchestration/Cargo.toml) and [`crates/orchestration/LICENSE-GPL`](../../crates/orchestration/LICENSE-GPL)

- Scope: the new crate's packaging.
- What changed: manifest with workspace dependencies (`anyhow`, `chrono`, `collections`, `futures`, `gpui`, `log`, `schemars`, `serde`, `serde_json`, `settings`, `sqlez`, `uuid` with `v4`, `watch`) and dev-dependencies (`gpui` test-support, `tempfile`); `LICENSE-GPL` is a link to `../../LICENSE-GPL`.
- Why: new crates need a manifest and a license file.
- Review focus:
  - Confirm the license link and lint settings match sibling crates.
  - `settings` is used by `orchestration_log_dir` in `registry.rs` to read `log_dir`.

Changed again in `b1c2733`: `Cargo.toml` adds `context_server`, `net` and `util` (for the MCP server and the bridge) and the `settings` test-support feature in dev-dependencies.
- Review focus:
  - The crate now depends on `context_server`; it still must not depend on `agent_ui`.

#### [`crates/orchestration/src/orchestration.rs`](../../crates/orchestration/src/orchestration.rs)

- Scope: shared vocabulary of the feature (R1 tools, R2 child start options, R3 status, R5 final-message-only results).
- What changed: `ThreadId` (moved here from `thread_metadata_store`, with `sqlez` `Bind` and `Column`, plus `from_key_string`, `from_uuid`, `as_uuid`); `ChildStatus` and `WorktreeMode`; input and output types for `start_child`, `await_children`, `child_status`, `send_to_child`, `cancel_child`; `AvailableAgents`; `DEFAULT_AWAIT_TIMEOUT_SECS` (120) and `MAX_AWAIT_TIMEOUT_SECS` (3600); the `OrchestrationHost` trait (`child_status` and `await_children` have default implementations over the registry).
- Why: no UI dependency, so tool adapters (native tools, an MCP server) can be thin layers over one trait.
- Review focus:
  - Moving `ThreadId` changes its home crate; `thread_metadata_store` re-exports it (`pub use`).
  - The await timeout default sits below common MCP client tool timeouts (per the code comment).

Changed again in `b1c2733`: `mod bridge;`, `mod mcp;` and their `pub use`. In `d303f89` (R14 to R16): `PendingMessage` and `HumanIntervention` on the await and status outputs, `SendToParentInput` and `SendToParentOutput`, `WhoAmIInput` and `WhoAmIOutput` with `ThreadRole`, `ThreadReference` and `ChildReference`, `HumanSummaryRequest`, and the text helpers `child_header`, `urgent_message_prefix`, `human_summary_prompt`, `truncate_summary` and `human_summary_fallback`. The `OrchestrationHost` trait gains `thread_title`, `is_thread_idle`, `deliver_message`, `summarize_human_interventions` (default implementations), `send_to_parent` and `whoami`, and `child_status` now returns a `Task`. In `10530cf` (R17): `StartChildInput.profile`, `AvailableProfile`, `AvailableAgents.profiles`, `ChildModelSource`, `resolve_child_model` and `unknown_profile_error`, with tests.
- Review focus:
  - Changing `child_status` to a `Task` is needed because summaries are generated on demand.
  - The child header text is what a child model reads first; check the wording.
  - The model order: explicit, profile default, `agent.subagent_model`, user default.

#### [`crates/orchestration/src/registry.rs`](../../crates/orchestration/src/registry.rs)

- Scope: the live record of every child of every orchestrator (R3), including `await_children` and `child_status` (R1, R5) and the logging calls (R4).
- What changed: global `OrchestrationRegistry` (`global_or_init`, `try_global`), `ChildRecord`, `NewChild`, tokens (`mint_token`, `resolve_token`, `revoke_tokens`), host registration (`register_host`, `host_for_session`, `host_for_thread`), `register_child`, `remove_orchestrator`, status transitions (`set_status`, `record_message_to_child`, `record_turn_finished`), `child_status`, `await_children`, event type `OrchestrationRegistryEvent`, and tests (status state machine, scoping per orchestrator, tokens, parallel await, timeout, unknown ids, metadata only, log contents, cleanup).
- Why: live-only state (not persisted) keeps the first slice small; a record holds only the last message of the latest turn, never a transcript (R5).
- Review focus:
  - Scoping: a child must be reachable only by the orchestrator that started it (`resolve_child_ids`).
  - `await_children` race handling and timeout behavior, and that results contain no child content other than the final message.
  - Cleanup in `remove_orchestrator` (children and tokens).

Changed again in `b1c2733`: session tokens (`mint_pending_session_token`, `bind_session_token`, `token_for_session`) and `resolve_caller`, which maps an MCP token to the orchestrator thread and its host. Why: the code that creates ACP sessions knows the session id, not the Zed thread. In `d303f89` (R14 to R16): the inbox (`InboxMessage`, `record_message_from_child`, `send_to_parent`, `take_pending_messages`), `flush_urgent` and `wait_for_urgent` (an urgent message wakes `await_children`), human-turn tracking (`note_human_turn`, `note_orchestrator_turn`, `pending_human_turns`), `collect_human_interventions`, `child_status_with_summaries`, `whoami`, `human_summary_max_chars` and a large test set.
- Review focus:
  - Tokens: an unknown token resolves to nothing; the same session always gets the same token.
  - Urgent delivery only when the orchestrator is idle, and never delivered twice.
  - Human messages never reach the log or the orchestrator, only the summary (`direction: human_intervention`).

#### [`crates/orchestration/src/child_log.rs`](../../crates/orchestration/src/child_log.rs)

- Scope: R4, the per-child exchange log.
- What changed: `LogDirection`, `ChildLogEntry { ts, direction, text, status }`, `default_log_dir`, `log_dir_from_setting` (empty counts as unset), `ChildLog::{new, path, append, read}`, with tests.
- Why: JSONL at `<log_dir>/<orchestrator-id>/<child-id>.jsonl`; only messages crossing between the two are written, never tool calls or intermediate output; writes are synchronous because they are tiny and at turn boundaries (module doc).
- Review focus:
  - Synchronous file writes on the main thread (acceptable per the comment; confirm).
  - Directory creation and error handling in `append`.

Changed again in `d303f89`: `LogDirection` gains `ToParent` (from `send_to_parent`) and `HumanIntervention` (the summary, never the human's messages).

#### [`crates/agent_ui/Cargo.toml`](../../crates/agent_ui/Cargo.toml)

- Scope: dependency for the panel host.
- What changed: adds `orchestration.workspace = true`.
- Why: `agent_ui` implements `OrchestrationHost` and uses the registry and `ThreadId`.
- Review focus:
  - Dependency direction: `orchestration` must not depend on `agent_ui`.

#### [`crates/agent_ui/src/orchestration_host.rs`](../../crates/agent_ui/src/orchestration_host.rs)

- Scope: the UI side of R1 to R5: creating children, messaging them, and tracking their turns.
- What changed: new `AgentPanelOrchestrationHost` implementing `OrchestrationHost` (`start_child`, `send_to_child`, `cancel_child`, `list_available_agents`, `resolve_orchestrator`, `owns_thread`); `ensure_enabled` checks `agent.orchestration.enabled`; helpers `observe_child`, `attach_thread_observer`, `on_thread_event`, `failure_message`, `sync_live_status`, `finish_turn`.
- Why: children may live in another panel (new worktree), so the host keeps its own weak handle to each child's `ConversationView`. `model` is ignored with a warning for external agents (only the native agent supports it). A turn end is recorded once even if several events report it.
- Review focus:
  - `start_child` error paths: no child record or thread is left behind on failure (for example a new worktree without git).
  - `finish_turn` de-duplication and the `Running` to `WaitingForConfirmation` sync.
  - That `enabled = false` really refuses every entry point.

Changed again in `d303f89` (R14 to R16): the child's first prompt is prefixed with `child_header(<orchestrator title>)`; new trait methods `thread_title`, `is_thread_idle`, `deliver_message` and `summarize_human_interventions` (uses the `thread_summary_model`, with a bounded transcript from `human_turn_transcript`); `watch_orchestrator` flushes urgent messages when the orchestrator stops or becomes idle; `note_turn` lets the thread view report who wrote each turn; `send_text` is shared by `send_to_child` and `deliver_message`. In `10530cf` (R17): `start_child` validates `profile` against `agent::available_profiles` (unknown id is an error listing the valid ones), ignores it with a warning for external agents, and resolves the native model with `resolve_child_model`; `list_available_agents` returns the profiles.
- Review focus:
  - Summaries cap at `human_summary_max_chars`; with no summary model the output is a minimal note.
  - `watch_orchestrator` subscription lifetime (one per orchestrator).
  - The "profile ignored for external agents" warning path.

#### [`crates/agent_ui/src/agent_panel.rs`](../../crates/agent_ui/src/agent_panel.rs)

- Scope: the panel is the host registered with the registry (R1), starts children (R2) and links them to the parent (R3). Also adds `navigate_to_subthread` for R9 (`b7c55ae`).
- What changed: `AgentPanel::new` now takes `window` and registers a `AgentPanelOrchestrationHost`, unregistering on release; `CreateThreadOptions.parent_thread_id`, applied through `ThreadMetadataStore::set_parent_thread_id`; deleting a thread (`remove_thread`) calls `remove_orchestrator`; new `selected_agent_is_native`, `available_agents` and `navigate_to_subthread`; the body of the existing sibling-thread creation was moved into `create_child_thread` (with `ChildThreadRequest` and `ChildThread`), which the old `create_thread` tool path now also calls with `parent_thread_id: None`; tests `test_orchestration_*`.
- Why: extracting instead of duplicating keeps one code path for agent-started threads and lets the host reuse the worktree flow.
- Review focus:
  - The extraction diff is large but should be behavior-preserving for the existing `create_thread` tool; compare the moved block line by line.
  - Lifetime of the host registration (`on_release`) and `remove_orchestrator` on thread removal.
  - `AgentPanel::new` now uses its `window` argument (previously `_window`); the signature types are unchanged.

Changed again in `d303f89`: only tests were added (`OrchestrationFixture` helpers and tests for the child header, `whoami` and `send_to_parent`, urgent delivery, human messages neither logged nor forwarded, on-demand summary, the character cap and the no-model note). In `10530cf` (R17): `CreateThreadOptions.profile` and `ChildThreadRequest.profile`; `apply_native_run_config` applies the profile first and then the explicit model once the native thread exists, and `persist_native_run_config` stores model and profile in the thread metadata (and again when the model changes); `available_agents` returns `profiles`. A profile that `profile_was_downgraded` (untrusted workspace) is not applied. Tests cover the resolution order, the profile's tool set, an unknown profile, the profile list and an external agent.
- Review focus:
  - Ordering: the profile swaps the thread to its own model, so an explicit model must be applied after it.
  - The restricted-workspace case: a model-requested profile must not lift the `minimal` downgrade.

#### [`crates/agent_ui/src/conversation_view/thread_view.rs`](../../crates/agent_ui/src/conversation_view/thread_view.rs)

- Scope: R3 follow-ups from the orchestrator to a child.
- What changed: `send_follow_up(content, window, cx) -> bool`: queues the content when the thread is generating, otherwise resumes the message queue and sends it; returns whether it was queued.
- Why: matches the `send_to_child` contract (queued when mid-turn, delivered when the turn ends).
- Review focus:
  - The `message_queue.resume()` call before sending an idle thread's message.
  - Queued messages being delivered as a new turn (covered by the panel test).

Changed again in `b1c2733`: `active_model(cx)` returns the model selector's active model with its provider icon (used for row labels). In `d303f89` (R16): messages carry a `MessageOrigin`; `send_content_with_origin` and `add_to_queue_with_origin` keep it, and every send except the thread's first calls `orchestration_host::note_turn` with the origin and the first entry index; `send_follow_up` marks messages as `Orchestration`.
- Review focus:
  - The first message of a thread is never counted as human (for a child it is the orchestrator's prompt).
  - A queued message keeps its origin when it is sent later.

#### [`crates/agent_ui/src/thread_metadata_store.rs`](../../crates/agent_ui/src/thread_metadata_store.rs)

- Scope: the persisted parent link (R3), so children survive restarts as linked threads. Also holds the sort-order rule for `interacted_at` (R9, see "Sort-order fix").
- What changed: `ThreadId` now re-exported from `orchestration`; `ThreadMetadata.parent_thread_id: Option<ThreadId>`; new migration `ALTER TABLE sidebar_threads ADD COLUMN parent_thread_id BLOB`; queries read and upsert it; fallback `LIST_QUERY_WITHOUT_PARENT_THREAD_ID` for databases of other release channels; `set_parent_thread_id` with `pending_parent_links` for threads without metadata yet; `child_threads`; tests for round trip, pending links, migration and old-database reads.
- Why: the link is persisted (the registry itself is live-only). A pending map covers the time between creating a thread and its first metadata save.
- Review focus:
  - The migration is append-only and the fallback chain for thread import (three queries) is correct.
  - `parent_thread_id` is preserved on every re-save built from the conversation.
  - `interacted_at` change (see "Sort-order fix").

Changed again in `10530cf` (R17): `ThreadMetadata.run_config: ThreadRunConfig { model_provider_id, model_id, profile_id }` with `model()`; migration adding three nullable columns (`model_provider_id`, `model_id`, `profile_id`); `update_run_config` with a `pending_run_configs` map for threads without metadata yet; `LIST_QUERY_WITHOUT_RUN_CONFIG` as a fallback for databases of other release channels; empty strings read back as `None`; tests for the round trip, the migration and reading an older database. Why: the sidebar must show a child's model and provider icon even when the thread is not loaded.
- Review focus:
  - The migration is append-only and the three-step fallback chain of `list` is correct.
  - The run config is preserved on every re-save (existing value first, then the pending one).

#### [`crates/agent_ui/src/thread_import.rs`](../../crates/agent_ui/src/thread_import.rs) and [`crates/agent_ui/src/threads_archive_view.rs`](../../crates/agent_ui/src/threads_archive_view.rs)

- Scope: construction sites of `ThreadMetadata`.
- What changed: one line each: `parent_thread_id: None`.
- Why: the struct gained a field; imported and restored threads have no parent.
- Review focus:
  - Confirm no other construction site needs a real parent value.

Changed again in `10530cf`: `run_config: Default::default()` as well.

#### [`crates/agent_ui/src/test_support.rs`](../../crates/agent_ui/src/test_support.rs)

- Scope: test infrastructure for the panel-level orchestration tests.
- What changed: thread-local `STUB_AGENT_SERVER_CONNECTION`, `use_connection_for_stub_agent` (returns a guard that clears it on drop), `stub_agent_server()`.
- Why: when the panel starts the thread itself, a test cannot pass a server in, so it overrides what `Agent::Stub` creates.
- Review focus:
  - Test-support module (`pub mod test_support`); confirm how it is gated for release builds.
  - The guard clears the override on drop so tests do not leak state.

### R1–R5: native tools and MCP server

Commit `b1c2733` (extended by `d303f89` and `10530cf`). The orchestration tools are exposed to agents in two ways: as native tools of the Zed agent, and as an MCP server for external ACP agents, reached through a hidden stdio bridge mode of the `zed` binary. Seven tools exist: `start_child`, `await_children`, `child_status`, `send_to_child`, `cancel_child`, plus `send_to_parent` and `whoami` (`d303f89`). Entries that changed again (`registry.rs`, `orchestration.rs`, `thread.rs`, `acp.rs`, `v1.rs`, `Cargo.lock`, `orchestration/Cargo.toml`, `default.json`) have a "Changed again" paragraph in their original section above.

#### [`crates/agent/Cargo.toml`](../../crates/agent/Cargo.toml), [`crates/agent_servers/Cargo.toml`](../../crates/agent_servers/Cargo.toml) and [`crates/zed/Cargo.toml`](../../crates/zed/Cargo.toml)

- Scope: dependency wiring.
- What changed: each adds `orchestration.workspace = true`.
- Why: the native tools, the MCP session entries and the bridge mode use the `orchestration` crate.
- Review focus:
  - `Cargo.lock` matches (see its entry above).

#### [`crates/agent/src/tools.rs`](../../crates/agent/src/tools.rs)

- Scope: the tool registry of the native agent.
- What changed: modules and re-exports for the new tool files; the tools are added to the `tools!` list (`AwaitChildrenTool`, `CancelChildTool`, `ChildStatusTool`, `SendToChildTool`, `SendToParentTool`, `StartChildTool`, `WhoAmITool`); a new arm in the feature-flag check: orchestration tools follow `agent.orchestration.enabled`.
- Why: tools must be registered to be offered to the model, and the setting must switch them off (R10).
- Review focus:
  - The new arm uses the merged `AgentSettings`, so the setting takes effect without a restart.

#### [`crates/agent/src/tools/orchestration_common.rs`](../../crates/agent/src/tools/orchestration_common.rs)

- Scope: plumbing shared by all native orchestration tools.
- What changed: `ORCHESTRATION_TOOL_NAMES` (seven names) and `is_orchestration_tool`; `OrchestrationToolOutput` (success or an error the model can read); `resolve_orchestrator`, `orchestration_available`, `orchestration_tool_available` (`send_to_parent` also needs an orchestrator) and `run_orchestration_tool`; tests with a fake host (each tool calls the host for its orchestrator, host errors reach the model, no registry gives a clear error, names match the list).
- Why: each tool is a thin adapter, so the lookup of host and orchestrator lives once.
- Review focus:
  - Error messages are written for the model ("Orchestration is not available for this thread...").
  - `d303f89` added the two child-side tools and the stricter availability check.

#### [`crates/agent/src/tools/start_child_tool.rs`](../../crates/agent/src/tools/start_child_tool.rs), [`await_children_tool.rs`](../../crates/agent/src/tools/await_children_tool.rs), [`child_status_tool.rs`](../../crates/agent/src/tools/child_status_tool.rs), [`send_to_child_tool.rs`](../../crates/agent/src/tools/send_to_child_tool.rs) and [`cancel_child_tool.rs`](../../crates/agent/src/tools/cancel_child_tool.rs)

- Scope: the orchestrator-side tools (R1 to R5).
- What changed: five `AgentTool` implementations, each holding the session id of its thread and calling the host through `run_orchestration_tool`. The descriptions tell the model how to use them: results come back only as final messages, parallel children should not overlap, prefer `await_children` over polling. In `d303f89` the `await_children` and `child_status` descriptions also explain `pending_messages` and `human_interventions`, and `child_status` now returns a `Task`. In `10530cf` the `start_child` description explains `profile` and the model order, and the tool passes `profile` through.
- Why: native agents get the same operations as external ones through MCP (R1).
- Review focus:
  - Description text is behavior: check it against R5 (no transcripts) and R15, R16.
  - No tool returns child content other than the final message.

#### [`crates/agent/src/tools/list_agents_and_models_tool.rs`](../../crates/agent/src/tools/list_agents_and_models_tool.rs)

- Scope: the existing tool that lists agents and models.
- What changed: the description mentions `start_child` and, from `10530cf`, the `profiles` list and how to choose a profile (prefer `profile` over `model` and say which profile was used).
- Why: lets the orchestrator pick a profile by matching task to descriptions (R17).
- Review focus:
  - Only the doc comment changed; confirm no behavior change.

#### [`crates/agent_servers/src/acp/v2.rs`](../../crates/agent_servers/src/acp/v2.rs)

- Scope: ACP v2 sessions of external agents.
- What changed: new and loaded sessions pass `mcp_entry_for_new_session` or `mcp_entry_for_session` to `mcp_configurations_for_project`; new sessions bind the token to the session id; test `v2_session_adds_the_orchestration_server_when_stdio_is_supported` (without stdio capability the entry is dropped).
- Why: same as `v1.rs`, for the v2 protocol.
- Review focus:
  - The server is added only if the agent supports stdio MCP.

#### [`crates/context_server/src/listener.rs`](../../crates/context_server/src/listener.rs)

- Scope: the existing in-app MCP server that listens on a Unix socket.
- What changed: `IDENTIFY_METHOD` (`zed/identify`) and `McpCaller { token }`; the first `zed/identify` notification of a connection sets the token and is not a request; `McpServerTool::run` and the tool handlers receive `&McpCaller`; the server startup is split into `bind`, `serve`, `accept_loop` and a new `McpServer::start`; a tool called without arguments tries `{}` before `null`.
- Why: orchestration tools must know which orchestrator is calling without trusting model input.
- Review focus:
  - The token is taken only once per connection (`caller.token.is_none()`).
  - The refactor of the startup code is behavior-preserving for the existing users of `McpServer`.

#### [`crates/orchestration/src/bridge.rs`](../../crates/orchestration/src/bridge.rs)

- Scope: the process the external agent starts for the MCP server.
- What changed: `BRIDGE_TOKEN_ENV` (`ZED_ORCH_TOKEN`), `identify_line(token)` and `run_bridge(socket)`, which relays stdin and stdout to the socket and sends the identify line first. Unix only.
- Why: ACP agents can reach MCP only over stdio or HTTP; the bridge keeps the token in the process environment, not in a prompt or tool argument.
- Review focus:
  - Errors go to stderr; stdout belongs to the MCP protocol.
  - The relay copies bytes and never parses them.

#### [`crates/orchestration/src/mcp.rs`](../../crates/orchestration/src/mcp.rs)

- Scope: the orchestration tools as an MCP server, one per app.
- What changed: `OrchestrationMcpEntry` (command, args `--orchestration-bridge <socket>`, env with the token), `mcp_entry_for_new_session` and `mcp_entry_for_session` (none unless local project, Unix and `agent.orchestration.enabled`), `ensure_mcp_server` (socket in a 0700 tempdir, no network listener), `register_orchestration_tools` and one `McpServerTool` per tool (`start_child`, `await_children`, `child_status`, `send_to_child`, `cancel_child`, plus `send_to_parent`, `whoami` and `list_agents_and_models`). Each call resolves the token with `resolve_caller`; unknown or missing tokens are rejected. Tests cover the tool list and schemas, token scoping, unknown token, a connection that never identified, stable tokens per session and entry gating. In `d303f89` the two child-side tools and summaries were added, and in `10530cf` the tests check `profiles` in the list output.
- Why: gives external agents the same tools as native ones (R1), scoped to their own orchestrator (R3).
- Review focus:
  - Token handling and rejection paths.
  - The tool annotations (read-only or destructive) are correct.
  - The Unix-only code paths are `cfg`-gated.

#### [`crates/zed/src/main.rs`](../../crates/zed/src/main.rs)

- Scope: process entry point.
- What changed: hidden `--orchestration-bridge <SOCKET>` argument; when present, `orchestration::run_bridge` runs and the process exits (code 1 on error) before the app starts.
- Why: the agent starts `zed --orchestration-bridge` as its MCP stdio server.
- Review focus:
  - The branch runs before any app initialization.
  - The flag is hidden from `--help`.

#### [`crates/settings_ui/src/pages/tool_permissions_setup.rs`](../../crates/settings_ui/src/pages/tool_permissions_setup.rs)

- Scope: a test list only.
- What changed: the seven orchestration tool names are added to the list of tools that need no permission settings, with a comment that permission checks happen inside the child threads.
- Why: a test checks that every tool is either listed in the page or explicitly exempt.
- Review focus:
  - Confirm that exempting these tools matches R13 (children's permission requests surface in the orchestrator).

### R9: metadata children, rename and provider icons

Commit `b1c2733`, with the persisted model from `10530cf`. After a restart, children are no longer live subagents, so they are read from thread metadata and shown nested under their parent. Rows show the child's own provider icon and model, and a child can be renamed from the sidebar. The code changes are in the "Changed again" paragraphs of the entries for `subthreads.rs`, `sidebar.rs`, `sidebar_tests.rs`, `thread_item.rs`, `conversation_view.rs` and `agent_ui.rs`; the only new code outside them is below.

#### [`crates/acp_thread/src/acp_thread.rs`](../../crates/acp_thread/src/acp_thread.rs) and [`crates/agent_ui/src/conversation_view/thread_view.rs`](../../crates/agent_ui/src/conversation_view/thread_view.rs)

- Scope: model and rename support for subagent rows.
- What changed: `AcpThread::rename_subagent(title)` replaces the parent's label for the subagent, emits `SubagentAssociationUpdated` and sets the thread's own title; `ThreadView::active_model` returns the active model with its icon.
- Why: the sidebar needs to rename a live subagent and to show which model runs a thread.
- Review focus:
  - An agent-owned subagent has no persistence of its own, so a rename lasts for the session.
  - Existing entries for both files carry the full "Changed again" detail.

### R14–R16: child awareness, inbox and human summary

Commit `d303f89` (spec in `1fcdad9`). A child is told it is a child and how to reach its orchestrator; it can write to an inbox that the orchestrator reads on its next `await_children` or `child_status`; and when a human writes in a child, the orchestrator gets one short summary instead of the human's messages. Changes in `orchestration.rs`, `registry.rs`, `child_log.rs`, `orchestration_host.rs`, `thread_view.rs`, `agent_panel.rs` and the settings files are in their "Changed again" paragraphs above.

#### [`crates/agent/src/tools/send_to_parent_tool.rs`](../../crates/agent/src/tools/send_to_parent_tool.rs)

- Scope: R15, the child-to-orchestrator tool.
- What changed: new tool `send_to_parent(message, urgent?)`; the description tells the child to use it and not its own chat, and that `urgent: true` also adds the message to the orchestrator's thread, only once it is idle.
- Why: in testing, a child asked to talk to its orchestrator answered in its own chat.
- Review focus:
  - Only a thread started by an orchestrator is offered the tool (`orchestration_tool_available`).
  - An urgent message never interrupts a running orchestrator turn.

#### [`crates/agent/src/tools/whoami_tool.rs`](../../crates/agent/src/tools/whoami_tool.rs)

- Scope: R14, the role lookup.
- What changed: new tool `whoami` returning `role` (orchestrator, child, both or none), `parent` and `children` (id, title, status).
- Why: a thread that is unsure whether someone started it can check, and reach its parent with `send_to_parent`.
- Review focus:
  - Available to every thread with a host; it returns ids and titles only.

#### [`crates/agent_ui/src/conversation_view/message_queue.rs`](../../crates/agent_ui/src/conversation_view/message_queue.rs)

- Scope: who wrote a queued message.
- What changed: `MessageOrigin { Human (default), Orchestration }` and an `origin` field on queued messages.
- Why: R16 treats a human writing in a child differently from the orchestrator's own messages (`send_to_child`, urgent messages).
- Review focus:
  - Every queue construction sets an origin; the default is `Human`.

### R17: task profiles

Commit `10530cf` (spec in `1fcdad9`). Agent profiles get a `description`; `start_child` takes an optional `profile`; the child's model resolves as: explicit `model`, the profile's `default_model`, `agent.subagent_model`, the user's default. The child's model and profile are stored in thread metadata. Changes in `orchestration.rs`, `orchestration_host.rs`, `agent_panel.rs`, `thread_metadata_store.rs`, `sidebar.rs`, `agent.rs`, `default.json`, the settings files and the tool descriptions are in their "Changed again" paragraphs above.

#### [`crates/agent_settings/src/agent_profile.rs`](../../crates/agent_settings/src/agent_profile.rs)

- Scope: the profile data type.
- What changed: `AgentProfileSettings.description: Option<SharedString>`; `AgentProfile::create` takes a description; conversions to and from the settings content carry it; test `profile_descriptions_come_from_settings` (built-in profiles have one, a user profile may set or omit it).
- Why: gives an orchestrator the text it needs to match a task to a profile.
- Review focus:
  - Existing callers pass `None` where they build a profile.

#### [`crates/agent_ui/src/agent_configuration/manage_profiles_modal.rs`](../../crates/agent_ui/src/agent_configuration/manage_profiles_modal.rs)

- Scope: the Manage Profiles UI.
- What changed: a single-line description editor with a placeholder next to the name field when creating a profile (blank becomes `None`), and the description shown under the profile in the list.
- Why: the description can be set without editing `settings.json`.
- Review focus:
  - Layout of the name and description fields; empty input.

#### [`crates/agent_ui/src/agent_configuration/tool_picker.rs`](../../crates/agent_ui/src/agent_configuration/tool_picker.rs) and [`crates/agent_ui/src/profile_selector.rs`](../../crates/agent_ui/src/profile_selector.rs)

- Scope: construction sites of a profile.
- What changed: `tool_picker.rs` keeps the profile's `description` when it rebuilds the profile; `profile_selector.rs` sets `description: None`.
- Why: the struct gained a field.
- Review focus:
  - The tool picker must not drop an existing description.

### Sort-order fix

Commit `c449f10` ("expand subthreads by default, keep order stable on open"). Relates to R9: clicking or opening a thread, or replaying a subagent's history, must never reorder the sidebar; only sending a message moves a thread to the top.

#### [`crates/acp_thread/src/acp_thread.rs`](../../crates/acp_thread/src/acp_thread.rs)

- Scope: where the "last message" time is tracked per thread.
- What changed: field `last_message_at` (added in `b7c55ae` for sorting subthreads), getter `last_message_at()` (falls back to the session's `updated_at`), `replaying_history` flag with `set_replaying_history`, `restore_last_message_at`, private `stamp_last_message` (does nothing while replaying). From `36ead50` (R5): `last_assistant_message`, which returns the text of the latest assistant turn without thoughts, tool calls or subagent output, with a test.
- Why: replayed history must not count as activity at load time, or reopening a thread would look like a new message.
- Review focus:
  - All former `Utc::now()` stamp sites now go through the replay guard.
  - `last_assistant_message` semantics: only text after the latest user message (feeds the final message in R5).

#### [`crates/agent/src/agent.rs`](../../crates/agent/src/agent.rs)

- Scope: native agent loading a saved thread.
- What changed: around the history replay (`handle_thread_events`), sets `replaying_history` true, then after replay sets it false and calls `restore_last_message_at(thread.updated_at())`; the replay result is checked after state is restored.
- Why: the loaded thread keeps the time of its real last message.
- Review focus:
  - The error from the replay is propagated only after the flags are reset.

Changed again in `10530cf` (R17): `selection_to_model_string` (formats a selection as `provider/model-id`) and `available_profiles` (id, name, description and default model of each profile), plus the `profiles` field when the agent list is built.
- Review focus:
  - The `provider/model-id` form must match what `start_child`'s `model` accepts.

#### [`crates/agent/src/thread.rs`](../../crates/agent/src/thread.rs)

- Scope: access to a private field.
- What changed: new `pub fn updated_at(&self) -> DateTime<Utc>` getter.
- Why: needed by the call above.
- Review focus:
  - Confirm `updated_at` means "last message added" (doc comment says so).

Changed again in `b1c2733`, `d303f89` and `10530cf`: `add_default_tools` registers the seven orchestration tools only for a thread without a `parent_thread_id` (never for a subagent), and `enabled_tools` drops them unless `orchestration_tool_available` says so for the session; `send_to_parent` also needs an orchestrator for the thread. New `enabled_tool_names` getter, a `profiles` field on the agent list type, and a gating test (6 tools without an orchestrator, 7 with one, 0 when the setting is off).
- Review focus:
  - Availability is decided in `enabled_tools` from the host registry; check it is not stale after a host registers or a child is created.
  - Subagents (threads with a parent) must not get the tools.

#### [`crates/agent_servers/src/acp.rs`](../../crates/agent_servers/src/acp.rs) and [`crates/agent_servers/src/acp/v1.rs`](../../crates/agent_servers/src/acp/v1.rs)

- Scope: external agents replaying history through ACP.
- What changed: `acp.rs` wraps `notification.apply` with `set_replaying_history(replaying)` and resets it afterwards; `v1.rs` reads `session.is_replaying(&session_id)` and wraps `handle_session_update` the same way, with a test that a loaded thread has `last_message_at() == None`.
- Why: the same rule as for the native agent, for external agents.
- Review focus:
  - The flag is reset on every path, including errors.
  - `is_replaying` existed already; check it is true for the whole load.

Changed again in `b1c2733`: `mcp_configurations_for_project` takes an optional `OrchestrationMcpEntry` and adds Zed's own MCP server (a custom stdio command, the bridge) to the session's MCP servers. Why: external ACP agents reach the orchestration tools only through MCP (R1).
- Review focus:
  - The entry is added only when the caller passes one (local project, orchestration enabled, Unix).

Changed again in `b1c2733`: new, loaded and resumed sessions build the MCP list with `mcp_entry_for_new_session` or `mcp_entry_for_session`; a new session's token is bound to its session id after creation; test `orchestration_server_is_added_only_when_an_entry_is_given`.
- Review focus:
  - Loading or resuming a session reuses the same token.

### Documentation

Commits `82b86e0`, `20cd02f`, `134b4c2`, `20bec29`, `1fcdad9`, and edits in `b7c55ae`, `c449f10` and `b1c2733`. These files do not affect the build.

#### [`docs-product/features/orchestrated-threads.md`](orchestrated-threads.md)

- Scope: the source of truth for requirements R1 to R17.
- What changed: new spec with summary, problem, reused code, requirements, out of scope, planned evolutions, decisions (D-series), expected files, acceptance criteria, risks and upstream path (PR split).
- Why: the code review in this file maps files to these requirements.
- Review focus:
  - That the settings table in R10 still matches `default.json`.

Changed again in `1fcdad9` (adds R14 to R17: child awareness, inbox, human summary, task profiles; revises R11, which now lets the human write in a child; adds `human_summary_max_chars` to the R10 table; E1 marked done). In `b1c2733` the how-it-works note changed only in the token paragraph.
- Review focus:
  - R11 and R16 together define what the orchestrator may learn about human messages.

#### [`docs-product/features/orchestrated-threads-r1-plan.md`](orchestrated-threads-r1-plan.md)

- Scope: implementation plan for the R1 to R5 slice.
- What changed: new document with decisions, steps, risks and tests.
- Why: records the intended design that `36ead50` implements the groundwork of.
- Review focus:
  - Steps not yet implemented in `36ead50` (tool adapters, MCP server) remain open.

#### [`docs-product/features/orchestrated-threads-how-it-works.md`](orchestrated-threads-how-it-works.md)

- Scope: explainer for how an agent opens a child conversation.
- What changed: new document: flow, what the orchestrating agent can and cannot see, why the token is not a key, why a bridge process exists.
- Why: answers recurring questions about the design before upstream review.
- Review focus:
  - Claims about visibility and tokens must match `registry.rs`.

Changed again in `b1c2733`: the bullet about the token no longer says it is "not shown to the model". It now says the token is not part of the conversation, and that a model with shell access on the same machine could read it from the bridge process environment, which only lets it act on its own orchestrator's children.
- Review focus:
  - The wording is honest about the limit and matches `bridge.rs` and `mcp.rs`.

#### [`docs-product/features/manual-review.md`](manual-review.md)

- Scope: this checklist.
- What changed: added in `20bec29` with the table and the details up to `36ead50`; extended up to `10530cf`.
- Why: lets the fork owner review every file individually before anything goes upstream.
- Review focus:
  - Only the "Manual review" column is edited by the fork owner; a "🔁" marks a file reviewed before and changed since.
