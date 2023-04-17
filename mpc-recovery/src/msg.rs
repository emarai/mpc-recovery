use crate::NodeId;
use serde::{Deserialize, Serialize};
use threshold_crypto::{Signature, SignatureShare};

#[derive(Serialize, Deserialize)]
pub struct NewAccountRequest {
    pub account_id: String,
    pub id_token: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum NewAccountResponse {
    Ok,
    Err { msg: String },
}

#[derive(Serialize, Deserialize)]
pub struct AddKeyRequest {
    pub account_id: String,
    pub public_key: String,
    pub id_token: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum AddKeyResponse {
    Ok,
    Err { msg: String },
}

#[derive(Serialize, Deserialize)]
pub struct LeaderRequest {
    pub payload: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum LeaderResponse {
    Ok {
        #[serde(with = "hex_sig_share")]
        signature: Signature,
    },
    Err,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SigShareRequest {
    pub payload: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum SigShareResponse {
    Ok {
        node_id: NodeId,
        sig_share: SignatureShare,
    },
    Err,
}

mod hex_sig_share {
    use serde::{Deserialize, Deserializer, Serializer};
    use threshold_crypto::Signature;

    pub fn serialize<S>(sig_share: &Signature, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = hex::encode(sig_share.to_bytes());
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Signature, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Signature::from_bytes(
            <[u8; 96]>::try_from(hex::decode(s).map_err(serde::de::Error::custom)?).map_err(
                |v: Vec<u8>| {
                    serde::de::Error::custom(format!(
                        "signature has incorrect length: expected 96 bytes, but got {}",
                        v.len()
                    ))
                },
            )?,
        )
        .map_err(serde::de::Error::custom)
    }
}