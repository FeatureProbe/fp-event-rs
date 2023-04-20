use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Serialize, Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Event {
    AccessEvent(AccessEvent),
    CustomEvent(CustomEvent),
    DebugEvent(DebugEvent),
}

#[derive(Serialize, Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccessEvent {
    pub kind: String,
    pub time: u128,
    pub key: String,
    pub user: String,
    pub value: Value,
    pub variation_index: usize,
    pub version: Option<u64>,
    pub rule_index: Option<usize>,
    #[serde(skip)]
    pub track_access_events: bool,
}

#[derive(Serialize, Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CustomEvent {
    pub kind: String,
    pub time: u128,
    pub user: String,
    pub name: String,
    pub value: Option<f64>,
}

#[derive(Serialize, Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DebugEvent {
    pub kind: String,
    pub time: u128,
    pub key: String,
    pub user: String,
    pub user_detail: Value,
    pub value: Value,
    pub variation_index: usize,
    pub version: Option<u64>,
    pub rule_index: Option<usize>,
    pub reason: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Variation {
    pub key: String,
    pub index: usize,
    pub version: Option<u64>,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct CountValue {
    pub count: u128,
    pub value: Value,
}

#[derive(Serialize, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Access {
    pub start_time: u128,
    pub end_time: u128,
    pub counters: HashMap<String, Vec<ToggleCounter>>,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct ToggleCounter {
    pub value: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
    pub index: usize,
    pub count: u128,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct PackedData {
    #[serde(default)]
    pub events: Vec<Event>,
    pub access: Access,
}

#[cfg(test)]
mod tests {
    use super::PackedData;

    #[test]
    fn test_packed_data_without_events() {
        let s = r#"
        {
            "access": {
                "startTime": 1,
                "endTime": 1,
                "counters": {}
            }
        }
        "#;

        let p = serde_json::from_str::<PackedData>(s);
        assert!(p.is_ok());
        let p = p.unwrap();
        assert!(p.events.is_empty());
    }
}
