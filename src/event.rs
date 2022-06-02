use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct AccessEvent {
    pub time: u128,
    pub key: String,
    pub value: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
    pub reason: String,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Variation {
    pub key: String,
    pub index: Option<usize>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    pub count: u128,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct PackedData {
    #[serde(default)]
    pub events: Vec<AccessEvent>,
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
