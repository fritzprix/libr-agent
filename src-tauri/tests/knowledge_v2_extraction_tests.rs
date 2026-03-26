use tauri_mcp_agent_lib::mcp::builtin::knowledge::extraction::{
    extract_graph_from_content, merge_plans, normalize_graph_plan, ExtractedEntity,
    ExtractedRelationship,
};
use tauri_mcp_agent_lib::mcp::builtin::knowledge::tools::record_knowledge_tool;
use tauri_mcp_agent_lib::mcp::schema::JSONSchemaType;

#[test]
fn extraction_finds_entities_and_relationships_from_content() {
    let plan = extract_graph_from_content(
        "LibrAgent uses sqlite-vec and fastembed for local memory.",
        &["knowledge".to_string()],
    );

    let entity_names = plan
        .entities
        .iter()
        .map(|entity| entity.name.as_str())
        .collect::<Vec<_>>();
    assert!(entity_names.contains(&"LibrAgent"));
    assert!(entity_names.contains(&"sqlite-vec"));
    assert!(entity_names.contains(&"fastembed"));
    assert!(entity_names.contains(&"knowledge"));

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
    let fallback = extract_graph_from_content("LibrAgent uses sqlite-vec and fastembed.", &[]);

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
}
