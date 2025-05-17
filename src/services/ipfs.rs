use anyhow::Context;
use futures_util::StreamExt;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient};
use log::info;
use serde_json::Value;
use std::io::Cursor;

pub async fn upload_to_ipfs(client: &IpfsClient, data: Vec<u8>) -> anyhow::Result<String> {
    info!("开始上传文件到IPFS...");
    let cursor = Cursor::new(data);
    let res = client
        .add(cursor)
        .await
        .context("Failed to upload to IPFS")?;
    info!("IPFS上传成功, CID: {}", res.hash);
    Ok(res.hash)
}

pub async fn get_image_from_ipfs(ipfs_client: &IpfsClient, cid: &str) -> anyhow::Result<Vec<u8>> {
    info!("从IPFS获取图片数据, CID: {cid}");
    let mut stream = ipfs_client.cat(cid);
    let mut data = Vec::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to get chunk from IPFS")?;
        data.extend_from_slice(&chunk);
    }

    info!("成功从IPFS获取图片数据，大小: {} bytes", data.len());
    Ok(data)
}

pub async fn get_json_from_ipfs(ipfs_client: &IpfsClient, cid: &str) -> anyhow::Result<String> {
    info!("从IPFS获取json数据, CID: {cid}");
    let mut stream = ipfs_client.cat(cid);
    let mut data = Vec::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to get chunk from IPFS")?;
        data.extend_from_slice(&chunk);
    }

    let json_str = String::from_utf8(data).context("Failed to parse JSON")?;
    info!("成功从IPFS获取json数据, {json_str}");
    Ok(json_str)
}

pub async fn process_images(
    ipfs_client: &IpfsClient,
    images: &Vec<Value>,
) -> anyhow::Result<Vec<String>> {
    let mut processed_images = Vec::new();

    for image in images {
        if let Some(data) = image.get("data").and_then(Value::as_str) {
            // 解码Base64数据
            let image_data = base64::decode(data)
                .map_err(|e| anyhow::anyhow!("Failed to decode base64 data: {}", e))?;

            // 上传到IPFS
            let ipfs_hash = upload_to_ipfs(ipfs_client, image_data.clone()).await?;

            processed_images.push(ipfs_hash);
        }
    }

    Ok(processed_images)
}
