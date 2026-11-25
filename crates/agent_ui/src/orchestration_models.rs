//! The models of external agents for orchestration (R2).
//!
//! An external ACP agent offers its models as a session config option (the
//! one the model selector shows). This module reads that option, matches the
//! `model` an orchestrator asks for against it, applies it through
//! `session/set_config_option` before the child's first prompt, and keeps the
//! last list each agent reported so `list_agents_and_models` and `start_child`
//! can use it when no session of that agent is open.

use std::{cell::RefCell, future::Future, rc::Rc};

use acp_thread::AgentSessionConfigOptions;
use agent_client_protocol::v2::schema as acp;
use anyhow::{Result, anyhow};
use collections::HashMap;
use futures::{FutureExt as _, future::Either};
use gpui::{App, AsyncApp, Entity, Global};

use crate::conversation_view::ConversationView;

/// How long a child's first prompt waits for its model to be set before it is
/// sent anyway.
pub(crate) const MODEL_APPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// One model of an external agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExternalModel {
    /// The value of the model config option, and what `start_child` takes.
    pub value: String,
    pub name: String,
}

/// The model option of a session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModelOption {
    pub config_id: acp::SessionConfigId,
    pub models: Vec<ExternalModel>,
    /// The model the session started with.
    pub current: Option<String>,
}

/// The model config option of a session: the select of the `model` category,
/// else the select called `model`.
pub(crate) fn model_option(options: &[acp::SessionConfigOption]) -> Option<ModelOption> {
    let option = options
        .iter()
        .find(|option| {
            matches!(option.kind, acp::SessionConfigKind::Select(_))
                && option.category == Some(acp::SessionConfigOptionCategory::Model)
        })
        .or_else(|| {
            options.iter().find(|option| {
                matches!(option.kind, acp::SessionConfigKind::Select(_))
                    && option.config_id.to_string() == "model"
            })
        })?;
    let acp::SessionConfigKind::Select(select) = &option.kind else {
        return None;
    };
    let models = match &select.options {
        acp::SessionConfigSelectOptions::Ungrouped(options) => options
            .iter()
            .map(|option| ExternalModel {
                value: option.value.to_string(),
                name: option.name.to_string(),
            })
            .collect(),
        acp::SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .flat_map(|group| group.options.iter())
            .map(|option| ExternalModel {
                value: option.value.to_string(),
                name: option.name.to_string(),
            })
            .collect(),
        _ => Vec::new(),
    };
    Some(ModelOption {
        config_id: option.config_id.clone(),
        models,
        current: Some(select.current_value.to_string()),
    })
}

/// The `model` of `start_child` without the agent prefix: for an external
/// agent a model may be `model-id` or `agent-id/model-id`.
pub(crate) fn strip_agent_prefix<'a>(agent_id: &str, model: &'a str) -> &'a str {
    model
        .strip_prefix(agent_id)
        .and_then(|rest| rest.strip_prefix('/'))
        .filter(|rest| !rest.is_empty())
        .unwrap_or(model)
}

/// Finds the model of an agent that `requested` names: the exact value or
/// name (ignoring case), else the one model whose value or name contains it
/// as a word ("haiku" for "Claude Haiku 4.5"). The error lists the models.
pub(crate) fn resolve_model<'a>(
    agent_id: &str,
    models: &'a [ExternalModel],
    requested: &str,
) -> Result<&'a ExternalModel, String> {
    let requested = strip_agent_prefix(agent_id, requested.trim());
    let wanted = requested.to_lowercase();
    if let Some(exact) = models
        .iter()
        .find(|model| model.value == requested)
        .or_else(|| {
            models.iter().find(|model| {
                model.value.to_lowercase() == wanted || model.name.to_lowercase() == wanted
            })
        })
    {
        return Ok(exact);
    }
    let words = |text: &str| -> Vec<String> {
        text.to_lowercase()
            .split(|character: char| !character.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_string)
            .collect()
    };
    let wanted_words = words(requested);
    let matches = models
        .iter()
        .filter(|model| {
            !wanted_words.is_empty() && {
                let haystack = words(&format!("{} {}", model.value, model.name));
                wanted_words.iter().all(|word| haystack.contains(word))
            }
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [only] => Ok(only),
        [] => Err(unknown_model_error(agent_id, requested, models)),
        several => Err(format!(
            "The model {requested:?} is ambiguous for agent {agent_id:?}. It matches: {}. \
             Use the exact id.",
            describe_models(several.iter().copied())
        )),
    }
}

fn describe_models<'a>(models: impl Iterator<Item = &'a ExternalModel>) -> String {
    models
        .map(|model| format!("{:?} ({})", model.value, model.name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The error for a model that the agent does not offer.
pub(crate) fn unknown_model_error(
    agent_id: &str,
    requested: &str,
    models: &[ExternalModel],
) -> String {
    if models.is_empty() {
        return no_model_option_error(agent_id, requested);
    }
    format!(
        "Unknown model {requested:?} for agent {agent_id:?}. Available models: {}. \
         Call `list_agents_and_models` to see them.",
        describe_models(models.iter())
    )
}

/// The error for an agent that offers no model to choose.
pub(crate) fn no_model_option_error(agent_id: &str, requested: &str) -> String {
    format!(
        "Agent {agent_id:?} does not let the client choose a model, so the model {requested:?} \
         cannot be applied. Leave `model` out to use the agent's own model."
    )
}

/// The models each external agent reported, by agent id. It is the cache that
/// `start_child` and `list_agents_and_models` fall back to when no session of
/// the agent is open. It lives for the app session only.
#[derive(Default)]
pub(crate) struct ExternalAgentModels(RefCell<HashMap<String, ModelOption>>);

impl Global for ExternalAgentModels {}

pub(crate) fn init(cx: &mut App) {
    if !cx.has_global::<ExternalAgentModels>() {
        cx.set_global(ExternalAgentModels::default());
    }
}

/// Remembers the models of `agent_id`. An empty list is not remembered.
pub(crate) fn remember(agent_id: &str, option: &ModelOption, cx: &App) {
    if option.models.is_empty() {
        return;
    }
    if let Some(cache) = cx.try_global::<ExternalAgentModels>() {
        cache
            .0
            .borrow_mut()
            .insert(agent_id.to_string(), option.clone());
    }
}

pub(crate) fn cached(agent_id: &str, cx: &App) -> Option<ModelOption> {
    cx.try_global::<ExternalAgentModels>()?
        .0
        .borrow()
        .get(agent_id)
        .cloned()
}

/// The models to list for an external agent, and a note that says where they
/// come from. An open session of the agent gives the live list (and refreshes
/// the cache); else the last list the agent reported; else none.
pub(crate) fn available_models(
    agent_id: &str,
    views: &[Entity<ConversationView>],
    cx: &App,
) -> (Vec<agent::AvailableModel>, String) {
    let live = views.iter().find_map(|view| {
        let thread = view.read(cx).root_thread(cx)?;
        let thread = thread.read(cx);
        let connection = thread.connection();
        if connection.agent_id().0.as_ref() != agent_id {
            return None;
        }
        let options = connection.session_config_options(thread.session_id(), cx)?;
        model_option(&options.config_options())
    });
    let (option, note) = match live {
        Some(option) => {
            remember(agent_id, &option, cx);
            (
                Some(option),
                "Read from an open session of this agent. Pass an id as `model`.",
            )
        }
        None => match cached(agent_id, cx) {
            Some(option) => (
                Some(option),
                "From the last session of this agent in this Zed run, so it may be out of \
                 date. Pass an id as `model`.",
            ),
            None => (
                None,
                "This agent reports its models when a session connects, and none has yet. \
                 `model` is still accepted: Zed checks it and sets it when the child connects, \
                 and an unknown model stops the child with the list of the agent's models.",
            ),
        },
    };
    let models = option
        .map(|option| {
            option
                .models
                .into_iter()
                .map(|model| agent::AvailableModel {
                    is_default: option.current.as_deref() == Some(model.value.as_str()),
                    id: model.value,
                    name: model.name.into(),
                })
                .collect()
        })
        .unwrap_or_default();
    (models, note.to_string())
}

/// What `start_child` learned about the model of an external child.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AppliedModel {
    pub value: String,
    pub name: String,
}

/// How the wait for the child's model ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ModelGateOutcome {
    /// The model is set: the first prompt can go.
    Applied(AppliedModel),
    /// The model cannot be used: the child must not start.
    Rejected(String),
    /// The agent did not answer in time: the prompt goes anyway.
    TimedOut,
}

/// Sets the model of a session through its model config option, and gives up
/// after `timeout`. It does not send anything: the caller sends the first
/// prompt when this returns, so the model is set before it.
pub(crate) async fn apply_model_before_prompt(
    options: Option<Rc<dyn AgentSessionConfigOptions>>,
    agent_id: &str,
    requested: &str,
    timeout: impl Future<Output = ()>,
    cx: &mut AsyncApp,
) -> ModelGateOutcome {
    let Some(options) = options else {
        return ModelGateOutcome::Rejected(no_model_option_error(agent_id, requested));
    };
    let apply = apply_model(options, agent_id, requested, cx);
    futures::pin_mut!(apply);
    futures::pin_mut!(timeout);
    match futures::future::select(apply, timeout.fuse()).await {
        Either::Left((Ok(applied), _)) => ModelGateOutcome::Applied(applied),
        Either::Left((Err(error), _)) => ModelGateOutcome::Rejected(format!("{error:#}")),
        Either::Right(_) => ModelGateOutcome::TimedOut,
    }
}

async fn apply_model(
    options: Rc<dyn AgentSessionConfigOptions>,
    agent_id: &str,
    requested: &str,
    cx: &mut AsyncApp,
) -> Result<AppliedModel> {
    let Some(option) = model_option(&options.config_options()) else {
        return Err(anyhow!(no_model_option_error(agent_id, requested)));
    };
    cx.update(|cx| remember(agent_id, &option, cx));
    let model =
        resolve_model(agent_id, &option.models, requested).map_err(|error| anyhow!(error))?;
    let applied = AppliedModel {
        value: model.value.clone(),
        name: model.name.clone(),
    };
    if option.current.as_deref() != Some(applied.value.as_str()) {
        let value = acp::SessionConfigOptionValue::id(applied.value.clone());
        let task = cx.update(|cx| options.set_config_option(option.config_id.clone(), value, cx));
        let updated = task.await.map_err(|error| {
            anyhow!("The agent refused the model {:?}: {error:#}", applied.value)
        })?;
        cx.update(|cx| {
            if let Some(option) = model_option(&updated) {
                remember(agent_id, &option, cx);
            }
        });
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claude_models() -> Vec<ExternalModel> {
        [
            ("default", "Default (recommended)"),
            ("sonnet", "Claude Sonnet 4.5"),
            ("opus", "Claude Opus 4.5"),
            ("haiku", "Claude Haiku 4.5"),
        ]
        .into_iter()
        .map(|(value, name)| ExternalModel {
            value: value.into(),
            name: name.into(),
        })
        .collect()
    }

    pub(crate) fn model_config_option(current: &str) -> acp::SessionConfigOption {
        acp::SessionConfigOption::select(
            "model",
            "Model",
            current,
            claude_models()
                .into_iter()
                .map(|model| acp::SessionConfigSelectOption::new(model.value, model.name))
                .collect::<Vec<_>>(),
        )
        .category(acp::SessionConfigOptionCategory::Model)
    }

    #[test]
    fn test_reads_the_model_option_among_other_options() {
        let options = vec![
            acp::SessionConfigOption::select(
                "mode",
                "Mode",
                "ask",
                vec![acp::SessionConfigSelectOption::new("ask", "Ask")],
            )
            .category(acp::SessionConfigOptionCategory::Mode),
            model_config_option("sonnet"),
        ];
        let option = model_option(&options).expect("the model option");
        assert_eq!(option.config_id.to_string(), "model");
        assert_eq!(option.current.as_deref(), Some("sonnet"));
        assert_eq!(option.models, claude_models());
        assert!(model_option(&options[..1]).is_none());
    }

    #[test]
    fn test_resolves_a_family_name_an_id_and_the_prefixed_form() {
        let models = claude_models();
        for requested in [
            "haiku",
            "Haiku",
            "claude-acp/haiku",
            "Claude Haiku 4.5",
            " haiku ",
        ] {
            assert_eq!(
                resolve_model("claude-acp", &models, requested)
                    .unwrap()
                    .value,
                "haiku",
                "{requested}"
            );
        }
        assert_eq!(
            resolve_model("claude-acp", &models, "OPUS").unwrap().value,
            "opus"
        );
        assert_eq!(
            resolve_model("claude-acp", &models, "claude sonnet")
                .unwrap()
                .value,
            "sonnet"
        );
    }

    #[test]
    fn test_unknown_model_error_lists_the_agents_models() {
        let error = resolve_model("claude-acp", &claude_models(), "gpt-5").unwrap_err();
        assert!(error.contains("Unknown model \"gpt-5\""), "{error}");
        assert!(error.contains("claude-acp"), "{error}");
        for model in ["default", "sonnet", "opus", "haiku"] {
            assert!(error.contains(&format!("{model:?}")), "{error}");
        }
        assert!(error.contains("list_agents_and_models"), "{error}");
    }

    #[test]
    fn test_ambiguous_model_asks_for_the_exact_id() {
        let models = vec![
            ExternalModel {
                value: "sonnet-fast".into(),
                name: "Sonnet fast".into(),
            },
            ExternalModel {
                value: "sonnet-slow".into(),
                name: "Sonnet slow".into(),
            },
        ];
        let error = resolve_model("claude-acp", &models, "sonnet").unwrap_err();
        assert!(error.contains("ambiguous"), "{error}");
        assert!(
            error.contains("sonnet-fast") && error.contains("sonnet-slow"),
            "{error}"
        );
    }

    #[test]
    fn test_an_agent_without_models_says_so() {
        let error = unknown_model_error("codex-acp", "haiku", &[]);
        assert!(
            error.contains("does not let the client choose a model"),
            "{error}"
        );
    }
}
