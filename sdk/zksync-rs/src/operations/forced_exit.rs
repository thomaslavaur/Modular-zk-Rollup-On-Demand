use num::BigUint;
use zksync_eth_signer::EthereumSigner;
use zksync_types::{
    helpers::{
        closest_packable_fee_amount, closest_packable_token_amount, is_fee_amount_packable,
        is_token_amount_packable,
    },
    tx::{PackedEthSignature, TimeRange},
    Address, Nonce, Token, TokenLike, TxFeeTypes, ZkSyncTx,
};

use crate::{
    error::ClientError, operations::SyncTransactionHandle, provider::Provider, wallet::Wallet,
};

#[derive(Debug)]
pub struct ForcedExitBuilder<'a, S: EthereumSigner, P: Provider> {
    wallet: &'a Wallet<S, P>,
    target: Option<Address>,
    token: Option<Token>,
    fee: Option<BigUint>,
    group: Option<u16>,
    nonce: Option<Nonce>,
    valid_from: Option<u64>,
    valid_until: Option<u64>,
}

impl<'a, S, P> ForcedExitBuilder<'a, S, P>
where
    S: EthereumSigner,
    P: Provider + Clone,
{
    /// Initializes a forcedexit transaction building process.
    pub fn new(wallet: &'a Wallet<S, P>) -> Self {
        Self {
            wallet,
            token: None,
            target: None,
            fee: None,
            group: None,
            nonce: None,
            valid_from: None,
            valid_until: None,
        }
    }

    /// Directly returns the signed forcedexit transaction for the subsequent usage.
    pub async fn tx(self) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        let token = self
            .token
            .ok_or_else(|| ClientError::MissingRequiredField("token".into()))?;
        let target = self
            .target
            .ok_or_else(|| ClientError::MissingRequiredField("target".into()))?;
        let group = self
            .group
            .ok_or_else(|| ClientError::MissingRequiredField("group".into()))?;

        let nonce = match self.nonce {
            Some(nonce) => nonce,
            None => {
                let account_info = self
                    .wallet
                    .provider
                    .account_info(self.wallet.address())
                    .await?;
                account_info.committed.nonce
            }
        };

        let fee = match self.fee {
            Some(fee) => fee,
            None => {
                let fee = self
                    .wallet
                    .provider
                    .get_tx_fee(TxFeeTypes::Withdraw, target, token.id)
                    .await?;
                fee.total_fee
            }
        };

        let valid_from = self.valid_from.unwrap_or(0);
        let valid_until = self.valid_until.unwrap_or(u64::MAX);

        self.wallet
            .signer
            .sign_forced_exit(
                target,
                token,
                fee,
                group,
                nonce,
                TimeRange::new(valid_from, valid_until),
            )
            .await
            .map(|(tx, sign)| (ZkSyncTx::ForcedExit(Box::new(tx)), sign))
            .map_err(ClientError::SigningError)
    }

    /// Sends the transaction, returning the handle for its awaiting.
    pub async fn send(self) -> Result<SyncTransactionHandle<P>, ClientError> {
        let provider = self.wallet.provider.clone();

        let (tx, eth_signature) = self.tx().await?;
        let tx_hash = provider.send_tx(tx, eth_signature).await?;

        Ok(SyncTransactionHandle::new(tx_hash, provider))
    }

    /// Sets the transaction token. Returns an error if token is not supported by zkSync.
    pub fn token(mut self, token: impl Into<TokenLike>) -> Result<Self, ClientError> {
        let token_like = token.into();
        let token = self
            .wallet
            .tokens
            .resolve(token_like)
            .ok_or(ClientError::UnknownToken)?;

        self.token = Some(token);

        Ok(self)
    }

    /// Set the fee amount. If the amount provided is not packable,
    /// rounds it to the closest packable fee amount.
    ///
    /// For more details, see [utils](../utils/index.html) functions.
    pub fn fee(mut self, fee: impl Into<BigUint>) -> Self {
        let fee = closest_packable_fee_amount(&fee.into());
        self.fee = Some(fee);

        self
    }

    /// Set the fee amount. If the provided fee is not packable,
    /// returns an error.
    ///
    /// For more details, see [utils](../utils/index.html) functions.
    pub fn fee_exact(mut self, fee: impl Into<BigUint>) -> Result<Self, ClientError> {
        let fee = fee.into();
        if !is_fee_amount_packable(&fee) {
            return Err(ClientError::NotPackableValue);
        }
        self.fee = Some(fee);

        Ok(self)
    }

    /// Sets the address of Ethereum wallet to forcedexit funds to.
    pub fn target(mut self, to: Address) -> Self {
        self.target = Some(to);
        self
    }

    /// Same as `ForcedExitBuilder::to`, but accepts a string address value.
    ///
    /// Provided string value must be a correct address in a hexadecimal form,
    /// otherwise an error will be returned.
    pub fn str_to(mut self, to: impl AsRef<str>) -> Result<Self, ClientError> {
        let to: Address = to
            .as_ref()
            .parse()
            .map_err(|_| ClientError::IncorrectAddress)?;

        self.target = Some(to);
        Ok(self)
    }

    /// Sets the group of the withdraw.
    pub fn group(mut self, group: u16) -> Self {
        self.group = Some(group);
        self
    }

    /// Sets the transaction nonce.
    pub fn nonce(mut self, nonce: Nonce) -> Self {
        self.nonce = Some(nonce);
        self
    }

    /// Sets the unix format timestamp of the first moment when transaction execution is valid.
    pub fn valid_from(mut self, valid_from: u64) -> Self {
        self.valid_from = Some(valid_from);
        self
    }

    /// Sets the unix format timestamp of the last moment when transaction execution is valid.
    pub fn valid_until(mut self, valid_until: u64) -> Self {
        self.valid_until = Some(valid_until);
        self
    }
}
