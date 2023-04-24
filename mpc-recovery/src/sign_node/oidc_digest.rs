use crate::gcp::{
    error::ConvertError,
    value::{FromValue, IntoValue, Value},
    KeyKind,
};

use google_datastore1::api::{Key, PathElement};
use near_crypto::PublicKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OidcDigest {
    // Not sure if this is strictly neccessarry
    pub node_id: usize,
    pub digest: [u8; 32],
    pub public_key: PublicKey,
}

impl KeyKind for OidcDigest {
    fn kind() -> String {
        "OidcDigest".to_string()
    }
}

impl IntoValue for OidcDigest {
    fn into_value(self) -> Value {
        let mut properties = HashMap::new();
        properties.insert(
            "node_id".to_string(),
            Value::IntegerValue(self.node_id as i64),
        );
        properties.insert("digest".to_string(), Value::BlobValue(self.digest.into()));
        properties.insert(
            "public_key".to_string(),
            Value::StringValue(serde_json::to_string(&self.public_key).unwrap()),
        );

        Value::EntityValue {
            key: Key {
                path: Some(vec![PathElement {
                    kind: Some(Self::kind()),
                    name: Some(self.to_name()),
                    id: None,
                }]),
                partition_id: None,
            },
            properties,
        }
    }
}

impl FromValue for OidcDigest {
    fn from_value(value: Value) -> Result<Self, ConvertError> {
        match value.clone() {
            Value::EntityValue { mut properties, .. } => {
                let (_, node_id) = properties
                    .remove_entry("node_id")
                    .ok_or_else(|| ConvertError::MissingProperty("node_id".to_string()))?;
                let node_id = i64::from_value(node_id)? as usize;
                let (_, digest) = properties
                    .remove_entry("digest")
                    .ok_or_else(|| ConvertError::MissingProperty("digest".to_string()))?;
                let digest = hex::decode(String::from_value(digest)?)
                    .map_err(|_| ConvertError::MalformedProperty("digest".to_string()))?;

                let digest = <[u8; 32]>::try_from(digest)
                    .map_err(|_| ConvertError::MalformedProperty("digest".to_string()))?;

                let (_, public_key) = properties
                    .remove_entry("public_key")
                    .ok_or_else(|| ConvertError::MissingProperty("public_key".to_string()))?;
                let public_key = String::from_value(public_key)?;
                let public_key = serde_json::from_str(&public_key)
                    .map_err(|_| ConvertError::MalformedProperty("public_key".to_string()))?;

                Ok(Self {
                    node_id,
                    digest,
                    public_key,
                })
            }
            error => Err(ConvertError::UnexpectedPropertyType {
                expected: "entity".to_string(),
                got: format!("{:?}", value),
            }),
        }
    }
}

impl OidcDigest {
    pub fn to_name(&self) -> String {
        format!("{}/{}", self.node_id, hex::encode(self.digest))
    }
}

impl Eq for OidcDigest {}