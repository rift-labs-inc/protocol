use crate::sp1_verifier_bytecode::{SP1_MOCK_VERIFIER_BYTECODE, SP1_VERIFIER_BYTECODE};
use alloy::primitives::{Address, U256};
use alloy::providers::ext::AnvilApi;
use alloy::sol;
use alloy::sol_types::SolCall;
use alloy::{hex::FromHex, pubsub::PubSubFrontend};
use bitcoin::hex::DisplayHex;
use bitcoin::Amount;
use chrono;
use hypernode::HypernodeArgs;
use log::info;
use rift_core::btc_light_client::AsLittleEndianBytes;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use tokio::runtime::Runtime;

use bitcoin::{address::NetworkChecked, hashes::Hash};
// Spawn bitcoin and anvil processes, and deploy contracts to anvil.
use alloy::{
    hex,
    network::EthereumWallet,
    node_bindings::{Anvil, AnvilInstance},
    providers::{ProviderBuilder, WsConnect},
    signers::local::PrivateKeySigner,
};
use bitcoind::{bitcoincore_rpc::RpcApi, BitcoinD};
use eyre::{eyre, Result};
use hypernode::{
    self,
    core::{EvmWebsocketProvider, RiftExchange, RiftExchangeWebsocket},
};
use rift_lib::transaction::P2WPKHBitcoinWallet;

use bitcoind::bitcoincore_rpc::Client;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    MockUSDT,
    "../hypernode/artifacts/MockUSDT.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    ERC1967Proxy,
    "../hypernode/artifacts/ERC1967Proxy.json"
);

pub fn get_new_core_aware_address(
    bitcoin_regtest_instance: &BitcoinD,
    network: bitcoin::Network,
) -> bitcoin::Address<NetworkChecked> {
    let new_address = bitcoin_regtest_instance
        .client
        .get_new_address(
            None,
            Some(bitcoind::bitcoincore_rpc::json::AddressType::Bech32),
        )
        .unwrap();
    new_address.require_network(network).unwrap()
}

pub type MockUSDTWebsocket = MockUSDT::MockUSDTInstance<PubSubFrontend, Arc<EvmWebsocketProvider>>;

pub struct RiftDevnet {
    pub bitcoin_regtest_instance: BitcoinD,
    pub anvil_instance: AnvilInstance,
    pub funded_btc_wallet: P2WPKHBitcoinWallet,
    pub miner: bitcoin::Address<NetworkChecked>,
    pub usdt_contract: Arc<MockUSDTWebsocket>,
    pub rift_exchange_contract: Arc<hypernode::core::RiftExchangeWebsocket>,
    pub mock_proof: bool,
    evm_ws_rpc: String,
    btc_rpc: String,
    hypernode_signer: PrivateKeySigner,
}

impl RiftDevnet {
    pub async fn spawn_hypernode(&self) -> Result<()> {
        let rift_exchange_address = self.rift_exchange_contract.address().to_string();
        let anvil_block_time_secs = 1;
        let bitcoin_block_time_secs = 600;
        let evm_rpc_concurrency = 10;
        let btc_rpc_concurrency = 1;
        let btc_polling_interval = 1;
        let proof_gen_concurrency = 1;
        let mock_proof = self.mock_proof;
        let flashbots = false;
        let flashbots_relay_rpc: Option<String> = None;
        let private_key = self
            .hypernode_signer
            .clone()
            .into_credential()
            .to_bytes()
            .to_lower_hex_string();
        let evm_ws_rpc = self.evm_ws_rpc.clone();
        let btc_rpc = self.btc_rpc.clone();
        // spawn hypernode
        thread::spawn(move || {
            let runtime = Runtime::new().expect("Failed to create Tokio runtime");
            runtime.block_on(async {
                hypernode::node::run(HypernodeArgs {
                    evm_ws_rpc,
                    btc_rpc,
                    private_key,
                    rift_exchange_address,
                    evm_rpc_concurrency,
                    btc_rpc_concurrency,
                    btc_polling_interval,
                    evm_block_time: anvil_block_time_secs,
                    btc_block_time: bitcoin_block_time_secs,
                    mock_proof,
                    proof_gen_concurrency,
                    flashbots,
                    flashbots_relay_rpc,
                })
                .await
                .expect("Hypernode run failed");
            });
        });
        info!("Setup complete...");
        Ok(())
    }

    pub async fn setup(mock_proof: bool) -> Result<Self> {
        let network = bitcoin::Network::Regtest;
        let (bitcoin_regtest, anvil) = tokio::try_join!(spawn_bitcoin_regtest(), spawn_anvil())?;
        let cookie = bitcoin_regtest.params.get_cookie_values()?.unwrap();
        let btc_rpc = format!(
            "http://{}:{}@127.0.0.1:{}",
            cookie.user,
            cookie.password,
            bitcoin_regtest.params.rpc_socket.port()
        );

        info!("---RIFT DEVNET---");

        info!("Bitcoin Regtest Url: {}", btc_rpc);

        info!(
            "bitcoin-cli connect: \"bitcoin-cli -regtest -rpcport={} -rpccookiefile={}\"",
            bitcoin_regtest.params.rpc_socket.port(),
            bitcoin_regtest.params.cookie_file.display()
        );

        info!("Anvil Url: {}", anvil.endpoint());

        info!("Anvil WS Url: {}", anvil.ws_endpoint());

        // Constant miner address for test consistency
        let private_key = hex!("000000000000000000000000000000000000000000000000000000000000dead");

        let miner = get_new_core_aware_address(&bitcoin_regtest, network);

        let funded_btc_wallet = P2WPKHBitcoinWallet::from_secret_key(private_key, network);

        let hypernode_signer: PrivateKeySigner = anvil.keys()[1].clone().into();
        let hypernode_address = hypernode_signer.address();

        // Generate blocks to the miner's address
        bitcoin_regtest
            .client
            .generate_to_address(101, &miner)
            .unwrap();

        let sp1_circuit_verification_hash = hex::decode(rift_lib::proof::compute_circuit_hash());

        // now setup contracts
        let (rift_exchange, usdt_contract) = deploy_contracts(
            &anvil,
            &bitcoin_regtest.client,
            hypernode_address,
            sp1_circuit_verification_hash.unwrap().try_into().unwrap(),
            mock_proof,
        )
        .await?;

        let provider = rift_exchange.provider().clone();

        let evm_ws_rpc: String = anvil.ws_endpoint();

        // give some eth using anvil
        provider
            .anvil_set_balance(hypernode_address, U256::from_str("10000000000000000000")?)
            .await?;

        Ok(RiftDevnet {
            mock_proof,
            bitcoin_regtest_instance: bitcoin_regtest,
            anvil_instance: anvil,
            miner,
            funded_btc_wallet,
            rift_exchange_contract: rift_exchange,
            usdt_contract,
            evm_ws_rpc,
            btc_rpc,
            hypernode_signer,
        })
    }

    pub fn create_btc_utxo(&self, sats: u64) -> Result<bitcoin::Transaction> {
        // the current block height
        let _txid_block_height = self.bitcoin_regtest_instance.client.get_block_count()? + 1;
        let txid = self
            .bitcoin_regtest_instance
            .client
            .send_to_address(
                &self.funded_btc_wallet.address,
                Amount::from_sat(sats),
                None,
                None,
                Some(true),
                None,
                None,
                None,
            )
            .unwrap();

        // mine the tx
        self.bitcoin_regtest_instance
            .client
            .generate_to_address(6, &self.miner)
            .unwrap();

        let tx = self
            .bitcoin_regtest_instance
            .client
            .get_raw_transaction(&txid, None)?;

        Ok(tx)
    }
}

async fn deploy_contracts(
    anvil: &AnvilInstance,
    bitcoind_client: &Client,
    hypernode_address: Address,
    circuit_verification_key_hash: [u8; 32],
    mock_proof: bool,
) -> Result<(Arc<RiftExchangeWebsocket>, Arc<MockUSDTWebsocket>)> {
    let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
    info!("Exchange owner address: {}", signer.address());
    let wallet = EthereumWallet::from(signer.clone());
    let provider: Arc<EvmWebsocketProvider> = Arc::new(
        ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_ws(WsConnect::new(anvil.ws_endpoint_url()))
            .await
            .expect("Failed to connect to WebSocket"),
    );
    let verifier_contract =
        alloy::primitives::Address::from_str("0xaeE21CeadF7A03b3034DAE4f190bFE5F861b6ebf")?;
    provider
        .anvil_set_code(
            verifier_contract,
            match mock_proof {
                true => Vec::from_hex(SP1_MOCK_VERIFIER_BYTECODE)?.into(),
                false => Vec::from_hex(SP1_VERIFIER_BYTECODE)?.into(),
            },
        )
        .await?;
    let usdt_contract = MockUSDT::deploy(provider.clone()).await?;
    let initial_checkpoint_height = bitcoind_client.get_block_count()?;
    let initial_block_hash = bitcoind_client.get_block_hash(initial_checkpoint_height)?;
    let initial_block_chainwork: [u8; 32] = bitcoind_client
        .get_block_header_info(&initial_block_hash)?
        .chainwork
        .try_into()
        .unwrap();
    let retarget_block_height = initial_checkpoint_height - (initial_checkpoint_height % 2016);
    let initial_retarget_block_hash = bitcoind_client.get_block_hash(retarget_block_height)?;

    info!("USDT address: {}", usdt_contract.address());

    let implementation = RiftExchange::deploy(provider.clone()).await?;

    let init_data = RiftExchange::initializeCall {
        initialCheckpointHeight: U256::from(initial_checkpoint_height),
        initialBlockHash: initial_block_hash.to_byte_array().into(),
        initialRetargetBlockHash: initial_retarget_block_hash
            .to_byte_array()
            .to_little_endian()
            .into(),
        initialChainwork: U256::from_be_bytes(initial_block_chainwork),
        verifierContractAddress: verifier_contract,
        depositTokenAddress: *usdt_contract.address(),
        initialFeeRouterAddress: signer.address(),
        initialOwner: signer.address(),
        verificationKeyHash: circuit_verification_key_hash.into(),
        initialPermissionedHypernodes: [hypernode_address].to_vec(),
    }
    .abi_encode();

    let proxy_contract = ERC1967Proxy::deploy(
        provider.clone(),
        *implementation.address(),
        init_data.into(),
    )
    .await?;

    let contract = RiftExchange::new(*proxy_contract.address(), provider.clone());

    Ok((Arc::new(contract), Arc::new(usdt_contract)))
}

async fn spawn_bitcoin_regtest() -> Result<BitcoinD> {
    tokio::task::spawn_blocking(|| {
        let bitcoind_path = bitcoind::exe_path().map_err(|e| eyre!(e))?;
        let mut conf = bitcoind::Conf::default();
        conf.args = vec!["-regtest", "-fallbackfee=0.0001", "-txindex"];
        BitcoinD::with_conf(bitcoind_path, &conf).map_err(|e| eyre!(e))
    })
    .await?
}

async fn spawn_anvil() -> Result<AnvilInstance> {
    tokio::task::spawn_blocking(|| {
        let _ = Anvil::new().arg("--accounts").arg("20").spawn();
        Anvil::new()
            .block_time(1)
            .chain_id(1337)
            .port(50123_u16)
            .arg("--steps-tracing")
            .arg("--timestamp")
            .arg((chrono::Utc::now().timestamp() - 9 * 60 * 60).to_string())
            .try_spawn()
            .map_err(|e| eyre!(e))
    })
    .await?
}
