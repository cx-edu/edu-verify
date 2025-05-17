use std::collections::BTreeMap;

use actix_web::{error, web, Result};
use ipfs_api_backend_hyper::IpfsClient;
use log::{error, info};
use serde_json::Value;
use snark_verifier_sdk::snark_verifier::halo2_base::halo2_proofs::halo2curves::bn256::Fr;
use snark_verifier_sdk::snark_verifier::halo2_base::halo2_proofs::halo2curves::serde::SerdeObject;
use snark_verifier_sdk::snark_verifier::halo2_base::utils::fs::gen_srs;

use crate::handler::{decompse_edu_data, RANDOM};
use crate::models::{
    AuthenticationData, Certificate, GenerateAuthenticationData, VerifiedData, VerifiedImage,
};
use crate::services::circuit::CircuitParams;
use crate::services::circuit::CircuitProver;
use crate::services::ipfs::get_image_from_ipfs;
use crate::services::ipfs::{get_json_from_ipfs, upload_to_ipfs};
use crate::services::poseidon::poseidon;
use crate::services::{compress_fr, compress_g1};

lazy_static::lazy_static! {
    static ref PROVER: CircuitProver = CircuitProver::new(CircuitParams {
        degree: 16,
        num_instance: 1,
        lookup_bits: 16,
        limb_bits: 88,
        num_limbs: 3,
    }, gen_srs(16));
}

pub async fn get_images(
    ipfs_client: &IpfsClient,
    data: &BTreeMap<String, Value>,
) -> Result<Vec<VerifiedImage>> {
    let mut verified_images = Vec::new();

    if let Some(images) = data.get("images") {
        if let Some(images_array) = images.as_array() {
            for image in images_array {
                let cid = image.as_str().unwrap();
                match get_image_from_ipfs(ipfs_client, cid).await {
                    Ok(image_data) => {
                        let base64_data = base64::encode(&image_data);
                        verified_images.push(VerifiedImage {
                            ipfs_cid: cid.to_string(),
                            data: base64_data,
                        });
                    }
                    Err(e) => {
                        error!("获取图片失败 {cid}: {e}");
                    }
                }
            }
        }
    }

    Ok(verified_images)
}

pub async fn upload(
    certs: web::Json<Vec<Certificate>>,
    ipfs_client: web::Data<IpfsClient>,
) -> Result<web::Json<Vec<VerifiedData>>> {
    let mut verified_data_vec = Vec::new();

    for cert in certs.iter() {
        let edu_type_cids = get_json_from_ipfs(&ipfs_client, &cert.original_data_cid)
            .await
            .map_err(error::ErrorInternalServerError)?;

        let str_cids = serde_json::from_str::<Vec<String>>(&edu_type_cids).unwrap();

        let proof = hex::decode(cert.proof.clone()).unwrap();
        let proof_chunk = proof.chunks(128).collect::<Vec<_>>();

        for (i, cid) in str_cids.iter().enumerate() {
            let original_data = get_json_from_ipfs(&ipfs_client, cid)
                .await
                .map_err(error::ErrorInternalServerError)?;

            let original_data_obj = serde_json::from_str::<BTreeMap<String, Value>>(&original_data)
                .map_err(error::ErrorInternalServerError)?;

            let images = get_images(&ipfs_client, &original_data_obj).await?;

            let verified_data = VerifiedData {
                original_data: original_data_obj,
                images,
                tx_hash: cert.tx_hash.clone(),
                cid: cid.to_string(),
                proof: hex::encode(proof_chunk[i]),
            };

            verified_data_vec.push(verified_data);
        }
    }

    info!("数据收集成功");
    Ok(web::Json(verified_data_vec))
}

pub async fn generate_authentication(
    auths: web::Json<Vec<GenerateAuthenticationData>>,
    ipfs_client: web::Data<IpfsClient>,
) -> Result<web::Json<AuthenticationData>> {
    let mut origin_data_vec_vec = Vec::new();
    let mut selected_fields_vec_vec = Vec::new();
    for auth in auths.iter() {
        let mut origin_data_vec = Vec::new();
        for auth_type_cid in auth.cid.iter() {
            let origin_data = get_json_from_ipfs(&ipfs_client, auth_type_cid)
                .await
                .map_err(error::ErrorInternalServerError)?;

            let origin_data_obj = serde_json::from_str::<BTreeMap<String, Value>>(&origin_data)
                .map_err(error::ErrorInternalServerError)?;

            origin_data_vec.push(origin_data_obj);
        }
        origin_data_vec_vec.push(origin_data_vec);
        selected_fields_vec_vec.push(auth.selected_fields.clone());
    }

    let mut compress_data_vec_vec = Vec::new();
    for origin_data_vec in origin_data_vec_vec.iter() {
        let mut compress_data_vec = Vec::new();
        for origin_data in origin_data_vec {
            let decompress_data = decompse_edu_data(origin_data);

            let compress_data = compress_fr(&decompress_data, *RANDOM);

            compress_data_vec.push(compress_data);
        }
        compress_data_vec_vec.push(compress_data_vec);
    }

    let xi = poseidon(
        &compress_data_vec_vec
            .clone()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
    );

    let mut final_compress_data_vec = Vec::new();
    let mut compress_proof_vec = Vec::new();
    let mut compress_value_vec = Vec::new();
    for (compress_data_vec, auth) in compress_data_vec_vec.iter().zip(auths.iter()) {
        let compress_data = compress_fr(compress_data_vec, xi);

        let (proofs, values) = auth
            .proof
            .iter()
            .map(|proof| {
                let proof_bytes = hex::decode(proof).unwrap();
                let proof = proof_bytes[..96].to_vec();
                let value = Fr::from_raw_bytes(&proof_bytes[96..128]).expect("value 格式错误");
                (proof, value)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .unzip::<Vec<u8>, Fr, Vec<_>, Vec<_>>();

        let compress_proof = compress_g1(&proofs, xi);
        let compress_value = compress_fr(&values, xi);

        compress_proof_vec.push(compress_proof);
        compress_value_vec.push(compress_value);
        final_compress_data_vec.push(compress_data);
    }

    let delta = poseidon(&final_compress_data_vec);
    let final_compress_proof = compress_g1(&compress_proof_vec, delta);
    let final_compress_value = compress_fr(&compress_value_vec, delta);

    let proof = hex::encode(
        [
            final_compress_proof.clone(),
            final_compress_value.to_raw_bytes(),
        ]
        .concat(),
    );

    // if have private data, then we need gen zk proof
    let is_zk = auths.iter().any(|auth| auth.is_zk);
    let zk_proof = if is_zk {
        let zk_proof_bytes = (*PROVER).gen_proof(&origin_data_vec_vec, &selected_fields_vec_vec);
        // TODO: remove
        println!("size:{}", zk_proof_bytes.len());

        hex::encode([zk_proof_bytes, compress_proof_vec.concat()].concat())
    } else {
        String::new()
    };

    let mut new_origin_data_cid = Vec::new();
    for (origin_data_vec, selected_fields) in origin_data_vec_vec
        .iter()
        .zip(selected_fields_vec_vec.iter())
    {
        let mut new_origin_data_vec = Vec::new();
        for (i, origin_data) in origin_data_vec.iter().enumerate() {
            let mut new_origin_data_obj = origin_data.clone();
            for key in origin_data.keys() {
                if !selected_fields[i].contains(key) {
                    new_origin_data_obj.insert(key.to_string(), Value::Null);
                }
            }
            let new_cid = upload_to_ipfs(
                &ipfs_client,
                serde_json::to_string(&new_origin_data_obj)
                    .unwrap()
                    .as_bytes()
                    .to_vec(),
            )
            .await
            .map_err(error::ErrorInternalServerError)?;
            new_origin_data_vec.push(new_cid);
        }

        new_origin_data_cid.push(
            upload_to_ipfs(
                &ipfs_client,
                serde_json::to_string(&new_origin_data_vec)
                    .unwrap()
                    .as_bytes()
                    .to_vec(),
            )
            .await
            .unwrap(),
        );
    }

    let response = AuthenticationData {
        id: auths.iter().map(|auth| auth.id.clone()).collect(),
        data_cid: new_origin_data_cid,
        tx_hash: auths.iter().map(|auth| auth.tx_hash.clone()).collect(),
        proof,
        zk_proof,
        random: if is_zk {
            hex::encode(xi.to_bytes())
        } else {
            String::new()
        },
    };

    info!("生成认证数据成功, 返回结果: {response:#?}");

    Ok(web::Json(response))
}

#[cfg(test)]
mod tests {
    use ipfs_api_backend_hyper::TryFromUri;
    use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
    use snark_verifier_sdk::snark_verifier::{
        halo2_base::halo2_proofs::halo2curves::bn256::G1, util::arithmetic::Group,
    };

    use super::*;
    use crate::services::{
        commit::verify_proof,
        ethereum::{create_web3_connection, get_transaction_data},
    };

    #[tokio::test]
    async fn test_auth_proof() {
        use crate::models::AuthenticationData;
        use crate::services::ipfs::get_json_from_ipfs;

        let ipfs_url = "http://localhost:5001";
        let ipfs_client = IpfsClient::from_str(ipfs_url).unwrap();
        let web3 = create_web3_connection().await.unwrap();

        let auth_data = AuthenticationData {
            id: vec!["D202501".to_string()],
            data_cid: vec!["QmWHJJWskF2dzXPFgybPj2HSeTqMx9bUDCAUUN5D2d8Vwh".to_string()],
            tx_hash: vec![
                "0xc6b00aa31d19f36b0824a86b4fa34a3fe8db941997f10a75f69a8a578b9047ee".to_string(),
            ],
            proof: "eac9cf9af6438f8a0b82323285f063840c4e98fce3cff86b456958ce50479c7f".to_string(),
            zk_proof: "130bd08c2c842588d4be476747459815d9547277c750b183e041cb3c59395bbd"
                .to_string(),
            random: "693f0c0000000000".to_string(),
        };
        println!("认证文件: {auth_data:#?}");

        let mut data_vec = Vec::new();
        let mut commit_vec = Vec::new();

        for (data_cid, tx_hash) in auth_data.data_cid.iter().zip(auth_data.tx_hash.iter()) {
            let origin_data = get_json_from_ipfs(&ipfs_client, data_cid).await.unwrap();
            println!("IPFS原始数据: {origin_data}");

            let commit = get_transaction_data(&web3, tx_hash).await.unwrap();
            println!("区块链存证数据: {commit}");

            data_vec.push(origin_data);
            commit_vec.push(commit);
        }

        let res = verify_proof(
            &data_vec,
            &commit_vec,
            auth_data.proof,
            auth_data.zk_proof,
            auth_data.random,
        );
        println!("认证文件验证结果: {res}");
    }

    #[test]
    fn test_g1_bytes() {
        let rng = ChaCha20Rng::seed_from_u64(0);

        // let g1 = G1::random(&mut rng);
        // let bytes = g1.to_raw_bytes();
        // println!("bytes size: {}", bytes.len());

        let bytes = vec![
            172, 1, 181, 207, 35, 147, 197, 101, 164, 8, 98, 4, 206, 148, 216, 233, 18, 5, 120,
            166, 100, 1, 12, 76, 255, 16, 51, 40, 166, 188, 123, 14, 179, 44, 70, 35, 19, 213, 0,
            214, 99, 23, 250, 213, 178, 17, 249, 47, 212, 130, 59, 174, 68, 149, 206, 208, 97, 145,
            31, 140, 15, 84, 13, 37, 78, 206, 172, 184, 149, 131, 10, 220, 146, 176, 57, 29, 70,
            174, 55, 15, 239, 92, 155, 239, 54, 211, 142, 221, 158, 59, 16, 11, 220, 134, 253, 36,
        ];

        let g1_from_bytes = G1::from_raw_bytes(&bytes).unwrap();

        println!("g1_from_bytes: {g1_from_bytes:?}");
        // assert_eq!(g1, g1_from_bytes);
    }
}
