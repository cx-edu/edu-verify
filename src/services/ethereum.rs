use log::{error, info};
use reqwest::Url;
use std::sync::OnceLock;
use web3::contract::{Contract, Options};
use web3::transports::Http;
use web3::types::{TransactionRequest, H256, U256};
use web3::Web3;

// 添加合约ABI和字节码
pub const CONTRACT_ABI: &str = include_str!("../../contracts/CertificateVerifier.abi");
pub const CONTRACT_BYTECODE: &str = include_str!("../../contracts/CertificateVerifier.bin");

// 添加合约地址存储
pub static CONTRACT_ADDRESS: OnceLock<String> = OnceLock::new();

pub async fn create_web3_connection() -> anyhow::Result<Web3<Http>> {
    info!("创建以太坊连接...");

    let eth_url =
        std::env::var("ETHEREUM_NODE_URL").unwrap_or_else(|_| "http://127.0.0.1:8545".to_string());
    info!("使用以太坊节点地址: {eth_url}");

    // 创建不使用代理的 HTTP 客户端
    let http_client = reqwest::Client::builder().no_proxy().build()?;

    let url = Url::parse(&eth_url)?;
    let http = Http::with_client(http_client, url);

    let web3 = Web3::new(http);
    info!("Web3实例创建成功");

    Ok(web3)
}

pub async fn deploy_contract(web3: &Web3<Http>) -> anyhow::Result<String> {
    info!("开始部署合约...");

    // 获取账户列表
    let accounts = web3.eth().accounts().await?;
    info!("获取到的账户列表: {accounts:?}");

    let account = accounts
        .first()
        .ok_or_else(|| anyhow::anyhow!("没有找到可用的以太坊账户"))?;
    info!("使用账户 {account} 部署合约");

    // 获取当前 gas price
    let gas_price = web3.eth().gas_price().await?;
    info!("当前 gas price: {gas_price} wei");

    // 打印合约字节码
    info!("合约字节码: {CONTRACT_BYTECODE}");
    info!("合约 ABI: {CONTRACT_ABI}");

    // 部署合约
    info!("开始部署合约...");
    let contract = Contract::deploy(web3.eth(), CONTRACT_ABI.as_bytes())?
        .confirmations(0) // 不等待确认
        .options(Options::with(|opt| {
            opt.gas = Some(3_000_000.into());
            opt.gas_price = Some(gas_price);
        }))
        .execute(CONTRACT_BYTECODE, (), *account)
        .await
        .map_err(|e| {
            error!("合约部署失败: {e}");
            anyhow::anyhow!("合约部署失败: {}", e)
        })?;

    let address = format!("{:?}", contract.address());
    info!("合约部署成功！");
    info!("合约地址: {address}");

    Ok(address)
}

pub async fn send_transaction(web3: &Web3<Http>, data: Vec<u8>) -> anyhow::Result<H256> {
    let accounts = web3.eth().accounts().await?;
    let account = accounts
        .first()
        .ok_or_else(|| anyhow::anyhow!("No Ethereum account available"))?;

    let gas_price = web3.eth().gas_price().await?;

    let tx = TransactionRequest {
        from: *account,
        to: None,
        gas: Some(U256::from(3000000)),
        gas_price: Some(gas_price),
        value: Some(U256::from(0)),
        data: Some(web3::types::Bytes(data)),
        nonce: None,
        condition: None,
        transaction_type: None,
        access_list: None,
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
    };

    web3.eth()
        .send_transaction(tx)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send transaction: {}", e))
}

pub async fn get_transaction_data(web3: &Web3<Http>, tx_hash: &str) -> anyhow::Result<String> {
    let tx_hash_bytes = hex::decode(&tx_hash[2..])
        .map_err(|e| anyhow::anyhow!("Invalid transaction hash: {}", e))?;
    let tx_hash = H256::from_slice(&tx_hash_bytes);

    let transaction = web3
        .eth()
        .transaction(web3::types::TransactionId::Hash(tx_hash))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch transaction: {}", e))?;

    match transaction {
        Some(tx) => {
            if tx.input.0.is_empty() {
                Ok("".to_string())
            } else {
                Ok(hex::encode(tx.input.0))
            }
        }
        None => Err(anyhow::anyhow!("Transaction not found")),
    }
}
