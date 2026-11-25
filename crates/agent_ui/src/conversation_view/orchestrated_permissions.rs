//! Permission requests of orchestrated children, shown in the orchestrator's
//! thread (R13).
//!
//! A child is an ordinary thread with its own `AcpThread`, so its permission
//! requests live in the child's own `Conversation`. The orchestrator's thread
//! view reads them from there and shows one compact card per request. An
//! answer given on a card goes to the child's `Conversation`, exactly as if the
//! human had answered in the child. Nothing here answers on its own.
//!
//! Only the orchestrator's direct children are listed, and only the requests
//! of each child's own root session. A grandchild's request appears in its own
//! orchestrator (the child), and a child's agent-owned subagents keep their
//! existing surface inside the child.

use acp_thread::{PermissionOptions, PermissionRequestId, SelectedPermissionOutcome};
use agent_client_protocol::v2::schema as acp_v2;
use gpui::{AnyElement, SharedString, Subscription};
use orchestration::{OrchestrationRegistry, OrchestrationRegistryEvent, ThreadId};
use ui::{Tooltip, prelude::*};

use super::thread_view::ThreadView;
use crate::{AgentPanel, AgentThreadSource, orchestration_host};

/// The longest tool input shown on a card, in characters.
const INPUT_SUMMARY_MAX_CHARS: usize = 240;

/// How a button on a card answers the child's request.
#[derive(Clone)]
pub(crate) enum CardAnswer {
    /// A tool-call request: the outcome to send.
    Tool(SelectedPermissionOutcome),
    /// A generic request: the option to select.
    Option(acp_v2::PermissionOptionId),
    /// A generic request: cancel it.
    Cancel,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CardActionKind {
    Allow,
    AllowAlways,
    Deny,
}

#[derive(Clone)]
pub(crate) struct CardAction {
    pub label: SharedString,
    pub kind: CardActionKind,
    pub answer: CardAnswer,
}

/// What one card shows. It is a snapshot: it holds no reference to the child.
pub(crate) struct ChildPermissionCard {
    pub child_id: ThreadId,
    pub child_title: SharedString,
    pub agent_name: SharedString,
    pub agent_icon: IconName,
    pub agent_icon_from_external_svg: Option<SharedString>,
    pub model: Option<SharedString>,
    pub session_id: acp_v2::SessionId,
    pub request_id: PermissionRequestId,
    /// The tool, as the child shows it.
    pub tool: SharedString,
    /// A short rendering of the tool's input.
    pub input: Option<SharedString>,
    pub actions: Vec<CardAction>,
}

/// The buttons of a card for a tool-call request: Allow, "always" when the
/// request offers it, and Deny. They are the child's own options, so the
/// outcome is the one the child's prompt would produce.
pub(crate) fn tool_request_actions(options: &PermissionOptions) -> Vec<CardAction> {
    let action = |label: SharedString, kind, outcome| CardAction {
        label,
        kind,
        answer: CardAnswer::Tool(outcome),
    };
    let outcome = |option: &acp_v2::PermissionOption| {
        SelectedPermissionOutcome::new(option.option_id.clone(), option.kind.clone())
    };
    match options {
        PermissionOptions::Flat(flat) => flat
            .iter()
            .filter_map(|option| {
                let kind = match option.kind {
                    acp_v2::PermissionOptionKind::AllowOnce => CardActionKind::Allow,
                    acp_v2::PermissionOptionKind::AllowAlways => CardActionKind::AllowAlways,
                    acp_v2::PermissionOptionKind::RejectOnce => CardActionKind::Deny,
                    _ => return None,
                };
                Some(action(option.name.clone().into(), kind, outcome(option)))
            })
            .collect(),
        PermissionOptions::Dropdown(choices)
        | PermissionOptions::DropdownWithPatterns { choices, .. } => {
            // The last choice is "only this time"; the first, when there is
            // more than one, is the broadest "always".
            let mut actions = Vec::new();
            if let Some(once) = choices.last() {
                actions.push(action(
                    "Allow".into(),
                    CardActionKind::Allow,
                    once.build_outcome(true),
                ));
            }
            if choices.len() > 1
                && let Some(always) = choices.first()
            {
                actions.push(action(
                    always.label(),
                    CardActionKind::AllowAlways,
                    always.build_outcome(true),
                ));
            }
            if let Some(once) = choices.last() {
                actions.push(action(
                    "Deny".into(),
                    CardActionKind::Deny,
                    once.build_outcome(false),
                ));
            }
            actions
        }
    }
}

fn summarize_input(input: &serde_json::Value) -> Option<SharedString> {
    let text = match input {
        serde_json::Value::Null => return None,
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }
    if text.chars().count() <= INPUT_SUMMARY_MAX_CHARS {
        return Some(text.into());
    }
    let truncated: String = text.chars().take(INPUT_SUMMARY_MAX_CHARS).collect();
    Some(format!("{truncated}…").into())
}

impl ThreadView {
    /// One card per pending permission request of this thread's orchestrated
    /// children. Empty for a thread that orchestrates nothing and for subagent
    /// sessions.
    pub(crate) fn orchestrated_child_permission_cards(&self, cx: &App) -> Vec<ChildPermissionCard> {
        if self.parent_session_id.is_some() {
            return Vec::new();
        }
        let Some(registry) = OrchestrationRegistry::try_global(cx) else {
            return Vec::new();
        };
        let mut cards = Vec::new();
        for record in registry.read(cx).children(self.root_thread_id) {
            let Some(child_view) = orchestration_host::child_conversation_view(record.child_id, cx)
            else {
                continue;
            };
            let Some(child_thread_view) = child_view.read(cx).root_thread_view() else {
                continue;
            };
            let child_thread_view = child_thread_view.read(cx);
            let session_id = child_thread_view.session_id.clone();
            let thread = child_thread_view.thread.read(cx);
            let conversation = child_thread_view.conversation.read(cx);
            let Some(request_ids) = conversation.permission_requests.get(&session_id) else {
                continue;
            };
            let model = record
                .model
                .clone()
                .map(SharedString::from)
                .or_else(|| child_thread_view.active_model(cx).map(|model| model.name));
            for request_id in request_ids {
                let Some(request) = thread.permission_request(*request_id) else {
                    continue;
                };
                let (tool, input, actions) = if let Some(options) = request.legacy_options() {
                    let tool_call = request
                        .legacy_tool_call_id()
                        .and_then(|id| thread.tool_call(id))
                        .map(|(_, tool_call)| tool_call);
                    let tool = tool_call
                        .map(|tool_call| tool_call.label.read(cx).source().to_string())
                        .filter(|label| !label.is_empty())
                        .unwrap_or_else(|| "Tool request".to_string());
                    let input = tool_call
                        .and_then(|tool_call| tool_call.raw_input.as_ref())
                        .and_then(summarize_input);
                    (tool.into(), input, tool_request_actions(options))
                } else if let Some(request) = request.generic_request() {
                    let mut actions: Vec<CardAction> = request
                        .options
                        .iter()
                        .map(|option| CardAction {
                            label: option.name.clone().into(),
                            kind: match option.kind {
                                acp_v2::PermissionOptionKind::AllowOnce => CardActionKind::Allow,
                                acp_v2::PermissionOptionKind::AllowAlways => {
                                    CardActionKind::AllowAlways
                                }
                                _ => CardActionKind::Deny,
                            },
                            answer: CardAnswer::Option(option.option_id.clone()),
                        })
                        .collect();
                    actions.push(CardAction {
                        label: "Cancel".into(),
                        kind: CardActionKind::Deny,
                        answer: CardAnswer::Cancel,
                    });
                    (
                        SharedString::from(request.title.clone()),
                        request.description.as_deref().and_then(|description| {
                            summarize_input(&serde_json::Value::String(description.to_string()))
                        }),
                        actions,
                    )
                } else {
                    continue;
                };
                cards.push(ChildPermissionCard {
                    child_id: record.child_id,
                    child_title: record.title.clone(),
                    agent_name: child_thread_view.agent_display_name.clone(),
                    agent_icon: child_thread_view.agent_icon,
                    agent_icon_from_external_svg: child_thread_view
                        .agent_icon_from_external_svg
                        .clone(),
                    model: model.clone(),
                    session_id: session_id.clone(),
                    request_id: *request_id,
                    tool,
                    input,
                    actions,
                });
            }
        }
        cards
    }

    /// Sends the human's answer to the child's request. It is the same call the
    /// child's own prompt makes, so the child's tool call, its sidebar row and
    /// the orchestrator registry all follow from it.
    pub(crate) fn answer_child_permission(
        &self,
        child_id: ThreadId,
        session_id: &acp_v2::SessionId,
        request_id: PermissionRequestId,
        answer: CardAnswer,
        cx: &mut Context<Self>,
    ) {
        let Some(child_thread_view) = orchestration_host::child_conversation_view(child_id, cx)
            .and_then(|view| view.read(cx).root_thread_view())
        else {
            return;
        };
        let conversation = child_thread_view.read(cx).conversation.clone();
        conversation.update(cx, |conversation, cx| match answer {
            CardAnswer::Tool(outcome) => {
                conversation.authorize_permission_request(
                    session_id.clone(),
                    request_id,
                    outcome,
                    cx,
                );
            }
            CardAnswer::Option(option_id) => {
                conversation.select_permission_option(session_id, request_id, option_id, cx);
            }
            CardAnswer::Cancel => {
                conversation.cancel_permission_request(session_id, request_id, cx);
            }
        });
        cx.notify();
    }

    /// Shows the child thread, as the sidebar row does.
    fn open_orchestrated_child(&self, child_id: ThreadId, window: &mut Window, cx: &mut App) {
        let Some(child_view) = orchestration_host::child_conversation_view(child_id, cx) else {
            return;
        };
        let (agent, workspace) = {
            let child_view = child_view.read(cx);
            (
                child_view.connection_key.clone(),
                child_view.workspace.upgrade(),
            )
        };
        let Some(workspace) = workspace else {
            return;
        };
        let Some(panel) = workspace.read(cx).panel::<AgentPanel>(cx) else {
            return;
        };
        panel.update(cx, |panel, cx| {
            panel.load_agent_thread(
                agent,
                child_id,
                None,
                None,
                true,
                AgentThreadSource::AgentPanel,
                window,
                cx,
            );
        });
    }

    /// Watches the children of this thread so the cards follow their requests:
    /// a card appears when a child asks and goes away when the request is
    /// answered, here or in the child.
    pub(crate) fn watch_orchestrated_children(
        &mut self,
        subscriptions: &mut Vec<Subscription>,
        cx: &mut Context<Self>,
    ) {
        let registry = OrchestrationRegistry::global_or_init(cx);
        subscriptions.push(cx.subscribe(
            &registry,
            |this, _registry, event: &OrchestrationRegistryEvent, cx| {
                let OrchestrationRegistryEvent::ChildChanged { orchestrator, .. } = event;
                if *orchestrator == this.root_thread_id && this.parent_session_id.is_none() {
                    this.sync_orchestrated_child_watches(cx);
                    cx.notify();
                }
            },
        ));
    }

    /// Subscribes to the `AcpThread` of every child not watched yet, and drops
    /// the watches of children that are gone.
    pub(crate) fn sync_orchestrated_child_watches(&mut self, cx: &mut Context<Self>) {
        let Some(registry) = OrchestrationRegistry::try_global(cx) else {
            return;
        };
        let children: Vec<ThreadId> = registry
            .read(cx)
            .children(self.root_thread_id)
            .iter()
            .map(|record| record.child_id)
            .collect();
        self.orchestrated_child_watches
            .retain(|child, _| children.contains(child));
        for child in children {
            if self.orchestrated_child_watches.contains_key(&child) {
                continue;
            }
            let Some(thread) = orchestration_host::child_conversation_view(child, cx)
                .and_then(|view| view.read(cx).root_thread(cx))
            else {
                continue;
            };
            let subscription = cx.subscribe(
                &thread,
                |_this, _thread, event: &acp_thread::AcpThreadEvent, cx| {
                    if matches!(
                        event,
                        acp_thread::AcpThreadEvent::ToolAuthorizationRequested(_)
                            | acp_thread::AcpThreadEvent::ToolAuthorizationReceived(_)
                            | acp_thread::AcpThreadEvent::StatusChanged
                            | acp_thread::AcpThreadEvent::ElicitationRequested(_)
                            | acp_thread::AcpThreadEvent::ElicitationResponded(_)
                    ) {
                        cx.notify();
                    }
                },
            );
            self.orchestrated_child_watches.insert(child, subscription);
        }
    }

    /// The cards of the orchestrated children's permission requests, in a
    /// block that sits with the other awaiting-permission prompts.
    pub(crate) fn render_orchestrated_child_permissions(
        &self,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let cards = self.orchestrated_child_permission_cards(cx);
        if cards.is_empty() {
            return None;
        }
        Some(
            v_flex()
                .id("orchestrated-child-permissions")
                .debug_selector(|| "orchestrated-child-permissions".to_string())
                .max_h(px(320.0))
                .overflow_y_scroll()
                .children(
                    cards
                        .into_iter()
                        .map(|card| self.render_child_card(card, cx)),
                )
                .into_any(),
        )
    }

    fn render_child_card(&self, card: ChildPermissionCard, cx: &Context<Self>) -> AnyElement {
        let ChildPermissionCard {
            child_id,
            child_title,
            agent_name,
            agent_icon,
            agent_icon_from_external_svg,
            model,
            session_id,
            request_id,
            tool,
            input,
            actions,
        } = card;
        let card_id = format!("orchestrated-child-permission-{request_id:?}");
        let agent_icon_element = match agent_icon_from_external_svg {
            Some(path) => Icon::from_external_svg(path),
            None => Icon::new(agent_icon),
        }
        .size(IconSize::XSmall)
        .color(Color::Muted);
        let details = match model {
            Some(model) => format!("{agent_name} · {model}"),
            None => agent_name.to_string(),
        };

        v_flex()
            .id(card_id.clone())
            .debug_selector({
                let card_id = card_id.clone();
                move || card_id.clone()
            })
            .p_2()
            .gap_1p5()
            .w_full()
            .min_w_0()
            .border_b_1()
            .border_color(cx.theme().colors().border_variant)
            .text_ui_sm(cx)
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap_1p5()
                    .justify_between()
                    .child(
                        h_flex()
                            .min_w_0()
                            .gap_1p5()
                            .child(agent_icon_element)
                            .child(Label::new(child_title).size(LabelSize::Small).truncate())
                            .child(
                                Label::new(details)
                                    .size(LabelSize::Small)
                                    .color(Color::Muted)
                                    .truncate(),
                            ),
                    )
                    .child(
                        Button::new(format!("{card_id}-open"), "Open")
                            .label_size(LabelSize::Small)
                            .end_icon(
                                Icon::new(IconName::ArrowUpRight)
                                    .size(IconSize::XSmall)
                                    .color(Color::Muted),
                            )
                            .tooltip(Tooltip::text("Open this thread to answer there"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_orchestrated_child(child_id, window, cx);
                            })),
                    ),
            )
            .child(
                Label::new(format!("Wants permission: {tool}"))
                    .size(LabelSize::Small)
                    .truncate(),
            )
            .children(input.map(|input| {
                div()
                    .w_full()
                    .min_w_0()
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .bg(cx.theme().colors().editor_background)
                    .font_buffer(cx)
                    .text_color(cx.theme().colors().text_muted)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(input)
            }))
            .child(h_flex().w_full().min_w_0().gap_1().flex_wrap().children(
                actions.into_iter().enumerate().map(|(ix, action)| {
                    let session_id = session_id.clone();
                    let answer = action.answer.clone();
                    let mut button =
                        Button::new(format!("{card_id}-action-{ix}"), action.label.clone())
                            .label_size(LabelSize::Small)
                            .truncate(true)
                            .tooltip(Tooltip::text(action.label));
                    if action.kind == CardActionKind::Allow {
                        button = button.style(ButtonStyle::Tinted(ui::TintColor::Success));
                    }
                    button.on_click(cx.listener(move |this, _, _, cx| {
                        this.answer_child_permission(
                            child_id,
                            &session_id,
                            request_id,
                            answer.clone(),
                            cx,
                        );
                    }))
                }),
            ))
            .into_any()
    }
}
