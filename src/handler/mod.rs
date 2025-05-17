use std::collections::BTreeMap;

use actix_web::{error, web, Result};
use ipfs_api_backend_hyper::{IpfsClient, TryFromUri};
use log::{debug, error, info, warn};
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha20Rng;
use serde_json::Value;
use snark_verifier_sdk::snark_verifier::halo2_base::halo2_proofs::halo2curves::bn256::Fr;
use snark_verifier_sdk::snark_verifier::halo2_base::utils::ScalarField;
use snark_verifier_sdk::snark_verifier::util::arithmetic::Field;
use web3::contract::Contract;
use web3::{transports::Http, Web3};

use crate::models::{RecordData, VerifiedImage, VerifyResponse};
use crate::services::compress_fr;
use crate::services::ethereum::get_transaction_data;
use crate::services::{
    ethereum::{CONTRACT_ABI, CONTRACT_ADDRESS},
    ipfs::get_image_from_ipfs,
};

pub mod company;
pub mod school;
pub mod student;

pub async fn verify_hash(
    certificate_data: web::Json<serde_json::Value>,
    web3: web::Data<Web3<Http>>,
) -> Result<web::Json<VerifyResponse>> {
    info!("收到验证请求，开始处理...");

    // 获取合约地址
    let contract_address = CONTRACT_ADDRESS.get().ok_or_else(|| {
        error!("合约未部署");
        error::ErrorInternalServerError("Contract not deployed")
    })?;

    // 创建合约实例
    let contract = Contract::from_json(
        web3.eth(),
        contract_address
            .parse()
            .map_err(error::ErrorInternalServerError)?,
        CONTRACT_ABI.as_bytes(),
    )
    .map_err(error::ErrorInternalServerError)?;

    // 从请求中提取数据
    let tx_hash = certificate_data["tx_hash"]
        .as_str()
        .ok_or_else(|| error::ErrorBadRequest("Missing tx_hash field"))?;

    let original_data = serde_json::to_string(&certificate_data["original_data"])
        .map_err(error::ErrorInternalServerError)?;
    let original_data_bytes = web3::types::Bytes::from(original_data.as_bytes());

    // 从tx_hash获取区块链上的数据
    let blockchain_data = get_transaction_data(&web3, tx_hash)
        .await
        .map_err(error::ErrorInternalServerError)?;

    // 获取以太坊账户
    let accounts = web3
        .eth()
        .accounts()
        .await
        .map_err(error::ErrorInternalServerError)?;

    let account = accounts
        .first()
        .cloned()
        .ok_or_else(|| error::ErrorInternalServerError("No Ethereum account available"))?;

    // 调用智能合约验证
    let verified: bool = contract
        .query(
            "verifyData",
            (original_data_bytes.clone(), blockchain_data.clone()),
            account,
            Default::default(),
            None,
        )
        .await
        .map_err(error::ErrorInternalServerError)?;

    let (message, data) = if verified {
        // 如果验证成功，处理图片数据
        let verified_data = None;

        // 创建IPFS客户端
        let ipfs_url =
            std::env::var("IPFS_API_URL").unwrap_or_else(|_| "http://localhost:5001".to_string());
        if let Ok(ipfs_client) = IpfsClient::from_str(&ipfs_url) {
            if let Some(original_data_obj) = certificate_data["original_data"].as_object() {
                let mut images = Vec::new();

                // 处理图片数据
                if let Some(images_array) =
                    original_data_obj.get("images").and_then(|v| v.as_array())
                {
                    for image in images_array {
                        if let (Some(name), Some(ipfs_cid), Some(size)) = (
                            image.get("name").and_then(|v| v.as_str()),
                            image.get("ipfs_cid").and_then(|v| v.as_str()),
                            image.get("size").and_then(|v| v.as_u64()),
                        ) {
                            match get_image_from_ipfs(&ipfs_client, ipfs_cid).await {
                                Ok(image_data) => {
                                    let base64_data = base64::encode(&image_data);
                                    images.push(VerifiedImage {
                                        ipfs_cid: ipfs_cid.to_string(),
                                        data: base64_data,
                                    });
                                }
                                Err(e) => {
                                    error!("获取图片失败 {ipfs_cid}: {e}");
                                }
                            }
                        }
                    }
                }

                // 创建验证数据响应
                // let original_data = original_data_obj.clone().into_iter().collect();

                // verified_data = Some(VerifiedData {
                //     original_data,
                //     images,
                //     tx_hash: tx_hash.to_string(),
                //     cid: cid.to_string(),
                // });
            }
        }

        (
            "Certificate verified successfully".to_string(),
            verified_data,
        )
    } else {
        ("Certificate verification failed".to_string(), None)
    };

    Ok(web::Json(VerifyResponse {
        verified,
        message,
        data,
    }))
}

lazy_static::lazy_static! {
    pub static ref RANDOM: Fr = {
        let mut rng = ChaCha20Rng::seed_from_u64(0);
        Fr::random(&mut rng)
    };
}

pub fn classify_edu_data(edus: Vec<RecordData>) -> Vec<Vec<BTreeMap<String, Value>>> {
    let edu_type_size = edus.iter().map(|edu| edu.data.len()).max().unwrap_or(0);

    let mut edu_type_vec = vec![Vec::with_capacity(edus.len()); edu_type_size];

    for edu in edus {
        for (i, data) in edu.data.into_iter().enumerate() {
            edu_type_vec[i].push(data);
        }
    }

    edu_type_vec
}

pub fn compress_edu_data(data_vec: &[BTreeMap<String, Value>], random: Fr) -> Vec<Fr> {
    let decompse = data_vec.iter().map(decompse_edu_data).collect::<Vec<_>>();

    decompse
        .iter()
        .map(|data| compress_fr(data, random))
        .collect()
}

pub fn decompse_edu_data(data: &BTreeMap<String, Value>) -> Vec<Fr> {
    data.values()
        .map(|value| {
            if value.is_null() {
                return Fr::zero();
            }

            let res = match value.is_array() {
                true => {
                    let mut value_fr = Fr::zero();
                    for v in value.as_array().unwrap() {
                        let mut bytes = v.as_str().unwrap().as_bytes();
                        if bytes.len() > 31 {
                            warn!("value num too long, {:?}, size:{}", v, bytes.len());
                            bytes = &bytes[..31];
                        }
                        value_fr = value_fr.add(&Fr::from_bytes_le(bytes));
                    }
                    value_fr
                }
                false => {
                    let mut bytes = value.as_str().unwrap().as_bytes();
                    if bytes.len() > 31 {
                        warn!("value num too long, {:?}, size:{}", value, bytes.len());
                        bytes = &bytes[..31];
                    }
                    Fr::from_bytes_le(bytes)
                }
            };
            debug!("value: {value:?}, fr: {res:?}");
            res
        })
        .collect::<Vec<Fr>>()
}
