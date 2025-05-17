use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageData {
    pub name: String,
    pub url: String,
    pub data: String, // base64 encoded image data
}

// [{
//     "id": "D202501",
//     "data": [{ID: '202001', 证书编号: 'D202501', 姓名: '张三', 身份证号: '111111', 专业: '软件工程', …},...]
// }]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RecordData {
    pub id: String,
    pub data: Vec<BTreeMap<String, Value>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GenerateAuthenticationData {
    pub id: String,
    pub selected_fields: Vec<Vec<String>>,
    pub tx_hash: String,
    pub cid: Vec<String>,
    pub proof: Vec<String>,
    pub is_zk: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthenticationData {
    pub id: Vec<String>,
    pub data_cid: Vec<String>,
    pub tx_hash: Vec<String>,
    pub proof: String,
    pub zk_proof: String,
    pub random: String,
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub success: bool,
    pub certificate_file: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Certificate {
    pub id: String,
    pub original_data_cid: String,
    pub proof: String,
    pub tx_hash: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyResponse {
    pub verified: bool,
    pub message: String,
    pub data: Option<VerifiedData>,
}

#[derive(Debug, Serialize)]
pub struct VerifiedData {
    pub original_data: BTreeMap<String, Value>,
    pub tx_hash: String,
    pub images: Vec<VerifiedImage>,
    pub cid: String,
    pub proof: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthVerifyData {
    pub data: Vec<Vec<BTreeMap<String, Value>>>,
    pub images: Vec<Vec<Vec<VerifiedImage>>>,
    pub tx_hashs: Vec<String>,
    pub proof: String,
    pub zk_proof: String,
    pub random: String,
}

#[derive(Debug, Serialize)]
pub struct AuthVerifyResultData {
    pub verified: bool,
    pub tx_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifiedImage {
    pub ipfs_cid: String,
    pub data: String, // base64 encoded image data
}
