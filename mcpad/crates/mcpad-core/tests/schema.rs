//! Согласованность между снимком и опубликованной JSON-схемой.

use mcpad_core::{ProcessInfo, ProcessKey, Snapshot, SystemInfo};

const SCHEMA: &str = include_str!("../schema/snapshot.schema.json");

fn sample_snapshot() -> serde_json::Value {
    let mut process = ProcessInfo::new(ProcessKey::from_pid(42), "example");
    process.parent_pid = Some(1);
    process.exe_path = Some("/usr/bin/example".to_string());

    let snapshot = Snapshot::new(SystemInfo::default(), vec![process]);
    serde_json::to_value(snapshot).expect("снимок сериализуется")
}

#[test]
fn schema_is_valid_json_with_required_fields() {
    let schema: serde_json::Value = serde_json::from_str(SCHEMA).expect("схема — валидный JSON");
    assert_eq!(schema["type"], "object");
    for field in ["schema_version", "taken_at_unix_ms", "system", "processes"] {
        assert!(
            schema["required"]
                .as_array()
                .expect("required — массив")
                .iter()
                .any(|v| v == field),
            "поле {field} не объявлено обязательным"
        );
    }
}

#[test]
fn every_snapshot_field_is_described_by_schema() {
    let schema: serde_json::Value = serde_json::from_str(SCHEMA).expect("схема — валидный JSON");
    let snapshot = sample_snapshot();
    let properties = schema["properties"]
        .as_object()
        .expect("properties — объект");

    for (field, _) in snapshot.as_object().expect("снимок — объект") {
        assert!(
            properties.contains_key(field),
            "поле {field} есть в снимке, но отсутствует в схеме"
        );
    }
}

#[test]
fn unavailable_counter_serializes_as_null() {
    let snapshot = sample_snapshot();
    let process = &snapshot["processes"][0];
    assert!(process["cpu_percent"]["value"].is_null());
    assert_eq!(process["cpu_percent"]["unavailable_reason"], "not_measured");
}
