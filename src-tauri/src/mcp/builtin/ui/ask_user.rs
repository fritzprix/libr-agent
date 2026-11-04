//! `ui__askUser` (#2547): validate the questions, then turn the user's answer
//! into the tool result. The wait itself is the shared user prompt (#2543).

use crate::agent::state::{UserPromptOutcome, UserQuestion};
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

pub const MAX_QUESTIONS: usize = 5;

/// Argument keys that ask for hidden input. Secrets go through the shell
/// `storeAs` path (#2544) so the value never enters the LLM context.
const SECRET_KEYS: [&str; 3] = ["password", "secret", "inputType"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AskUserArgs {
    questions: Vec<AskUserQuestionArg>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AskUserQuestionArg {
    id: String,
    question: String,
    description: Option<String>,
    options: Option<Vec<String>>,
    multi_select: Option<bool>,
    allow_custom: Option<bool>,
}

/// A submitted answer: a string, or a list of strings for a multi-select.
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum AskUserAnswer {
    One(String),
    Many(Vec<String>),
}

fn invalid(message: impl Into<String>, guidance: &[&str]) -> MCPResult {
    guided_error(ErrorCategory::InvalidInput, message.into(), ToolGroup::UI)
        .with_guidance(guidance.iter().map(|line| line.to_string()).collect())
        .to_mcp_result()
}

fn has_secret_key(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|map| SECRET_KEYS.iter().any(|key| map.contains_key(*key)))
}

/// Validate the tool arguments and return the questions to ask.
pub fn parse_ask_user_args(args: &Value) -> Result<Vec<UserQuestion>, MCPResult> {
    let items_have_secret_key = args
        .get("questions")
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(has_secret_key));
    if has_secret_key(args) || items_have_secret_key {
        return Err(invalid(
            "ui__askUser cannot collect passwords or other secrets.",
            &["Use workspace__runShell with requireUserInput=true, inputType=password, and storeAs to ask for a secret."],
        ));
    }

    let parsed: AskUserArgs = serde_json::from_value(args.clone()).map_err(|error| {
        invalid(
            format!("Invalid ui__askUser arguments: {error}"),
            &[
                "Pass {\"questions\": [{\"id\": \"...\", \"question\": \"...\"}]}",
                "Allowed item fields: id, question, description, options, multiSelect, allowCustom",
            ],
        )
    })?;

    if parsed.questions.is_empty() || parsed.questions.len() > MAX_QUESTIONS {
        return Err(invalid(
            format!("questions must have 1 to {MAX_QUESTIONS} items"),
            &["Ask only what you need now; ask again later if needed."],
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut questions = Vec::with_capacity(parsed.questions.len());
    for item in parsed.questions {
        let id = item.id.trim().to_string();
        if id.is_empty() || item.question.trim().is_empty() {
            return Err(invalid(
                "Each question needs a non-empty id and question",
                &[],
            ));
        }
        if !seen_ids.insert(id.clone()) {
            return Err(invalid(
                format!("Duplicate question id '{id}'"),
                &["Give every question a unique id."],
            ));
        }
        if item.options.as_ref().is_some_and(Vec::is_empty) {
            return Err(invalid(
                format!("Question '{id}' has an empty options list"),
                &["Omit options for a free-text question."],
            ));
        }
        if let Some(options) = &item.options {
            let mut seen_options = HashSet::new();
            if !options.iter().all(|option| seen_options.insert(option)) {
                return Err(invalid(
                    format!("Question '{id}' has duplicate options"),
                    &["Give every option different text."],
                ));
            }
        }
        let has_options = item.options.is_some();
        let multi_select = item.multi_select.unwrap_or(false);
        if multi_select && !has_options {
            return Err(invalid(
                format!("Question '{id}' sets multiSelect without options"),
                &["Add options, or remove multiSelect for a free-text question."],
            ));
        }

        questions.push(UserQuestion {
            id,
            question: item.question,
            description: item.description,
            options: item.options,
            multi_select,
            allow_custom: has_options && item.allow_custom.unwrap_or(true),
        });
    }

    Ok(questions)
}

/// Build the result object: `{ "<id>": { "question": ..., "answer": ... } }`.
///
/// The frontend sends a JSON object keyed by question id. Every question must
/// have a non-empty answer of the right shape.
pub fn format_ask_user_answer(questions: &[UserQuestion], raw: &str) -> Result<String, String> {
    let answers: HashMap<String, AskUserAnswer> =
        serde_json::from_str(raw).map_err(|error| format!("Malformed answer: {error}"))?;

    let mut result = Map::new();
    for question in questions {
        let answer = answers
            .get(&question.id)
            .ok_or_else(|| format!("Missing answer for question '{}'", question.id))?;
        let is_allowed = |value: &String| {
            !value.trim().is_empty()
                && (question.allow_custom
                    || question
                        .options
                        .as_ref()
                        .is_none_or(|options| options.contains(value)))
        };
        let answered = match (answer, question.multi_select) {
            (AskUserAnswer::One(text), false) => is_allowed(text),
            (AskUserAnswer::Many(values), true) => {
                !values.is_empty() && values.iter().all(is_allowed)
            }
            _ => false,
        };
        if !answered {
            return Err(format!("Invalid answer for question '{}'", question.id));
        }
        result.insert(
            question.id.clone(),
            json!({ "question": question.question, "answer": answer }),
        );
    }

    serde_json::to_string(&Value::Object(result)).map_err(|error| error.to_string())
}

/// The tool result for each way the prompt can end.
pub fn ask_user_outcome_result(
    questions: &[UserQuestion],
    outcome: &UserPromptOutcome,
) -> MCPResult {
    match outcome {
        UserPromptOutcome::Answered(raw) => match format_ask_user_answer(questions, raw) {
            Ok(text) => MCPResult::success(&text),
            Err(message) => MCPResult::error(&message),
        },
        UserPromptOutcome::Cancelled | UserPromptOutcome::Stopped => {
            MCPResult::informational(&outcome.caller_message())
        }
        UserPromptOutcome::TimedOut | UserPromptOutcome::Restarted => {
            MCPResult::error(&outcome.caller_message())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::state::{
        USER_PROMPT_CANCELLED_MESSAGE, USER_PROMPT_RESTART_MESSAGE, USER_PROMPT_TIMEOUT_MESSAGE,
    };

    fn result_text(result: &MCPResult) -> String {
        serde_json::to_string(result).expect("result serializes")
    }

    fn three_questions() -> Vec<UserQuestion> {
        parse_ask_user_args(&json!({
            "questions": [
                { "id": "name", "question": "Project name?" },
                { "id": "lang", "question": "Language?", "options": ["Rust", "TS"] },
                {
                    "id": "targets",
                    "question": "Targets?",
                    "options": ["linux", "mac", "win"],
                    "multiSelect": true,
                    "allowCustom": false
                }
            ]
        }))
        .expect("valid questions")
    }

    #[test]
    fn options_decide_the_question_type_and_allow_custom_defaults_on() {
        let questions = three_questions();

        assert_eq!(questions[0].options, None);
        assert!(!questions[0].allow_custom);
        assert!(!questions[1].multi_select);
        assert!(questions[1].allow_custom);
        assert!(questions[2].multi_select);
        assert!(!questions[2].allow_custom);
    }

    #[test]
    fn secret_keys_point_to_the_shell_store_as_path() {
        for args in [
            json!({ "questions": [{ "id": "a", "question": "Q?" }], "password": true }),
            json!({ "questions": [{ "id": "a", "question": "Q?", "inputType": "password" }] }),
            json!({ "questions": [{ "id": "a", "question": "Q?", "secret": true }] }),
        ] {
            let error = parse_ask_user_args(&args).expect_err("secret keys are refused");
            let text = result_text(&error);
            assert!(text.contains("workspace__runShell"), "{text}");
            assert!(text.contains("requireUserInput=true"), "{text}");
            assert!(text.contains("storeAs"), "{text}");
        }
    }

    #[test]
    fn the_word_password_in_question_text_is_allowed() {
        let questions = parse_ask_user_args(&json!({
            "questions": [{ "id": "a", "question": "Should the password policy change?" }]
        }))
        .expect("text content is not a key");
        assert_eq!(questions.len(), 1);
    }

    #[test]
    fn invalid_question_lists_are_refused() {
        let too_many: Vec<Value> = (0..6)
            .map(|i| json!({ "id": format!("q{i}"), "question": "Q?" }))
            .collect();
        for args in [
            json!({ "questions": [] }),
            json!({ "questions": too_many }),
            json!({ "questions": [{ "id": "a", "question": "Q?" }, { "id": "a", "question": "R?" }] }),
            json!({ "questions": [{ "id": "a", "question": "Q?", "type": "text" }] }),
            json!({ "questions": [{ "id": "a", "question": "Q?", "multiSelect": true }] }),
            json!({ "questions": [{ "id": "a", "question": "Q?", "options": [] }] }),
            json!({ "questions": [{ "id": "a", "question": "Q?", "options": ["x", "x"] }] }),
            json!({ "questions": [{ "id": " ", "question": "Q?" }] }),
            json!({}),
        ] {
            let error = parse_ask_user_args(&args).expect_err("invalid args are refused");
            assert_eq!(error.is_error, Some(true), "{args}");
        }
    }

    #[test]
    fn an_answer_returns_one_object_keyed_by_id() {
        let questions = three_questions();
        let raw = json!({
            "name": "libr",
            "lang": "Go",
            "targets": ["linux", "win"]
        })
        .to_string();

        let result = ask_user_outcome_result(&questions, &UserPromptOutcome::Answered(raw.clone()));
        assert_eq!(result.is_error, Some(false));

        let text = format_ask_user_answer(&questions, &raw).expect("valid answer");
        let parsed: Value = serde_json::from_str(&text).expect("result is JSON");
        assert_eq!(
            parsed,
            json!({
                "name": { "question": "Project name?", "answer": "libr" },
                "lang": { "question": "Language?", "answer": "Go" },
                "targets": { "question": "Targets?", "answer": ["linux", "win"] }
            })
        );
    }

    #[test]
    fn missing_or_misshaped_answers_are_errors() {
        let questions = three_questions();
        for raw in [
            json!({ "name": "libr", "lang": "Go" }).to_string(),
            json!({ "name": "", "lang": "Go", "targets": ["linux"] }).to_string(),
            json!({ "name": "libr", "lang": ["Go"], "targets": ["linux"] }).to_string(),
            json!({ "name": "libr", "lang": "Go", "targets": [] }).to_string(),
            json!({ "name": "libr", "lang": "Go", "targets": ["bsd"] }).to_string(),
            "not json".to_string(),
        ] {
            assert!(format_ask_user_answer(&questions, &raw).is_err(), "{raw}");
        }
    }

    #[tokio::test]
    async fn the_ui_server_lists_and_dispatches_ask_user() {
        use crate::mcp::builtin::ui::UiServer;
        use crate::mcp::builtin::BuiltinMCPServer;

        let server = UiServer::new();
        let names: Vec<String> = server.tools().into_iter().map(|tool| tool.name).collect();
        assert!(names.contains(&"askUser".to_string()));
        assert!(!names.contains(&"presentInteractive".to_string()));

        let refused = server
            .call_tool(
                "askUser",
                json!({ "questions": [{ "id": "a", "question": "Q?", "inputType": "password" }] }),
                Some("session-1".to_string()),
            )
            .await
            .expect("tool returns a result");
        assert!(result_text(&refused).contains("requireUserInput=true"));

        let no_session = server
            .call_tool(
                "askUser",
                json!({ "questions": [{ "id": "a", "question": "Q?" }] }),
                None,
            )
            .await
            .expect("tool returns a result");
        assert_eq!(no_session.is_error, Some(true));

        assert!(server
            .call_tool("presentInteractive", json!({}), None)
            .await
            .is_err());
    }

    #[test]
    fn cancel_is_informational_and_timeout_is_an_error() {
        let questions = three_questions();

        for outcome in [UserPromptOutcome::Cancelled, UserPromptOutcome::Stopped] {
            let result = ask_user_outcome_result(&questions, &outcome);
            assert_eq!(result.is_error, Some(false));
            assert!(result_text(&result).contains(USER_PROMPT_CANCELLED_MESSAGE));
        }

        let timeout = ask_user_outcome_result(&questions, &UserPromptOutcome::TimedOut);
        assert_eq!(timeout.is_error, Some(true));
        assert!(result_text(&timeout).contains(USER_PROMPT_TIMEOUT_MESSAGE));

        let restart = ask_user_outcome_result(&questions, &UserPromptOutcome::Restarted);
        assert_eq!(restart.is_error, Some(true));
        assert!(result_text(&restart).contains(USER_PROMPT_RESTART_MESSAGE));
    }
}
