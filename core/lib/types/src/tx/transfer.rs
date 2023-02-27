use std::fmt::{Display, Formatter};

use num::{BigUint, Zero};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use zksync_basic_types::Address;
use zksync_crypto::{
    franklin_crypto::eddsa::PrivateKey,
    params::{max_account_id, max_processable_token, max_token_id, CURRENT_TX_VERSION},
};
use zksync_utils::{parse_env, BigUintSerdeAsRadix10Str};

use super::{TxSignature, VerifiedSignatureCache};
use crate::{
    helpers::{
        is_fee_amount_packable, is_token_amount_packable, pack_fee_amount, pack_token_amount,
    },
    tx::TimeRange,
    AccountId, Nonce, TokenId,
};

use crate::tx::error::{
    AMOUNT_IS_NOT_PACKABLE, DESTINATION_NOT_WHITELISTED, FEE_AMOUNT_IS_NOT_PACKABLE,
    SENDER_NOT_WHITELISTED, WRONG_ACCOUNT_ID, WRONG_AMOUNT_ERROR, WRONG_FEE_ERROR, WRONG_GROUP,
    WRONG_SIGNATURE, WRONG_TIME_RANGE, WRONG_TOKEN, WRONG_TOKEN_FOR_PAYING_FEE, WRONG_TO_ADDRESS,
};
use crate::tx::version::TxVersion;
use crate::{account::PubKeyHash, utils::ethereum_sign_message_part, Engine};

/// `Transfer` transaction performs a move of funds from one zkSync account to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    /// zkSync network account ID of the transaction initiator.
    pub account_id: AccountId,
    /// Address of account to transfer funds from.
    pub from: Address,
    /// Address of account to transfer funds to.
    pub to: Address,
    /// Type of token for transfer. Also represents the token in which fee will be paid.
    pub token: TokenId,
    /// Amount of funds to transfer.
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub amount: BigUint,
    /// Fee for the transaction.
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub fee: BigUint,
    /// Group id where the transfer occur
    pub group: u16,
    /// Current account nonce.
    pub nonce: Nonce,
    /// Time range when the transaction is valid
    /// This fields must be Option<...> because of backward compatibility with first version of ZkSync
    #[serde(flatten)]
    pub time_range: Option<TimeRange>,
    /// Transaction zkSync signature.
    pub signature: TxSignature,
    #[serde(skip)]
    cached_signer: VerifiedSignatureCache,
}

impl Transfer {
    /// Unique identifier of the transaction type in zkSync network.
    pub const TX_TYPE: u8 = 5;

    /// Creates transaction from all the required fields.
    ///
    /// While `signature` field is mandatory for new transactions, it may be `None`
    /// in some cases (e.g. when restoring the network state from the L1 contract data).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        account_id: AccountId,
        from: Address,
        to: Address,
        token: TokenId,
        amount: BigUint,
        fee: BigUint,
        group: u16,
        nonce: Nonce,
        time_range: TimeRange,
        signature: Option<TxSignature>,
    ) -> Self {
        let mut tx = Self {
            account_id,
            from,
            to,
            token,
            amount,
            fee,
            group,
            nonce,
            time_range: Some(time_range),
            signature: signature.clone().unwrap_or_default(),
            cached_signer: VerifiedSignatureCache::NotCached,
        };
        if signature.is_some() {
            tx.cached_signer = VerifiedSignatureCache::Cached(tx.verify_signature());
        }
        tx
    }

    /// Creates a signed transaction using private key and
    /// checks for the transaction correcteness.
    #[allow(clippy::too_many_arguments)]
    pub fn new_signed(
        account_id: AccountId,
        from: Address,
        to: Address,
        token: TokenId,
        amount: BigUint,
        fee: BigUint,
        group: u16,
        nonce: Nonce,
        time_range: TimeRange,
        private_key: &PrivateKey<Engine>,
    ) -> Result<Self, TransactionError> {
        let mut tx = Self::new(
            account_id, from, to, token, amount, fee, group, nonce, time_range, None,
        );
        tx.signature = TxSignature::sign_musig(private_key, &tx.get_bytes());
        tx.check_correctness(true, true)?;
        Ok(tx)
    }

    pub fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[255u8 - Self::TX_TYPE]);
        out.extend_from_slice(&[CURRENT_TX_VERSION]);
        out.extend_from_slice(&self.account_id.to_be_bytes());
        out.extend_from_slice(self.from.as_bytes());
        out.extend_from_slice(self.to.as_bytes());
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&pack_token_amount(&self.amount));
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.group.to_be_bytes());
        out.extend_from_slice(&self.nonce.to_be_bytes());
        let time_range = self.time_range.unwrap_or_default();
        out.extend_from_slice(&time_range.as_be_bytes());
        out
    }

    /// Restores the `PubKeyHash` from the transaction signature.
    pub fn verify_signature(&self) -> Option<(PubKeyHash, TxVersion)> {
        if let VerifiedSignatureCache::Cached(cached_signer) = &self.cached_signer {
            *cached_signer
        } else {
            self.signature
                .verify_musig(&self.get_bytes())
                .map(|pub_key| (PubKeyHash::from_pubkey(&pub_key), TxVersion::V1))
        }
    }

    /// Get the first part of the message we expect to be signed by Ethereum account key.
    /// The only difference is the missing `nonce` since it's added at the end of the transactions
    /// batch message.
    pub fn get_ethereum_sign_message_part(&self, token_symbol: &str, decimals: u8) -> String {
        ethereum_sign_message_part(
            "Transfer",
            token_symbol,
            decimals,
            &self.amount,
            &self.fee,
            &self.to,
            self.group,
        )
    }

    /// Gets message that should be signed by Ethereum keys of the account for 2-Factor authentication.
    pub fn get_ethereum_sign_message(&self, token_symbol: &str, decimals: u8) -> String {
        let mut message = self.get_ethereum_sign_message_part(token_symbol, decimals);
        if !message.is_empty() {
            message.push('\n');
        }
        message.push_str(format!("Nonce: {}", self.nonce).as_str());
        message
    }

    /// Helper method to remove cache and test transaction behavior without the signature cache.
    #[doc(hidden)]
    pub fn wipe_signer_cache(&mut self) {
        self.cached_signer = VerifiedSignatureCache::NotCached;
    }

    /// Verifies the transaction correctness:
    ///
    /// - `account_id` field must be within supported range.
    /// - `token` field must be within supported range.
    /// - `amount` field must represent a packable value.
    /// - `fee` field must represent a packable value.
    /// - `group` field must be the one assigned to server
    /// - transfer recipient must not be `Adddress::zero()`.
    /// - zkSync signature must correspond to the PubKeyHash of the account.
    pub fn check_correctness(
        &mut self,
        sender_autorisation: bool,
        receiver_autorisation: bool,
    ) -> Result<(), TransactionError> {
        let server_group_id: u16 = parse_env("SERVER_GROUP_ID");
        let permissioned: bool = parse_env("SERVER_PERMISSIONED");
        if permissioned {
            if !sender_autorisation {
                vlog::info!("sender of this transfer transaction is not whitelisted");
                return Err(TransactionError::NotAllowedSender);
            }
            if !receiver_autorisation {
                vlog::info!("receiver of this transfer transaction is not whitelisted");
                return Err(TransactionError::NotAllowedDestination);
            }
        }
        if self.amount > BigUint::from(u128::MAX) {
            return Err(TransactionError::WrongAmount);
        }
        if self.fee > BigUint::from(u128::MAX) {
            return Err(TransactionError::WrongFee);
        }
        if !is_token_amount_packable(&self.amount) {
            return Err(TransactionError::AmountNotPackable);
        }
        if !is_fee_amount_packable(&self.fee) {
            return Err(TransactionError::FeeNotPackable);
        }
        if self.account_id > max_account_id() {
            return Err(TransactionError::WrongAccountId);
        }
        if self.group != server_group_id {
            return Err(TransactionError::WrongGroup);
        }
        if self.token > max_token_id() {
            return Err(TransactionError::WrongToken);
        }
        if self.to == Address::zero() {
            return Err(TransactionError::WrongToAddress);
        }
        if !self
            .time_range
            .map(|r| r.check_correctness())
            .unwrap_or(true)
        {
            return Err(TransactionError::WrongTimeRange);
        }

        // Fee can only be paid in processable tokens
        if self.fee != BigUint::zero() && self.token > max_processable_token() {
            return Err(TransactionError::WrongTokenForPayingFee);
        }

        let signer = self.verify_signature();
        self.cached_signer = VerifiedSignatureCache::Cached(signer);
        if signer.is_none() {
            return Err(TransactionError::WrongSignature);
        }
        Ok(())
    }
}

#[derive(Error, Debug, Copy, Clone, Serialize, Deserialize)]
pub enum TransactionError {
    WrongAmount,
    AmountNotPackable,
    WrongFee,
    FeeNotPackable,
    WrongAccountId,
    WrongToken,
    WrongTimeRange,
    WrongSignature,
    WrongToAddress,
    WrongTokenForPayingFee,
    WrongGroup,
    NotAllowedSender,
    NotAllowedDestination,
}

impl Display for TransactionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let error = match self {
            TransactionError::WrongAmount => WRONG_AMOUNT_ERROR,
            TransactionError::AmountNotPackable => AMOUNT_IS_NOT_PACKABLE,
            TransactionError::WrongFee => WRONG_FEE_ERROR,
            TransactionError::FeeNotPackable => FEE_AMOUNT_IS_NOT_PACKABLE,
            TransactionError::WrongAccountId => WRONG_ACCOUNT_ID,
            TransactionError::WrongToken => WRONG_TOKEN,
            TransactionError::WrongTimeRange => WRONG_TIME_RANGE,
            TransactionError::WrongSignature => WRONG_SIGNATURE,
            TransactionError::WrongTokenForPayingFee => WRONG_TOKEN_FOR_PAYING_FEE,
            TransactionError::WrongToAddress => WRONG_TO_ADDRESS,
            TransactionError::WrongGroup => WRONG_GROUP,
            TransactionError::NotAllowedSender => SENDER_NOT_WHITELISTED,
            TransactionError::NotAllowedDestination => DESTINATION_NOT_WHITELISTED,
        };
        write!(f, "{}", error)
    }
}
