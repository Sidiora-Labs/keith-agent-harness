//! Input shapes for existing memory and target tools; checked domain APIs validate use.

use serde_json::{Value, json};

fn object(properties: Value, required: &[&str]) -> Value {
    let mut value = json!({"type":"object", "required":required,"additionalProperties":false});
    value["properties"] = properties;
    value
}

pub(super) fn key_schema() -> Value {
    object(
        json!({"entity_id":{"type":"string"},"property":{"type":"string","maxLength":128}}),
        &["entity_id", "property"],
    )
}

fn span_schema() -> Value {
    object(
        json!({"start":{"type":"integer","minimum":0},"end":{"type":"integer","minimum":0}}),
        &["start", "end"],
    )
}

fn effective_schema() -> Value {
    object(
        json!({"from":{"type":["integer","null"]},"until":{"type":["integer","null"]}}),
        &["from", "until"],
    )
}

pub(super) fn draft_schema() -> Value {
    object(
        json!({
            "entity":{"oneOf":[
                object(json!({"mode":{"type":"string","enum":["existing"]},"entity_id":{"type":"string"}}), &["mode","entity_id"]),
                object(json!({"mode":{"type":"string","enum":["new_alias"]},"alias":{"type":"string","maxLength":256}}), &["mode","alias"])
            ]},
            "property":{"type":"string","maxLength":128},
            "target_kind":{"type":"string","enum":["workspace_path","http_url","literal"]},
            "value_quote":{"type":"string","maxLength":16384},
            "value_span":span_schema(),"effective":effective_schema()
        }),
        &["entity", "property", "target_kind", "value_quote"],
    )
}

pub(super) fn correction_schema() -> Value {
    object(
        json!({"value_quote":{"type":"string","maxLength":16384},
        "value_span":span_schema(),"effective":effective_schema()}),
        &["value_quote"],
    )
}

pub(super) fn reference_schema() -> Value {
    object(
        json!({"key":key_schema(),"binding_id":{"type":"string"},
        "revision":{"type":"integer","minimum":1},"evidence_id":{"type":"string","description":"Original quoted source evidence ID from the exact binding reference. Preserve it unchanged; this is not the saved-memory owner ID to replace."},
        "evidence_digest":{"type":"string","pattern":"^[0-9a-f]{64}$"},
        "value_digest":{"type":"string","pattern":"^[0-9a-f]{64}$"}}),
        &[
            "key",
            "binding_id",
            "revision",
            "evidence_id",
            "evidence_digest",
            "value_digest",
        ],
    )
}

pub(super) fn requirements_schema() -> Value {
    let target = object(
        json!({"kind":{"type":"string","enum":["workspace_path","http_url"]},
        "tool_name":{"type":"string","enum":["read","web_fetch"]},
        "argument_name":{"type":"string","enum":["path","url"]}}),
        &["kind", "tool_name", "argument_name"],
    );
    json!({"type":"array","maxItems":128,"minItems":1,
        "items":object(json!({"key":key_schema(),"target":target}), &["key","target"])})
}
