use num::BigUint;
use zksync_eth_signer::EthereumSigner;
use zksync_types::{AccountId, Address, TokenId, TokenLike};

use crate::{
    credentials::WalletCredentials,
    error::ClientError,
    ethereum::EthereumProvider,
    operations::*,
    provider::Provider,
    signer::Signer,
    tokens_cache::TokensCache,
    types::{AccountInfo, BlockStatus, NFT},
};
use web3::contract::Options;
use web3::signing::Key;
use web3::types::U256;

#[derive(Debug)]
pub struct Wallet<S: EthereumSigner, P: Provider> {
    pub provider: P,
    pub signer: Signer<S>,
    pub tokens: TokensCache,
}

impl<S, P> Wallet<S, P>
where
    S: EthereumSigner,
    P: Provider + Clone,
{
    pub async fn new(provider: P, credentials: WalletCredentials<S>) -> Result<Self, ClientError> {
        let account_info = provider.account_info(credentials.eth_address).await?;

        let mut signer = Signer::with_credentials(credentials);
        signer.set_account_id(account_info.id);

        let tokens = TokensCache::new(provider.tokens().await?);

        Ok(Wallet {
            provider,
            signer,
            tokens,
        })
    }

    /// Updates account ID stored in the wallet.
    /// This method must be invoked if the wallet was created for a non-existent account,
    /// and it was initialized after creation (e.g. by doing a deposit).
    /// If zkSync wallet was initialized, but information in `Wallet` object was not updated,
    /// `Wallet` won't be able to perform any zkSync transaction.
    pub async fn update_account_id(&mut self) -> Result<(), ClientError> {
        let account_info = self.provider.account_info(self.address()).await?;
        self.signer.set_account_id(account_info.id);
        Ok(())
    }

    /// Returns the wallet address.
    pub fn address(&self) -> Address {
        self.signer.address
    }

    /// Returns account state info.
    pub async fn account_info(&self) -> Result<AccountInfo, ClientError> {
        self.provider.account_info(self.address()).await
    }

    /// Returns balance in the account.
    pub async fn get_balance(
        &self,
        block_status: BlockStatus,
        token_like: impl Into<TokenLike>,
    ) -> Result<BigUint, ClientError> {
        let token = self
            .tokens
            .resolve(token_like.into())
            .ok_or(ClientError::UnknownToken)?;

        let account_state = match block_status {
            BlockStatus::Committed => self.account_info().await?.committed,
            BlockStatus::Verified => self.account_info().await?.verified,
        };

        Ok(account_state
            .balances
            .get(&token.symbol as &str)
            .map(|x| x.0.clone())
            .unwrap_or_default())
    }

    /// Returns nft in the account.
    pub async fn get_nft(
        &self,
        block_status: BlockStatus,
        token_id: TokenId,
    ) -> Result<Option<NFT>, ClientError> {
        let account_state = match block_status {
            BlockStatus::Committed => self.account_info().await?.committed,
            BlockStatus::Verified => self.account_info().await?.verified,
        };

        Ok(account_state.nfts.get(&token_id).cloned())
    }

    /// Returns the current account ID.
    /// Result may be `None` if the signing key was not set for account via `ChangePubKey` transaction.
    pub fn account_id(&self) -> Option<AccountId> {
        self.signer.account_id
    }

    /// Updates the list of tokens supported by zkSync.
    /// This method only needs to be called if a new token was added to zkSync after
    /// `Wallet` object was created.
    pub async fn refresh_tokens_cache(&mut self) -> Result<(), ClientError> {
        self.tokens = TokensCache::new(self.provider.tokens().await?);

        Ok(())
    }

    /// Returns `true` if signing key for account was set in zkSync network.
    /// In other words, returns `true` if `ChangePubKey` operation was performed for the
    /// account.
    ///
    /// If this method has returned `false`, one must send a `ChangePubKey` transaction
    /// via `Wallet::start_change_pubkey` method.
    pub async fn is_signing_key_set(&self) -> Result<bool, ClientError> {
        let account_info = self.provider.account_info(self.address()).await?;
        let signer_pub_key_hash = self.signer.pubkey_hash();

        let key_set = account_info.id.is_some()
            && &account_info.committed.pub_key_hash == signer_pub_key_hash;
        Ok(key_set)
    }

    /// Initializes `Transfer` transaction sending.
    pub fn start_transfer(&self) -> TransferBuilder<'_, S, P> {
        TransferBuilder::new(self)
    }

    /// Initializes `TransferNFT` transaction sending.
    pub fn start_transfer_nft(&self) -> TransferNFTBuilder<'_, S, P> {
        TransferNFTBuilder::new(self)
    }

    /// Initializes `ChangePubKey` transaction sending.
    pub fn start_change_pubkey(&self) -> ChangePubKeyBuilder<'_, S, P> {
        ChangePubKeyBuilder::new(self)
    }

    /// Initializes `Withdraw` transaction sending.
    pub fn start_withdraw(&self) -> WithdrawBuilder<'_, S, P> {
        WithdrawBuilder::new(self)
    }

    /// Initializes `ForcedExit` transaction sending
    pub fn start_forced_exit(&self) -> ForcedExitBuilder<'_, S, P> {
        ForcedExitBuilder::new(self)
    }

    /// Initializes `ChangeGroup` transaction sending.
    pub fn start_change_group(&self) -> ChangeGroupBuilder<'_, S, P> {
        ChangeGroupBuilder::new(self)
    }

    /// Initializes `Order` builder, notice an order is not a zksync transaction.
    pub fn start_order(&self) -> OrderBuilder<'_, S, P> {
        OrderBuilder::new(self)
    }

    /// Initializes `Swap` transaction sending.
    pub fn start_swap(&self) -> SwapBuilder<'_, S, P> {
        SwapBuilder::new(self)
    }

    /// Initializes `MintNFT` transaction sending.
    pub fn start_mint_nft(&self) -> MintNFTBuilder<'_, S, P> {
        MintNFTBuilder::new(self)
    }

    /// Initializes `WithdrawNFT` transaction sending.
    pub fn start_withdraw_nft(&self) -> WithdrawNFTBuilder<'_, S, P> {
        WithdrawNFTBuilder::new(self)
    }

    /// Creates an `EthereumProvider` to interact with the Ethereum network.
    ///
    /// Returns an error if wallet was created without providing an Ethereum private key.
    pub async fn ethereum(
        &self,
        web3_addr: impl AsRef<str>,
    ) -> Result<EthereumProvider<S>, ClientError> {
        if let Some(eth_signer) = &self.signer.eth_signer {
            let ethereum_provider = EthereumProvider::new(
                &self.provider,
                self.tokens.clone(),
                web3_addr,
                eth_signer.clone(),
                self.signer.address,
            )
            .await?;

            Ok(ethereum_provider)
        } else {
            Err(ClientError::NoEthereumPrivateKey)
        }
    }

    pub async fn get_pending_balance(&self) -> Result<(), ClientError> {
        let ethereum = self.ethereum("http://127.0.0.1:8545").await?;

        let main_contract = {
            let address = self.provider.contract_address().await?;
            let add = address.main_contract.parse().unwrap();
            ethereum.client().main_contract_with_address(add)
        };

        let token_eth = self.tokens.resolve("ETH".into()).unwrap();

        let pending: U256 = {
            let query = main_contract.query(
                "getPendingBalance",
                (self.address(), token_eth.address),
                None,
                Options::default(),
                None,
            );

            query.await.unwrap()
        };

        println!("{}", pending);

        Ok(())
    }
}
