use std::collections::BTreeMap;

use actix_web::{error, web, Result};
use ipfs_api_backend_hyper::IpfsClient;
use log::{error, info};
use serde_json::Value;
use web3::contract::Contract;
use web3::{transports::Http, Web3};

use crate::handler::student::get_images;
use crate::models::{AuthVerifyData, AuthVerifyResultData, AuthenticationData};
use crate::services::ethereum::get_transaction_data;
use crate::services::ethereum::{CONTRACT_ABI, CONTRACT_ADDRESS};
use crate::services::ipfs::get_json_from_ipfs;

pub async fn upload(
    auth_data: web::Json<AuthenticationData>,
    ipfs_client: web::Data<IpfsClient>,
) -> Result<web::Json<AuthVerifyData>> {
    let mut data_vec_vec = Vec::new();
    let mut images_vec_vec = Vec::new();
    let mut tx_hashs_vec = Vec::new();

    for (data_cid, tx_hash) in auth_data.data_cid.iter().zip(auth_data.tx_hash.iter()) {
        let original_data_cid = get_json_from_ipfs(&ipfs_client, data_cid)
            .await
            .map_err(error::ErrorInternalServerError)?;
        info!("IPFS原始数据: {original_data_cid}");
        let original_data_cid_vec = serde_json::from_str::<Vec<String>>(&original_data_cid)
            .map_err(error::ErrorInternalServerError)?;

        let mut original_data_obj_vec = Vec::new();
        let mut images_vec = Vec::new();

        for original_data_cid in original_data_cid_vec {
            let original_data = get_json_from_ipfs(&ipfs_client, &original_data_cid)
                .await
                .map_err(error::ErrorInternalServerError)?;

            let mut original_data_obj =
                serde_json::from_str::<BTreeMap<String, Value>>(&original_data)
                    .map_err(error::ErrorInternalServerError)?;

            let images = get_images(&ipfs_client, &original_data_obj)
                .await
                .map_err(error::ErrorInternalServerError)?;

            original_data_obj.remove("images");

            original_data_obj_vec.push(original_data_obj);
            images_vec.push(images);
        }

        data_vec_vec.push(original_data_obj_vec);
        images_vec_vec.push(images_vec);
        tx_hashs_vec.push(tx_hash.clone());
    }

    let verified_data = AuthVerifyData {
        data: data_vec_vec,
        images: images_vec_vec,
        tx_hashs: tx_hashs_vec,
        proof: auth_data.proof.clone(),
        zk_proof: auth_data.zk_proof.clone(),
        random: auth_data.random.clone(),
    };

    info!("数据收集成功");
    Ok(web::Json(verified_data))
}

pub async fn verify_auth_data(
    auth_data: web::Json<AuthVerifyData>,
    web3: web::Data<Web3<Http>>,
) -> Result<web::Json<AuthVerifyResultData>> {
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

    let mut blockchain_data_vec = Vec::new();
    for tx_hash in auth_data.tx_hashs.iter() {
        match get_transaction_data(&web3, tx_hash).await {
            Ok(data) => {
                blockchain_data_vec.push(data);
            }
            Err(e) => {
                error!("获取交易数据失败: {}", e);
                return Ok(web::Json(AuthVerifyResultData {
                    verified: false,
                    tx_hash: tx_hash.clone(),
                }));
            }
        }
    }

    let original_data_bytes =
        web3::types::Bytes::from(serde_json::to_string(&auth_data.data).unwrap().as_bytes());
    let blockchain_data_bytes = web3::types::Bytes::from(
        serde_json::to_string(&blockchain_data_vec)
            .unwrap()
            .as_bytes(),
    );

    info!("数据准备完毕");

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
    let call_result = contract
        .call(
            "verifyData",
            (
                original_data_bytes.clone(),
                blockchain_data_bytes.clone(),
                auth_data.proof.clone(),
                auth_data.zk_proof.clone(),
                auth_data.random.clone(),
            ),
            account,
            Default::default(),
        )
        .await
        .map_err(error::ErrorInternalServerError)?;

    let tx_hash = format!("{call_result:?}");

    // 查询验证结果
    let verified: bool = contract
        .query(
            "verifyData",
            (
                original_data_bytes,
                blockchain_data_bytes,
                auth_data.proof.clone(),
                auth_data.zk_proof.clone(),
                auth_data.random.clone(),
            ),
            account,
            Default::default(),
            None,
        )
        .await
        .map_err(error::ErrorInternalServerError)?;

    Ok(web::Json(AuthVerifyResultData { verified, tx_hash }))
}
