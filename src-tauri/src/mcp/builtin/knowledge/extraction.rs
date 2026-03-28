use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedEntity {
    pub name: String,
    pub entity_type: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedRelationship {
    pub source: String,
    pub target: String,
    pub relation_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExtractionPlan {
    pub entities: Vec<ExtractedEntity>,
    pub relationships: Vec<ExtractedRelationship>,
}

const RELATION_RULES: [(&str, &str); 11] = [
    ("integrates with", "INTEGRATES_WITH"),
    ("depends on", "DEPENDS_ON"),
    ("built with", "USES"),
    ("built on", "USES"),
    ("powered by", "POWERED_BY"),
    ("runs on", "RUNS_ON"),
    ("links to", "LINKS_TO"),
    ("connects to", "CONNECTS_TO"),
    ("supports", "SUPPORTS"),
    ("includes", "INCLUDES"),
    ("uses", "USES"),
];

const STOPWORDS: &[&str] = &[
    "a", "an", "and", "as", "at", "by", "for", "from", "in", "into", "memory", "of", "on", "or",
    "the", "this", "that", "these", "those", "to", "use", "uses", "using", "with",
];
const MAX_ENTITY_NAME_WORDS: usize = 10;

pub fn extract_graph_from_content(content: &str, tags: &[String]) -> ExtractionPlan {
    let mut entities = BTreeMap::<String, ExtractedEntity>::new();
    let mut relationships = BTreeMap::<(String, String, String), ExtractedRelationship>::new();

    for tag in tags {
        if let Some(name) = normalize_entity_name(tag, true) {
            insert_entity(
                &mut entities,
                ExtractedEntity {
                    name,
                    entity_type: Some("Tag".to_string()),
                    description: None,
                },
            );
        }
    }

    for candidate in extract_candidate_entities(content) {
        insert_entity(
            &mut entities,
            ExtractedEntity {
                entity_type: Some(infer_entity_type(&candidate, false).to_string()),
                name: candidate,
                description: None,
            },
        );
    }

    for sentence in content.split(['.', '!', '?', '\n']) {
        if let Some((source, relation_type, targets)) = extract_sentence_relationship(sentence) {
            insert_entity(
                &mut entities,
                ExtractedEntity {
                    entity_type: Some(infer_entity_type(&source, false).to_string()),
                    name: source.clone(),
                    description: None,
                },
            );

            for target in targets {
                insert_entity(
                    &mut entities,
                    ExtractedEntity {
                        entity_type: Some(infer_entity_type(&target, false).to_string()),
                        name: target.clone(),
                        description: None,
                    },
                );

                let key = (
                    source.to_ascii_lowercase(),
                    target.to_ascii_lowercase(),
                    relation_type.to_string(),
                );
                relationships
                    .entry(key)
                    .or_insert_with(|| ExtractedRelationship {
                        source: source.clone(),
                        target,
                        relation_type: relation_type.to_string(),
                    });
            }
        }
    }

    ExtractionPlan {
        entities: entities.into_values().collect(),
        relationships: relationships.into_values().collect(),
    }
}

pub fn normalize_graph_plan(
    entities: Vec<ExtractedEntity>,
    relationships: Vec<ExtractedRelationship>,
) -> Result<ExtractionPlan, String> {
    let mut normalized_entities = BTreeMap::<String, ExtractedEntity>::new();
    let mut normalized_relationships =
        BTreeMap::<(String, String, String), ExtractedRelationship>::new();

    for entity in entities {
        let name = normalize_entity_name_with_reason(&entity.name, true)
            .map_err(|reason| format!("Invalid entity name '{}': {}", entity.name, reason))?;
        insert_entity(
            &mut normalized_entities,
            ExtractedEntity {
                name,
                entity_type: entity.entity_type.filter(|value| !value.trim().is_empty()),
                description: entity.description.filter(|value| !value.trim().is_empty()),
            },
        );
    }

    for relationship in relationships {
        let source =
            normalize_entity_name_with_reason(&relationship.source, true).map_err(|reason| {
                format!(
                    "Invalid relationship source '{}': {}",
                    relationship.source, reason
                )
            })?;
        let target =
            normalize_entity_name_with_reason(&relationship.target, true).map_err(|reason| {
                format!(
                    "Invalid relationship target '{}': {}",
                    relationship.target, reason
                )
            })?;
        let relation_type = normalize_relation_type(&relationship.relation_type)
            .ok_or_else(|| format!("Invalid relation_type '{}'", relationship.relation_type))?;

        insert_entity(
            &mut normalized_entities,
            ExtractedEntity {
                name: source.clone(),
                entity_type: Some(infer_entity_type(&source, false).to_string()),
                description: None,
            },
        );
        insert_entity(
            &mut normalized_entities,
            ExtractedEntity {
                name: target.clone(),
                entity_type: Some(infer_entity_type(&target, false).to_string()),
                description: None,
            },
        );

        normalized_relationships
            .entry((
                source.to_ascii_lowercase(),
                target.to_ascii_lowercase(),
                relation_type.clone(),
            ))
            .or_insert(ExtractedRelationship {
                source,
                target,
                relation_type,
            });
    }

    Ok(ExtractionPlan {
        entities: normalized_entities.into_values().collect(),
        relationships: normalized_relationships.into_values().collect(),
    })
}

pub fn merge_plans(primary: &ExtractionPlan, fallback: &ExtractionPlan) -> ExtractionPlan {
    let mut merged_entities = BTreeMap::<String, ExtractedEntity>::new();
    let mut merged_relationships =
        BTreeMap::<(String, String, String), ExtractedRelationship>::new();

    for entity in &primary.entities {
        insert_entity(&mut merged_entities, entity.clone());
    }
    for relationship in &primary.relationships {
        merged_relationships.insert(
            (
                relationship.source.to_ascii_lowercase(),
                relationship.target.to_ascii_lowercase(),
                relationship.relation_type.clone(),
            ),
            relationship.clone(),
        );
    }

    for entity in &fallback.entities {
        insert_entity(&mut merged_entities, entity.clone());
    }
    for relationship in &fallback.relationships {
        merged_relationships
            .entry((
                relationship.source.to_ascii_lowercase(),
                relationship.target.to_ascii_lowercase(),
                relationship.relation_type.clone(),
            ))
            .or_insert_with(|| relationship.clone());
    }

    ExtractionPlan {
        entities: merged_entities.into_values().collect(),
        relationships: merged_relationships.into_values().collect(),
    }
}

fn insert_entity(store: &mut BTreeMap<String, ExtractedEntity>, entity: ExtractedEntity) {
    let key = entity.name.to_ascii_lowercase();
    store
        .entry(key)
        .and_modify(|existing| {
            if existing.entity_type.as_deref() != Some("Tag") {
                existing.entity_type = entity.entity_type.clone().or(existing.entity_type.clone());
            }
            if existing.description.is_none() {
                existing.description = entity.description.clone();
            }
        })
        .or_insert(entity);
}

fn extract_candidate_entities(content: &str) -> Vec<String> {
    let mut candidates = BTreeMap::<String, String>::new();

    for capture in backtick_regex().captures_iter(content) {
        if let Some(name) = normalize_entity_name(&capture[1], true) {
            candidates.insert(name.to_ascii_lowercase(), name);
        }
    }

    for matched in proper_noun_regex().find_iter(content) {
        if let Some(name) = normalize_entity_name(matched.as_str(), false) {
            candidates.insert(name.to_ascii_lowercase(), name);
        }
    }

    for matched in hyphenated_regex().find_iter(content) {
        if let Some(name) = normalize_entity_name(matched.as_str(), true) {
            candidates.insert(name.to_ascii_lowercase(), name);
        }
    }

    candidates.into_values().collect()
}

fn extract_sentence_relationship(sentence: &str) -> Option<(String, &'static str, Vec<String>)> {
    let trimmed = sentence.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lowercase = trimmed.to_ascii_lowercase();
    for (needle, relation_type) in RELATION_RULES {
        if let Some(index) = lowercase.find(needle) {
            let source = pick_source_entity(&trimmed[..index])?;
            let targets = pick_target_entities(&trimmed[index + needle.len()..]);
            if !targets.is_empty() {
                return Some((source, relation_type, targets));
            }
        }
    }

    None
}

fn pick_source_entity(source_phrase: &str) -> Option<String> {
    let mut candidates = extract_candidate_entities(source_phrase);
    if candidates.is_empty() {
        candidates = split_phrase_candidates(source_phrase, false);
    }
    candidates.pop()
}

fn pick_target_entities(target_phrase: &str) -> Vec<String> {
    let trimmed = truncate_at_qualifier(target_phrase.trim());
    let mut candidates = BTreeMap::<String, String>::new();

    for segment in target_split_regex().split(trimmed) {
        for candidate in extract_candidate_entities(segment) {
            candidates.insert(candidate.to_ascii_lowercase(), candidate);
        }

        if let Some(candidate) = normalize_entity_name(segment, true) {
            candidates.insert(candidate.to_ascii_lowercase(), candidate);
        }
    }

    candidates.into_values().collect()
}

fn split_phrase_candidates(phrase: &str, allow_simple_lowercase: bool) -> Vec<String> {
    phrase
        .split(',')
        .filter_map(|part| normalize_entity_name(part, allow_simple_lowercase))
        .collect()
}

fn truncate_at_qualifier(input: &str) -> &str {
    let lowercase = input.to_ascii_lowercase();
    let qualifiers = [
        " for ",
        " because ",
        " where ",
        " which ",
        " while ",
        " via ",
    ];

    let end = qualifiers
        .iter()
        .filter_map(|qualifier| lowercase.find(qualifier))
        .min()
        .unwrap_or(input.len());

    &input[..end]
}

fn normalize_entity_name(raw: &str, allow_simple_lowercase: bool) -> Option<String> {
    normalize_entity_name_with_reason(raw, allow_simple_lowercase).ok()
}

fn normalize_entity_name_with_reason(
    raw: &str,
    allow_simple_lowercase: bool,
) -> Result<String, String> {
    let collapsed = raw
        .trim()
        .trim_matches(|character: char| {
            matches!(
                character,
                '`' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '.' | ',' | ';' | ':'
            )
        })
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let collapsed = strip_leading_noise(&collapsed);

    if collapsed.len() < 2 {
        return Err("entity names must be at least 2 characters long".to_string());
    }

    let word_count = collapsed.split_whitespace().count();
    if word_count > MAX_ENTITY_NAME_WORDS {
        return Err(format!(
            "entity names must be {} words or fewer (received {} words)",
            MAX_ENTITY_NAME_WORDS, word_count
        ));
    }

    let lowercase = collapsed.to_ascii_lowercase();
    if STOPWORDS.contains(&lowercase.as_str()) {
        return Err("entity names cannot be a single stopword".to_string());
    }

    let has_uppercase = collapsed.chars().any(|character| character.is_uppercase());
    let has_hyphen = collapsed.contains('-') || collapsed.contains('_');
    let has_digit = collapsed
        .chars()
        .any(|character| character.is_ascii_digit());

    if !allow_simple_lowercase && !has_uppercase && !has_hyphen && !has_digit {
        return Err(
            "entity names should include a proper noun, acronym, digit, or hyphenated term"
                .to_string(),
        );
    }

    Ok(collapsed)
}

fn strip_leading_noise(input: &str) -> String {
    let lowercase = input.to_ascii_lowercase();
    for prefix in ["the ", "a ", "an ", "this ", "that ", "these ", "those "] {
        if lowercase.starts_with(prefix) {
            return input[prefix.len()..].trim().to_string();
        }
    }

    input.trim().to_string()
}

fn infer_entity_type(name: &str, from_tag: bool) -> &'static str {
    if from_tag {
        return "Tag";
    }

    let lowercase = name.to_ascii_lowercase();
    if lowercase.contains("agent")
        || lowercase.contains("server")
        || lowercase.contains("app")
        || lowercase.contains("platform")
        || lowercase.contains("service")
        || lowercase.contains("project")
    {
        return "Project";
    }

    if lowercase.contains("sqlite")
        || lowercase.contains("rust")
        || lowercase.contains("embed")
        || lowercase.contains("directml")
        || lowercase.contains("onnx")
        || lowercase.contains("tauri")
        || name.contains('-')
        || name
            .chars()
            .all(|character| !character.is_alphabetic() || character.is_uppercase())
    {
        return "Technology";
    }

    "Concept"
}

fn normalize_relation_type(raw: &str) -> Option<String> {
    let normalized = raw
        .trim()
        .replace([' ', '-'], "_")
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '_')
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    if normalized.is_empty() {
        return None;
    }

    Some(normalized.to_ascii_uppercase())
}

fn proper_noun_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"\b(?:[A-Z][A-Za-z0-9]+(?:[-_][A-Za-z0-9]+)*)(?:\s+(?:[A-Z][A-Za-z0-9]+(?:[-_][A-Za-z0-9]+)*))*\b",
        )
        .expect("proper noun regex should compile")
    })
}

fn hyphenated_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"\b[a-z0-9]+(?:[-_][a-z0-9]+)+\b").expect("hyphenated regex should compile")
    })
}

fn backtick_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"`([^`]+)`").expect("backtick regex should compile"))
}

fn target_split_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"\s*(?:,|/|\band\b|\bor\b)\s*").expect("target split regex should compile")
    })
}
