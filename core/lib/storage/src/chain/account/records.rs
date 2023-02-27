// Workspace imports
use zksync_api_types::v02::account::EthAccountType as ApiEthAccountType;
// External imports
use sqlx::{types::BigDecimal, FromRow};
use zksync_types::{AccountId, Address, PubKeyHash, TokenId, H256, NFT};

#[derive(Debug, FromRow)]
pub(crate) struct StorageAccount {
    pub id: i64,
    pub last_block: i64,
    pub nonce: i64,
    pub address: Vec<u8>,
    pub pubkey_hash: Vec<u8>,
}

#[derive(Debug, FromRow)]
pub(crate) struct StorageAccountCreation {
    pub account_id: i64,
    pub is_create: bool,
    pub block_number: i64,
    pub address: Vec<u8>,
    pub nonce: i64,
    pub update_order_id: i32,
    pub autorised: bool,
}

#[derive(Debug, FromRow)]
pub(crate) struct StorageAccountUpdate {
    #[allow(dead_code)]
    pub balance_update_id: i32,
    pub account_id: i64,
    pub block_number: i64,
    pub coin_id: i32,
    pub old_balance: BigDecimal,
    pub new_balance: BigDecimal,
    pub old_nonce: i64,
    pub new_nonce: i64,
    pub update_order_id: i32,
}

#[derive(Debug, FromRow)]
pub(crate) struct StorageMintNFTUpdate {
    pub token_id: i32,
    pub serial_id: i32,
    pub creator_account_id: i32,
    pub creator_address: Vec<u8>,
    pub address: Vec<u8>,
    pub content_hash: Vec<u8>,
    pub update_order_id: i32,
    pub block_number: i64,
    pub symbol: String,
    pub nonce: i64,
}

impl From<StorageMintNFTUpdate> for NFT {
    fn from(val: StorageMintNFTUpdate) -> Self {
        Self {
            id: TokenId(val.token_id as u32),
            serial_id: val.serial_id as u32,
            creator_address: Address::from_slice(val.creator_address.as_slice()),
            creator_id: AccountId(val.creator_account_id as u32),
            address: Address::from_slice(val.address.as_slice()),
            symbol: val.symbol,
            content_hash: H256::from_slice(val.content_hash.as_slice()),
        }
    }
}

#[derive(Debug, FromRow)]
pub(crate) struct StorageAccountPubkeyUpdate {
    #[allow(dead_code)]
    pub pubkey_update_id: i32,
    pub update_order_id: i32,
    pub account_id: i64,
    pub block_number: i64,
    pub old_pubkey_hash: Vec<u8>,
    pub new_pubkey_hash: Vec<u8>,
    pub old_nonce: i64,
    pub new_nonce: i64,
}

#[derive(Debug, FromRow, Clone)]
pub(crate) struct StorageBalance {
    pub account_id: i64,
    pub coin_id: i32,
    pub balance: BigDecimal,
}

#[derive(Debug, Clone, Copy, sqlx::Type)]
#[sqlx(type_name = "eth_account_type")]
pub(crate) enum DbAccountType {
    Owned,
    CREATE2,
    No2FA,
}

pub(crate) struct StorageAccountType {
    #[allow(dead_code)]
    pub account_id: i64,
    pub account_type: DbAccountType,
}

#[derive(Debug, Clone, Copy)]
pub enum EthAccountType {
    Owned,
    CREATE2,
    No2FA(Option<PubKeyHash>),
}

impl From<EthAccountType> for ApiEthAccountType {
    fn from(account_type: EthAccountType) -> ApiEthAccountType {
        match account_type {
            EthAccountType::Owned => ApiEthAccountType::Owned,
            EthAccountType::CREATE2 => ApiEthAccountType::CREATE2,
            EthAccountType::No2FA(hash) => ApiEthAccountType::No2FA(hash),
        }
    }
}

impl EthAccountType {
    pub(crate) fn from_db(account_type: DbAccountType, pub_key_hash: Option<PubKeyHash>) -> Self {
        match account_type {
            DbAccountType::Owned => EthAccountType::Owned,
            DbAccountType::CREATE2 => EthAccountType::CREATE2,
            DbAccountType::No2FA => EthAccountType::No2FA(pub_key_hash),
        }
    }

    pub(crate) fn into_db_types(self) -> (DbAccountType, Option<PubKeyHash>) {
        match self {
            EthAccountType::Owned => (DbAccountType::Owned, None),
            EthAccountType::CREATE2 => (DbAccountType::CREATE2, None),
            EthAccountType::No2FA(hash) => (DbAccountType::No2FA, hash),
        }
    }
}
