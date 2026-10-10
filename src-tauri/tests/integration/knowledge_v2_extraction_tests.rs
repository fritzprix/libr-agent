use serde_json::json;
use tauri_mcp_agent_lib::mcp::builtin::knowledge::extraction::{
    extract_graph_from_content, merge_plans, normalize_graph_plan, ExtractedEntity,
    ExtractedRelationship,
};
use tauri_mcp_agent_lib::mcp::builtin::knowledge::tools::record_knowledge_tool;
use tauri_mcp_agent_lib::mcp::schema::JSONSchemaType;

#[test]
fn extraction_finds_entities_and_relationships_from_content() {
    let plan =
        extract_graph_from_content("LibrAgent uses sqlite-vec and fastembed for local memory.");

    let entity_names = plan
        .entities
        .iter()
        .map(|entity| entity.name.as_str())
        .collect::<Vec<_>>();
    assert!(entity_names.contains(&"LibrAgent"));
    assert!(entity_names.contains(&"sqlite-vec"));
    assert!(entity_names.contains(&"fastembed"));

    assert!(plan.relationships.iter().any(|relationship| {
        relationship.source == "LibrAgent"
            && relationship.target == "sqlite-vec"
            && relationship.relation_type == "USES"
    }));
    assert!(plan.relationships.iter().any(|relationship| {
        relationship.source == "LibrAgent"
            && relationship.target == "fastembed"
            && relationship.relation_type == "USES"
    }));
}

#[test]
fn extraction_does_not_promote_tags_or_markdown_headings() {
    let plan = extract_graph_from_content(
        "# Key Facts\n\nLibrAgent uses SeaORM for persistence.\n\n## Appendix\n\nMore notes.",
    );

    let entity_names = plan
        .entities
        .iter()
        .map(|entity| entity.name.as_str())
        .collect::<Vec<_>>();
    assert!(entity_names.contains(&"LibrAgent"));
    assert!(entity_names.contains(&"SeaORM"));
    assert!(!entity_names.contains(&"Key Facts"));
    assert!(!entity_names.contains(&"Appendix"));
    assert!(!entity_names.iter().any(|name| *name == "knowledge"));
}

#[test]
fn extraction_keeps_hash_lines_that_are_not_markdown_headings() {
    let plan = extract_graph_from_content(
        "LibrAgent uses SeaORM.\n\n```c\n#include <stdio.h>\n# comment\n```\n",
    );

    let entity_names = plan
        .entities
        .iter()
        .map(|entity| entity.name.as_str())
        .collect::<Vec<_>>();
    assert!(entity_names.contains(&"LibrAgent"));
    assert!(entity_names.contains(&"SeaORM"));
    // Non-heading `#` lines must not drop surrounding content / relationships.
    assert!(plan.relationships.iter().any(|relationship| {
        relationship.source == "LibrAgent"
            && relationship.target == "SeaORM"
            && relationship.relation_type == "USES"
    }));
}

#[test]
fn normalize_rejects_iso_dates_as_entity_names() {
    let error = normalize_graph_plan(
        vec![ExtractedEntity {
            name: "2026-09-30".to_string(),
            entity_type: Some("Tag".to_string()),
            description: None,
        }],
        vec![],
    )
    .expect_err("ISO dates must not become entities");

    assert!(error.contains("ISO dates"));
}

#[test]
fn infer_entity_type_does_not_mark_short_acronyms_as_technology() {
    let plan = normalize_graph_plan(
        vec![
            ExtractedEntity {
                name: "ARR".to_string(),
                entity_type: None,
                description: None,
            },
            ExtractedEntity {
                name: "sqlite-vec".to_string(),
                entity_type: None,
                description: None,
            },
        ],
        vec![],
    )
    .expect("entities should normalize");

    // Short acronyms stay Concept when type is filled by relationship-side inference;
    // explicit entities with None type keep None through normalize_graph_plan.
    let arr = plan
        .entities
        .iter()
        .find(|entity| entity.name == "ARR")
        .expect("ARR present");
    assert!(arr.entity_type.is_none());

    let with_rel = normalize_graph_plan(
        vec![],
        vec![ExtractedRelationship {
            source: "ARR".to_string(),
            target: "sqlite-vec".to_string(),
            relation_type: "USES".to_string(),
        }],
    )
    .expect("relationship should normalize");

    let arr_inferred = with_rel
        .entities
        .iter()
        .find(|entity| entity.name == "ARR")
        .expect("ARR present");
    assert_eq!(arr_inferred.entity_type.as_deref(), Some("Concept"));

    let tech = with_rel
        .entities
        .iter()
        .find(|entity| entity.name == "sqlite-vec")
        .expect("sqlite-vec present");
    assert_eq!(tech.entity_type.as_deref(), Some("Technology"));
}

#[test]
fn serde_aliases_accept_legacy_type_from_to_fields() {
    let entity: ExtractedEntity = serde_json::from_value(json!({
        "name": "LibrAgent",
        "type": "Project",
        "description": "Desktop agent"
    }))
    .expect("entity alias should deserialize");
    assert_eq!(entity.entity_type.as_deref(), Some("Project"));

    let relationship: ExtractedRelationship = serde_json::from_value(json!({
        "from": "LibrAgent",
        "to": "SeaORM",
        "type": "USES"
    }))
    .expect("relationship aliases should deserialize");
    assert_eq!(relationship.source, "LibrAgent");
    assert_eq!(relationship.target, "SeaORM");
    assert_eq!(relationship.relation_type, "USES");
}

#[test]
fn normalize_graph_plan_adds_implicit_entities_and_normalizes_relationships() {
    let plan = normalize_graph_plan(
        vec![ExtractedEntity {
            name: "LibrAgent".to_string(),
            entity_type: Some("Project".to_string()),
            description: None,
        }],
        vec![ExtractedRelationship {
            source: "LibrAgent".to_string(),
            target: "sqlite-vec".to_string(),
            relation_type: "uses".to_string(),
        }],
    )
    .expect("graph payload should normalize");

    assert!(plan
        .entities
        .iter()
        .any(|entity| entity.name == "LibrAgent"));
    assert!(plan
        .entities
        .iter()
        .any(|entity| entity.name == "sqlite-vec"));
    assert!(plan.relationships.iter().any(|relationship| {
        relationship.source == "LibrAgent"
            && relationship.target == "sqlite-vec"
            && relationship.relation_type == "USES"
    }));
}

#[test]
fn normalize_graph_plan_accepts_longer_natural_language_entity_names() {
    let plan = normalize_graph_plan(
        vec![ExtractedEntity {
            name: "Sahm Rule vs Equity Market Divergence".to_string(),
            entity_type: Some("Market Trend".to_string()),
            description: None,
        }],
        vec![],
    )
    .expect("six-word natural-language entity name should normalize");

    assert!(plan
        .entities
        .iter()
        .any(|entity| { entity.name == "Sahm Rule vs Equity Market Divergence" }));
}

#[test]
fn normalize_graph_plan_rejects_overlong_entity_names_with_guidance() {
    let error = normalize_graph_plan(
        vec![ExtractedEntity {
            name: "One Two Three Four Five Six Seven Eight Nine Ten Eleven".to_string(),
            entity_type: Some("Concept".to_string()),
            description: None,
        }],
        vec![],
    )
    .expect_err("overlong entity name should fail validation");

    assert!(error.contains("Invalid entity name"));
    assert!(error.contains("10 words or fewer"));
    assert!(error.contains("received 11 words"));
}

#[test]
fn merge_plans_prefers_explicit_graph_and_fills_missing_heuristics() {
    let explicit = normalize_graph_plan(
        vec![ExtractedEntity {
            name: "LibrAgent".to_string(),
            entity_type: Some("Project".to_string()),
            description: Some("Agent platform".to_string()),
        }],
        vec![],
    )
    .expect("explicit plan should normalize");
    let fallback = extract_graph_from_content("LibrAgent uses sqlite-vec and fastembed.");

    let merged = merge_plans(&explicit, &fallback);

    assert!(merged.entities.iter().any(|entity| {
        entity.name == "LibrAgent" && entity.description.as_deref() == Some("Agent platform")
    }));
    assert!(merged
        .entities
        .iter()
        .any(|entity| entity.name == "sqlite-vec"));
    assert!(merged.relationships.iter().any(|relationship| {
        relationship.source == "LibrAgent" && relationship.target == "sqlite-vec"
    }));
}

#[test]
fn merge_plans_keeps_explicit_relationship_when_heuristic_disagrees() {
    let explicit = normalize_graph_plan(
        vec![],
        vec![ExtractedRelationship {
            source: "LibrAgent".to_string(),
            target: "sqlite-vec".to_string(),
            relation_type: "DEPENDS_ON".to_string(),
        }],
    )
    .expect("explicit plan should normalize");
    let heuristic = normalize_graph_plan(
        vec![],
        vec![ExtractedRelationship {
            source: "LibrAgent".to_string(),
            target: "sqlite-vec".to_string(),
            relation_type: "USES".to_string(),
        }],
    )
    .expect("heuristic plan should normalize");

    let merged = merge_plans(&explicit, &heuristic);

    assert!(merged.relationships.iter().any(|relationship| {
        relationship.source == "LibrAgent"
            && relationship.target == "sqlite-vec"
            && relationship.relation_type == "DEPENDS_ON"
    }));
}

#[test]
fn record_knowledge_tool_schema_exposes_structured_graph_inputs() {
    let tool = record_knowledge_tool();
    let JSONSchemaType::Object {
        properties,
        required,
        ..
    } = &tool.input_schema.schema_type
    else {
        panic!("record_knowledge input schema should be an object");
    };

    let properties = properties
        .as_ref()
        .expect("record_knowledge schema should expose properties");
    let required = required
        .as_ref()
        .expect("record_knowledge schema should declare required fields");

    assert!(required.iter().any(|field| field == "content"));
    assert!(properties.contains_key("entities"));
    assert!(properties.contains_key("relationships"));

    let auto_extract = properties
        .get("auto_extract")
        .expect("auto_extract schema should be present");
    assert_eq!(
        auto_extract.default.as_ref(),
        Some(&serde_json::Value::Bool(false)),
        "auto_extract must default to false"
    );

    let entities_schema = properties
        .get("entities")
        .expect("entities schema should be present");
    let JSONSchemaType::Array { items, .. } = &entities_schema.schema_type else {
        panic!("entities should be described as an array");
    };
    let entity_item = items
        .as_ref()
        .expect("entities array should describe item schema");
    let JSONSchemaType::Object {
        properties: entity_properties,
        required: entity_required,
        ..
    } = &entity_item.schema_type
    else {
        panic!("entity items should be described as objects");
    };
    let entity_properties = entity_properties
        .as_ref()
        .expect("entity items should expose properties");
    let entity_required = entity_required
        .as_ref()
        .expect("entity items should declare required fields");
    assert!(entity_properties.contains_key("name"));
    assert!(entity_properties.contains_key("entity_type"));
    assert!(entity_properties.contains_key("description"));
    assert!(entity_required.iter().any(|field| field == "name"));
    let entity_name_description = entity_properties
        .get("name")
        .and_then(|schema| schema.description.as_deref())
        .expect("entity name field should describe naming guidance");
    assert!(entity_name_description.contains("10 words or fewer"));

    let relationships_schema = properties
        .get("relationships")
        .expect("relationships schema should be present");
    let JSONSchemaType::Array { items, .. } = &relationships_schema.schema_type else {
        panic!("relationships should be described as an array");
    };
    let relationship_item = items
        .as_ref()
        .expect("relationships array should describe item schema");
    let JSONSchemaType::Object {
        properties: relationship_properties,
        required: relationship_required,
        ..
    } = &relationship_item.schema_type
    else {
        panic!("relationship items should be described as objects");
    };
    let relationship_properties = relationship_properties
        .as_ref()
        .expect("relationship items should expose properties");
    let relationship_required = relationship_required
        .as_ref()
        .expect("relationship items should declare required fields");
    assert!(relationship_properties.contains_key("source"));
    assert!(relationship_properties.contains_key("target"));
    assert!(relationship_properties.contains_key("relation_type"));
    assert!(relationship_required.iter().any(|field| field == "source"));
    assert!(relationship_required.iter().any(|field| field == "target"));
    assert!(relationship_required
        .iter()
        .any(|field| field == "relation_type"));
    let relationship_source_description = relationship_properties
        .get("source")
        .and_then(|schema| schema.description.as_deref())
        .expect("relationship source field should describe naming guidance");
    assert!(relationship_source_description.contains("Match entities[].name"));
}
