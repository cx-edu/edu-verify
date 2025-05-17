use actix_web::{error, web, HttpResponse, Result};
use ipfs_api_backend_hyper::IpfsClient;
use log::{error, info};
use serde_json::json;
use snark_verifier_sdk::snark_verifier::halo2_base::halo2_proofs::halo2curves::{
    bn256::Fr, serde::SerdeObject,
};
use web3::{transports::Http, Web3};

use crate::services::{
    certificate::generate_certificate, ethereum::send_transaction, ipfs::process_images,
};
use crate::services::{hash_to_u64, ipfs::upload_to_ipfs};
use crate::{
    models::{RecordData, UploadResponse},
    services::shplonk::{shplonk_lagrange_commit, shplonk_lagrange_open, Commit, Proof},
};

use super::{classify_edu_data, compress_edu_data, RANDOM};

pub struct School {
    random: Fr,
}

impl School {
    pub fn new() -> Self {
        Self { random: *RANDOM }
    }

    fn handle_edu_data(&self, edus: Vec<RecordData>) -> Vec<Vec<Fr>> {
        classify_edu_data(edus)
            .iter()
            .map(|edu| compress_edu_data(edu, self.random))
            .collect::<Vec<_>>()
    }

    fn commit(&self, edu_type_compress_evals: &[Vec<Fr>]) -> Vec<Commit> {
        edu_type_compress_evals
            .iter()
            .map(|poly_evals| shplonk_lagrange_commit(poly_evals))
            .collect::<Vec<_>>()
    }

    fn open(&self, idx: u64, edu_type_compress_evals: &[Vec<Fr>]) -> Vec<(Proof, Fr)> {
        edu_type_compress_evals
            .iter()
            .map(|poly_evals| shplonk_lagrange_open(poly_evals, Fr::from_raw([idx, 0, 0, 0])))
            .collect::<Vec<_>>()
    }
}

async fn handle_images(edus: &mut [RecordData], ipfs_client: &IpfsClient) -> Result<()> {
    for edu in edus {
        for data in edu.data.iter_mut() {
            if let Some(images) = data.get_mut("images") {
                if let Some(images_array) = images.as_array_mut() {
                    let images_vec = images_array.to_vec();
                    match process_images(ipfs_client, &images_vec).await {
                        Ok(processed_images) => *images = json!(processed_images),
                        Err(e) => {
                            error!("处理图片失败: {e}");
                            return Err(error::ErrorInternalServerError(e));
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_origin_data(
    records: &[RecordData],
    ipfs_client: &IpfsClient,
) -> Result<Vec<String>> {
    let mut origin_cids = Vec::with_capacity(records.len());

    for record in records {
        let mut student_origin_cids = Vec::with_capacity(record.data.len());
        for data in record.data.iter() {
            let origin_data_cid = upload_to_ipfs(
                ipfs_client,
                serde_json::to_string(&data).unwrap().as_bytes().to_vec(),
            )
            .await
            .map_err(error::ErrorInternalServerError)?;
            student_origin_cids.push(origin_data_cid);
        }

        let origin_data_cid = upload_to_ipfs(
            ipfs_client,
            serde_json::to_string(&student_origin_cids)
                .unwrap()
                .as_bytes()
                .to_vec(),
        )
        .await
        .map_err(error::ErrorInternalServerError)?;

        origin_cids.push(origin_data_cid);
    }

    Ok(origin_cids)
}

pub async fn upload_and_gen_cert(
    records: web::Json<Vec<RecordData>>,
    web3: web::Data<Web3<Http>>,
    ipfs_client: web::Data<IpfsClient>,
) -> Result<web::Json<UploadResponse>> {
    info!("收到上传请求，开始处理...");

    let nstu = records.len();
    let mut records = records.clone();

    handle_images(&mut records, &ipfs_client).await?;
    info!("处理图片完成, edus:{records:#?}");

    let origin_cids = handle_origin_data(&records, &ipfs_client).await?;
    info!("处理原始数据完成, origin_cids:{origin_cids:#?}");

    let school = School::new();

    let edu_type_compress_evals = school.handle_edu_data(records.clone());
    info!("压缩数据完成");

    let commitments = school.commit(&edu_type_compress_evals);
    info!("生成承诺完成");

    let mut student_proof = Vec::with_capacity(nstu);
    for idx in 0..nstu {
        let idx = hash_to_u64(records[idx].id.as_bytes());
        let proofs = school.open(idx, &edu_type_compress_evals);
        let proof_bytes = proofs
            .iter()
            .map(|proof| {
                // 96bytes
                let p_bytes = proof.0 .0.to_raw_bytes();
                // 32bytes
                let v_bytes = proof.1.to_raw_bytes();
                [p_bytes, v_bytes].concat()
            })
            .flatten()
            .collect::<Vec<_>>();

        student_proof.push(proof_bytes);
    }
    info!("生成证明完成");

    let tx_hash = send_transaction(
        &web3,
        commitments
            .iter()
            .flat_map(|commitment| commitment.0.to_raw_bytes())
            .collect::<Vec<_>>(),
    )
    .await
    .map_err(error::ErrorInternalServerError)?;

    let tx_hash_str = format!("{tx_hash:?}");
    info!("交易哈希: {tx_hash_str}");

    let mut processed_records = Vec::with_capacity(nstu);
    for idx in 0..nstu {
        processed_records.push((
            records[idx].id.clone(),
            origin_cids[idx].clone(),
            hex::encode(student_proof[idx].clone()),
            tx_hash_str.clone(),
        ));
    }

    let certificate_filename = generate_certificate(&processed_records)
        .await
        .map_err(error::ErrorInternalServerError)?;
    info!("证书文件名: {certificate_filename:?}");

    let response = UploadResponse {
        success: true,
        certificate_file: certificate_filename,
    };
    info!("上传成功返回结果: {response:?}");

    Ok(web::Json(response))
}

pub async fn download_certificate(filename: web::Path<String>) -> Result<HttpResponse> {
    let file_path = format!("./certificates/{filename}");

    match std::fs::read(&file_path) {
        Ok(contents) => Ok(HttpResponse::Ok()
            .content_type("application/zip")
            .append_header((
                "Content-Disposition",
                format!("attachment; filename={filename}"),
            ))
            .body(contents)),
        Err(_) => Err(error::ErrorNotFound("Certificate file not found")),
    }
}

#[cfg(test)]
mod tests {
    use ipfs_api_backend_hyper::TryFromUri;

    use super::*;
    use crate::services::{
        commit::verify,
        ethereum::{create_web3_connection, get_transaction_data},
    };

    #[tokio::test]
    async fn test_proof() {
        use crate::models::Certificate;
        use crate::services::ipfs::get_json_from_ipfs;

        let ipfs_url = "http://localhost:5001";
        let ipfs_client = IpfsClient::from_str(ipfs_url).unwrap();
        let web3 = create_web3_connection().await.unwrap();

        let certificate = Certificate {
            id: "D202501".to_string(),
            original_data_cid: "QmacwPSc6E9fUssVZvGnaCrWTunWWE5vPUQotKDd78urHo".to_string(),
            proof: "1ba2cb311e95d71f735040527e83043cdf6d5611fa49b6f638806fcee6809508".to_string(),
            tx_hash: "0xc6b00aa31d19f36b0824a86b4fa34a3fe8db941997f10a75f69a8a578b9047ee"
                .to_string(),
        };
        println!("证书信息: {certificate:#?}");

        let origin_data = get_json_from_ipfs(&ipfs_client, &certificate.original_data_cid)
            .await
            .unwrap();
        println!("IPFS原始数据: {origin_data}");

        let commit = get_transaction_data(&web3, &certificate.tx_hash)
            .await
            .unwrap();
        println!("区块链存证数据: {commit}");

        let res = verify(origin_data, commit, certificate.proof);
        println!("证书验证结果: {res}");
    }
}
