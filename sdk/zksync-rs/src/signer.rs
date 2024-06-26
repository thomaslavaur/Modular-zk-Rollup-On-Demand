// Built-in imports
use std::fmt;
// External uses
use num::BigUint;
// Workspace uses
use zksync_crypto::PrivateKey;
use zksync_eth_signer::{error::SignerError, EthereumSigner};
use zksync_types::{
    tx::{
        ChangePubKey, ChangePubKeyECDSAData, ChangePubKeyEthAuthData, PackedEthSignature,
        TimeRange, TxEthSignature,
    },
    AccountId, Address, ChangeGroup, ForcedExit, MintNFT, Nonce, Order, PubKeyHash, Swap, Token,
    TokenId, Transfer, Withdraw, WithdrawNFT, H256,
};
// Local imports
use crate::WalletCredentials;

fn signing_failed_error(err: impl ToString) -> SignerError {
    SignerError::SigningFailed(err.to_string())
}

pub struct Signer<S: EthereumSigner> {
    pub pubkey_hash: PubKeyHash,
    pub address: Address,
    pub(crate) private_key: PrivateKey,
    pub(crate) eth_signer: Option<S>,
    pub(crate) account_id: Option<AccountId>,
}

impl<S: EthereumSigner> fmt::Debug for Signer<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut pk_contents = Vec::new();
        self.private_key
            .write(&mut pk_contents)
            .expect("Failed writing the private key contents");
        f.debug_struct("Signer")
            .field("pubkey_hash", &self.pubkey_hash)
            .field("address", &self.address)
            .finish()
    }
}

impl<S: EthereumSigner> Signer<S> {
    pub fn new(private_key: PrivateKey, address: Address, eth_signer: Option<S>) -> Self {
        let pubkey_hash = PubKeyHash::from_privkey(&private_key);

        Self {
            private_key,
            pubkey_hash,
            address,
            eth_signer,
            account_id: None,
        }
    }

    /// Construct a `Signer` with the given credentials
    pub fn with_credentials(credentials: WalletCredentials<S>) -> Self {
        Self::new(
            credentials.zksync_private_key,
            credentials.eth_address,
            credentials.eth_signer,
        )
    }

    pub fn pubkey_hash(&self) -> &PubKeyHash {
        &self.pubkey_hash
    }

    pub fn set_account_id(&mut self, account_id: Option<AccountId>) {
        self.account_id = account_id;
    }

    pub fn get_account_id(&self) -> Option<AccountId> {
        self.account_id
    }

    pub async fn sign_change_pubkey_tx(
        &self,
        nonce: Nonce,
        auth_onchain: bool,
        fee_token: Token,
        fee: BigUint,
        group: u16,
        time_range: TimeRange,
    ) -> Result<ChangePubKey, SignerError> {
        let account_id = self.account_id.ok_or(SignerError::NoSigningKey)?;

        let mut change_pubkey = ChangePubKey::new_signed(
            account_id,
            self.address,
            self.pubkey_hash,
            fee_token.id,
            fee,
            group,
            nonce,
            time_range,
            None,
            &self.private_key,
        )
        .map_err(signing_failed_error)?;

        let eth_auth_data = if auth_onchain {
            ChangePubKeyEthAuthData::Onchain
        } else {
            let eth_signer = self
                .eth_signer
                .as_ref()
                .ok_or(SignerError::MissingEthSigner)?;

            let sign_bytes = change_pubkey
                .get_eth_signed_data()
                .map_err(signing_failed_error)?;
            let eth_signature = eth_signer
                .sign_message(&sign_bytes)
                .await
                .map_err(signing_failed_error)?;

            let eth_signature = match eth_signature {
                TxEthSignature::EthereumSignature(packed_signature) => Ok(packed_signature),
                TxEthSignature::EIP1271Signature(..) => Err(SignerError::CustomError(
                    "Can't sign ChangePubKey message with EIP1271 signer".to_string(),
                )),
            }?;

            ChangePubKeyEthAuthData::ECDSA(ChangePubKeyECDSAData {
                eth_signature,
                batch_hash: H256::zero(),
            })
        };
        change_pubkey.eth_auth_data = Some(eth_auth_data);

        assert!(
            change_pubkey.is_eth_auth_data_valid(),
            "eth auth data is incorrect"
        );

        Ok(change_pubkey)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn sign_order(
        &self,
        recipient: Address,
        group: u16,
        nonce: Nonce,
        token_sell: Token,
        token_buy: Token,
        prices: (BigUint, BigUint),
        amount: BigUint,
        time_range: TimeRange,
    ) -> Result<(Order, Option<PackedEthSignature>), SignerError> {
        let account_id = self.account_id.ok_or(SignerError::NoSigningKey)?;

        let order = Order::new_signed(
            account_id,
            recipient,
            group,
            nonce,
            token_sell.id,
            token_buy.id,
            prices,
            amount,
            time_range,
            &self.private_key,
        )
        .map_err(signing_failed_error)?;

        let eth_signature = match &self.eth_signer {
            Some(signer) => {
                let message = order.get_ethereum_sign_message(
                    &token_sell.symbol,
                    &token_buy.symbol,
                    token_sell.decimals,
                );
                let signature = signer.sign_message(message.as_bytes()).await?;

                if let TxEthSignature::EthereumSignature(packed_signature) = signature {
                    Some(packed_signature)
                } else {
                    return Err(SignerError::MissingEthSigner);
                }
            }
            _ => None,
        };

        Ok((order, eth_signature))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn sign_swap(
        &self,
        group: u16,
        nonce: Nonce,
        orders: (Order, Order),
        amounts: (BigUint, BigUint),
        fee: BigUint,
        fee_token: Token,
    ) -> Result<(Swap, Option<PackedEthSignature>), SignerError> {
        let account_id = self.account_id.ok_or(SignerError::NoSigningKey)?;

        let swap = Swap::new_signed(
            account_id,
            self.address,
            group,
            nonce,
            orders,
            amounts,
            fee,
            fee_token.id,
            &self.private_key,
        )
        .map_err(signing_failed_error)?;

        let eth_signature = match &self.eth_signer {
            Some(signer) => {
                let message = swap.get_ethereum_sign_message(&fee_token.symbol, fee_token.decimals);
                let signature = signer.sign_message(message.as_bytes()).await?;

                if let TxEthSignature::EthereumSignature(packed_signature) = signature {
                    Some(packed_signature)
                } else {
                    return Err(SignerError::MissingEthSigner);
                }
            }
            _ => None,
        };

        Ok((swap, eth_signature))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn sign_transfer(
        &self,
        token: Token,
        amount: BigUint,
        fee: BigUint,
        to: Address,
        group: u16,
        nonce: Nonce,
        time_range: TimeRange,
    ) -> Result<(Transfer, Option<PackedEthSignature>), SignerError> {
        let account_id = self.account_id.ok_or(SignerError::NoSigningKey)?;

        let transfer = Transfer::new_signed(
            account_id,
            self.address,
            to,
            token.id,
            amount,
            fee,
            group,
            nonce,
            time_range,
            &self.private_key,
        )
        .map_err(signing_failed_error)?;

        let eth_signature = match &self.eth_signer {
            Some(signer) => {
                let message = transfer.get_ethereum_sign_message(&token.symbol, token.decimals);
                let signature = signer.sign_message(message.as_bytes()).await?;

                if let TxEthSignature::EthereumSignature(packed_signature) = signature {
                    Some(packed_signature)
                } else {
                    return Err(SignerError::MissingEthSigner);
                }
            }
            _ => None,
        };

        Ok((transfer, eth_signature))
    }

    pub async fn sign_withdraw(
        &self,
        token: Token,
        amount: BigUint,
        fee: BigUint,
        eth_address: Address,
        group: u16,
        nonce: Nonce,
        time_range: TimeRange,
    ) -> Result<(Withdraw, Option<PackedEthSignature>), SignerError> {
        let account_id = self.account_id.ok_or(SignerError::NoSigningKey)?;

        let withdraw = Withdraw::new_signed(
            account_id,
            self.address,
            eth_address,
            token.id,
            amount,
            fee,
            group,
            nonce,
            time_range,
            &self.private_key,
        )
        .map_err(signing_failed_error)?;

        let eth_signature = match &self.eth_signer {
            Some(signer) => {
                let message = withdraw.get_ethereum_sign_message(&token.symbol, token.decimals);
                let signature = signer.sign_message(message.as_bytes()).await?;

                if let TxEthSignature::EthereumSignature(packed_signature) = signature {
                    Some(packed_signature)
                } else {
                    return Err(SignerError::MissingEthSigner);
                }
            }
            _ => None,
        };

        Ok((withdraw, eth_signature))
    }

    pub async fn sign_forced_exit(
        &self,
        target: Address,
        token: Token,
        fee: BigUint,
        group: u16,
        nonce: Nonce,
        time_range: TimeRange,
    ) -> Result<(ForcedExit, Option<PackedEthSignature>), SignerError> {
        let account_id = self.account_id.ok_or(SignerError::NoSigningKey)?;

        let forced_exit = ForcedExit::new_signed(
            account_id,
            target,
            token.id,
            fee,
            group,
            nonce,
            time_range,
            &self.private_key,
        )
        .map_err(signing_failed_error)?;

        let eth_signature = match &self.eth_signer {
            Some(signer) => {
                let message = forced_exit.get_ethereum_sign_message(&token.symbol, token.decimals);
                let signature = signer.sign_message(message.as_bytes()).await?;

                if let TxEthSignature::EthereumSignature(packed_signature) = signature {
                    Some(packed_signature)
                } else {
                    return Err(SignerError::MissingEthSigner);
                }
            }
            _ => None,
        };

        Ok((forced_exit, eth_signature))
    }

    pub async fn sign_mint_nft(
        &self,
        recipient: Address,
        content_hash: H256,
        fee_token: Token,
        fee: BigUint,
        group: u16,
        nonce: Nonce,
    ) -> Result<(MintNFT, Option<PackedEthSignature>), SignerError> {
        let account_id = self.account_id.ok_or(SignerError::NoSigningKey)?;

        let mint_nft = MintNFT::new_signed(
            account_id,
            self.address,
            content_hash,
            recipient,
            fee,
            fee_token.id,
            group,
            nonce,
            &self.private_key,
        )
        .map_err(signing_failed_error)?;

        let eth_signature = match &self.eth_signer {
            Some(signer) => {
                let message =
                    mint_nft.get_ethereum_sign_message(&fee_token.symbol, fee_token.decimals);
                let signature = signer.sign_message(message.as_bytes()).await?;

                if let TxEthSignature::EthereumSignature(packed_signature) = signature {
                    Some(packed_signature)
                } else {
                    return Err(SignerError::MissingEthSigner);
                }
            }
            _ => None,
        };

        Ok((mint_nft, eth_signature))
    }

    pub async fn sign_withdraw_nft(
        &self,
        to: Address,
        token: TokenId,
        fee_token: Token,
        fee: BigUint,
        group: u16,
        nonce: Nonce,
        time_range: TimeRange,
    ) -> Result<(WithdrawNFT, Option<PackedEthSignature>), SignerError> {
        let account_id = self.account_id.ok_or(SignerError::NoSigningKey)?;

        let withdraw_nft = WithdrawNFT::new_signed(
            account_id,
            self.address,
            to,
            token,
            fee_token.id,
            fee,
            group,
            nonce,
            time_range,
            &self.private_key,
        )
        .map_err(signing_failed_error)?;

        let eth_signature = match &self.eth_signer {
            Some(signer) => {
                let message =
                    withdraw_nft.get_ethereum_sign_message(&fee_token.symbol, fee_token.decimals);
                let signature = signer.sign_message(message.as_bytes()).await?;

                if let TxEthSignature::EthereumSignature(packed_signature) = signature {
                    Some(packed_signature)
                } else {
                    return Err(SignerError::MissingEthSigner);
                }
            }
            _ => None,
        };

        Ok((withdraw_nft, eth_signature))
    }

    pub async fn sign_change_group(
        &self,
        token: Token,
        amount: BigUint,
        fee: BigUint,
        eth_address: Address,
        group1: u16,
        group2: u16,
        nonce: Nonce,
        time_range: TimeRange,
    ) -> Result<(ChangeGroup, Option<PackedEthSignature>), SignerError> {
        let account_id = self.account_id.ok_or(SignerError::NoSigningKey)?;

        let change_group = ChangeGroup::new_signed(
            account_id,
            self.address,
            eth_address,
            token.id,
            amount,
            fee,
            group1,
            group2,
            nonce,
            time_range,
            &self.private_key,
        )
        .map_err(signing_failed_error)?;

        let eth_signature = match &self.eth_signer {
            Some(signer) => {
                let message = change_group.get_ethereum_sign_message(&token.symbol, token.decimals);
                let signature = signer.sign_message(message.as_bytes()).await?;

                if let TxEthSignature::EthereumSignature(packed_signature) = signature {
                    Some(packed_signature)
                } else {
                    return Err(SignerError::MissingEthSigner);
                }
            }
            _ => None,
        };

        Ok((change_group, eth_signature))
    }
}
