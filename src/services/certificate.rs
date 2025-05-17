use crate::models::Certificate;
use chrono::Local;
use std::fs::File;
use std::io::Write;
use zip::{write::FileOptions, ZipWriter};

pub async fn generate_certificate(
    records: &[(String, String, String, String)],
) -> anyhow::Result<String> {
    let mut certificates = Vec::new();

    for (id, origin_data_cid, proof, tx_hash) in records {
        let certificate = Certificate {
            id: id.clone(),
            original_data_cid: origin_data_cid.clone(),
            proof: proof.clone(),
            tx_hash: tx_hash.clone(),
        };
        certificates.push(certificate);
    }

    // 创建一个ZIP文件来存储所有证书
    let timestamp_str = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let zip_filename = format!("certificates_{timestamp_str}.zip");
    let zip_path = format!("certificates/{zip_filename}");

    // 确保目录存在
    std::fs::create_dir_all("certificates")?;

    let zip_file = File::create(&zip_path)?;
    let mut zip = ZipWriter::new(zip_file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // 将每个证书写入ZIP文件
    for certificate in certificates.iter() {
        let cert_filename = format!("{}.json", certificate.id);
        zip.start_file(&cert_filename, options)?;

        let cert_json = serde_json::to_string_pretty(certificate)?;
        zip.write_all(cert_json.as_bytes())?;
    }

    zip.finish()?;

    Ok(zip_filename)
}
