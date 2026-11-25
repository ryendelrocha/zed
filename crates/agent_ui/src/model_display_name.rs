//! Friendly names for models that only have an id.

use gpui::SharedString;

/// Brand words whose casing is not plain title case.
const BRANDS: &[(&str, &str)] = &[
    ("deepseek", "DeepSeek"),
    ("gpt", "GPT"),
    ("llama", "Llama"),
    ("qwen", "Qwen"),
    ("gemini", "Gemini"),
    ("claude", "Claude"),
];

/// Tags that say how a model is served, not which model it is.
const SERVING_TAGS: &[&str] = &["cloud", "latest", "free", "default"];

/// The name to show for a model.
///
/// `name` is what the provider or the agent reports. It is used as is when it
/// reads like a display name ("Haiku 5.5"): it has a space or an uppercase
/// letter. When it is just the id ("claude-haiku-5-5",
/// "deepseek-v4.1-flash:cloud"), it is prettified instead.
pub fn friendly_model_name(name: &str) -> SharedString {
    let name = name.trim();
    if name.contains(' ') || name.chars().any(|c| c.is_uppercase()) {
        return SharedString::from(name.to_string());
    }
    prettify_model_id(name)
}

/// Turns a model id into a readable name: `deepseek-v4.1-flash:cloud` becomes
/// "DeepSeek 4.1 Flash". Serving tags (`:cloud`, `:latest`) are dropped, a
/// size tag (`:30b`) is kept, version numbers split by dashes are joined
/// (`5-5` is "5.5") and a leading `v` before a version is dropped. An id with
/// nothing to prettify comes back as is.
pub fn prettify_model_id(id: &str) -> SharedString {
    let id = id.trim();
    // `provider/model` keeps only the model.
    let id = id.rsplit('/').next().unwrap_or(id);
    let (base, tag) = match id.split_once(':') {
        Some((base, tag)) => (base, Some(tag)),
        None => (id, None),
    };

    let mut words: Vec<String> = Vec::new();
    // Whether the last word is a bare number a following bare number extends.
    let mut last_is_number = false;
    for token in base
        .split(|c: char| c == '-' || c == '_' || c.is_whitespace())
        .filter(|token| !token.is_empty())
    {
        // Dates such as 20250514 name a snapshot, not the model.
        if token.len() == 8 && token.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let token = strip_version_prefix(token);
        let is_number = token.chars().all(|c| c.is_ascii_digit() || c == '.');
        if is_number
            && last_is_number
            && token.len() <= 2
            && let Some(previous) = words.last_mut()
        {
            previous.push('.');
            previous.push_str(token);
            continue;
        }
        last_is_number = is_number && token.len() <= 2;
        words.extend(split_brand_version(token));
    }

    if let Some(tag) = tag
        && !tag.is_empty()
        && !SERVING_TAGS.contains(&tag)
    {
        words.push(if is_size_tag(tag) {
            tag.to_uppercase()
        } else {
            title_case(tag)
        });
    }

    if words.is_empty() {
        return SharedString::from(id.to_string());
    }
    SharedString::from(words.join(" "))
}

/// `v4.1` is "4.1": a `v` followed by a digit is a version marker.
fn strip_version_prefix(token: &str) -> &str {
    match token.strip_prefix('v') {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => rest,
        _ => token,
    }
}

/// `llama3.2` is "Llama" and "3.2"; `qwen3` stays "Qwen3". Other words are
/// cased on their own.
fn split_brand_version(token: &str) -> Vec<String> {
    let split_at = token.find(|c: char| c.is_ascii_digit());
    if let Some(at) = split_at
        && at > 0
        && token[at..].contains('.')
        && token[at..].chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        return vec![title_case(&token[..at]), token[at..].to_string()];
    }
    vec![title_case(token)]
}

/// `30b`, `7b`, `270m`, `1.5b`.
fn is_size_tag(tag: &str) -> bool {
    let Some(last) = tag.chars().last() else {
        return false;
    };
    let digits = &tag[..tag.len() - last.len_utf8()];
    matches!(last.to_ascii_lowercase(), 'b' | 'm')
        && !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit() || c == '.')
}

fn title_case(word: &str) -> String {
    let lower = word.to_lowercase();
    if let Some((_, brand)) = BRANDS.iter().find(|(key, _)| *key == lower) {
        return (*brand).to_string();
    }
    let mut chars = lower.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prettify_model_ids() {
        for (id, expected) in [
            ("deepseek-v4.1-flash:cloud", "DeepSeek 4.1 Flash"),
            ("claude-haiku-5-5", "Claude Haiku 5.5"),
            ("qwen3:30b", "Qwen3 30B"),
            ("llama3.2:latest", "Llama 3.2"),
            ("gpt-5.5-mini", "GPT 5.5 Mini"),
            ("claude-sonnet-4-20250514", "Claude Sonnet 4"),
            ("anthropic/claude-3-5-sonnet", "Claude 3.5 Sonnet"),
            ("gemini_2.5_pro", "Gemini 2.5 Pro"),
        ] {
            assert_eq!(prettify_model_id(id).as_ref(), expected, "{id}");
        }
    }

    #[test]
    fn test_friendly_name_keeps_display_names() {
        assert_eq!(friendly_model_name("Haiku 5.5").as_ref(), "Haiku 5.5");
        assert_eq!(friendly_model_name("Opus").as_ref(), "Opus");
        assert_eq!(
            friendly_model_name("claude-haiku-5-5").as_ref(),
            "Claude Haiku 5.5"
        );
        assert_eq!(
            friendly_model_name("deepseek-v4.1-flash:cloud").as_ref(),
            "DeepSeek 4.1 Flash"
        );
    }
}
