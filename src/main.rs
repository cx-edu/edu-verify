use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use dotenv::dotenv;
use handler::{company, school, student, verify_hash};
use ipfs_api_backend_hyper::{IpfsClient, TryFromUri};
use log::{error, info};
use std::env;

mod handler;
mod models;
mod services;

use services::ethereum::{create_web3_connection, deploy_contract, CONTRACT_ADDRESS};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    env_logger::Builder::new()
        .filter_module("edu_verify", log::LevelFilter::Info)
        .filter_level(log::LevelFilter::Off)
        .init();

    info!("Starting server at http://127.0.0.1:3000");

    // 检查环境变量
    let eth_url =
        env::var("ETHEREUM_NODE_URL").unwrap_or_else(|_| "http://127.0.0.1:8545".to_string());
    let ipfs_url = env::var("IPFS_API_URL").unwrap_or_else(|_| "http://localhost:5001".to_string());

    info!("配置信息:");
    info!("IPFS endpoint: {ipfs_url}");
    info!("Ethereum endpoint: {eth_url}");

    // 初始化Web3连接并部署合约
    info!("正在连接以太坊网络...");
    let web3 = match create_web3_connection().await {
        Ok(web3) => {
            info!("成功连接到以太坊网络");
            web3
        }
        Err(e) => {
            error!("连接以太坊网络失败: {e}");
            panic!("无法连接到以太坊网络");
        }
    };

    // 初始化IPFS客户端
    let ipfs_client = match IpfsClient::from_str(&ipfs_url) {
        Ok(ipfs_client) => {
            info!("成功连接到IPFS网络");
            ipfs_client
        }
        Err(e) => {
            error!("连接IPFS网络失败: {e}");
            panic!("无法连接到IPFS网络");
        }
    };

    info!("正在部署合约...");
    let contract_address = match deploy_contract(&web3).await {
        Ok(address) => {
            info!("合约部署成功，地址: {address}");
            address
        }
        Err(e) => {
            error!("合约部署失败: {e}");
            panic!("合约部署失败");
        }
    };

    match CONTRACT_ADDRESS.set(contract_address.clone()) {
        Ok(_) => info!("合约地址已保存"),
        Err(_) => error!("无法保存合约地址"),
    }

    info!("Contract deployed at: {contract_address}");

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(web3.clone()))
            .app_data(web::Data::new(ipfs_client.clone()))
            .service(
                web::resource("/api/school/upload")
                    .route(web::post().to(school::upload_and_gen_cert)),
            )
            .service(
                web::resource("/api/school/download/{filename}")
                    .route(web::get().to(school::download_certificate)),
            )
            .service(web::resource("/api/student/upload").route(web::post().to(student::upload)))
            .service(
                web::resource("/api/student/generate-authentication")
                    .route(web::post().to(student::generate_authentication)),
            )
            .service(web::resource("/api/company/upload").route(web::post().to(company::upload)))
            .service(
                web::resource("/api/company/verify-auth-data")
                    .route(web::post().to(company::verify_auth_data)),
            )
            // only for test
            .service(web::resource("/verify").route(web::post().to(verify_hash)))
    })
    .bind("127.0.0.1:3000")?
    .run()
    .await
}
