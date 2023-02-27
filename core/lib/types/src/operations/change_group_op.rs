use crate::{
    helpers::{pack_fee_amount, unpack_fee_amount},
    operations::error::ChangeGroupOpError,
    AccountId, Address, ChangeGroup, Nonce, TokenId,
};
use num::{BigUint, FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};
use zksync_crypto::params::GROUP_LEN;
use zksync_crypto::{
    params::{
        ACCOUNT_ID_BIT_WIDTH, BALANCE_BIT_WIDTH, CHUNK_BYTES, ETH_ADDRESS_BIT_WIDTH,
        FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, LEGACY_CHUNK_BYTES, LEGACY_TOKEN_BIT_WIDTH,
        TOKEN_BIT_WIDTH,
    },
    primitives::FromBytes,
};

/// ChangeGroup operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeGroupOp {
    pub tx: ChangeGroup,
    pub account_id: AccountId,
}

impl ChangeGroupOp {
    pub const CHUNKS: usize = 6;
    pub const OP_CODE: u8 = 0x0c;
    pub const WITHDRAW_DATA_PREFIX: [u8; 1] = [1];

    pub(crate) fn get_public_data(&self) -> Vec<u8> {
        let mut data = vec![Self::OP_CODE];
        data.extend_from_slice(&self.account_id.to_be_bytes());
        data.extend_from_slice(&self.tx.token.to_be_bytes());
        data.extend_from_slice(&self.tx.amount.to_u128().unwrap().to_be_bytes());
        data.extend_from_slice(&pack_fee_amount(&self.tx.fee));
        data.extend_from_slice(self.tx.to.as_bytes());
        data.extend_from_slice(&self.tx.group2.to_be_bytes());
        data.resize(Self::CHUNKS * CHUNK_BYTES, 0x00);
        data
    }

    pub(crate) fn get_withdrawal_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&Self::WITHDRAW_DATA_PREFIX); // first byte is a bool variable 'addToPendingWithdrawalsQueue'
        data.extend_from_slice(self.tx.to.as_bytes());
        data.extend_from_slice(&self.tx.token.to_be_bytes());
        data.extend_from_slice(&self.tx.amount.to_u128().unwrap().to_be_bytes());
        data.extend_from_slice(&self.tx.group1.to_be_bytes());
        data.extend_from_slice(&self.tx.group2.to_be_bytes());
        data
    }

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, ChangeGroupOpError> {
        Self::parse_pub_data(bytes, TOKEN_BIT_WIDTH, CHUNK_BYTES)
    }

    pub fn from_legacy_public_data(bytes: &[u8]) -> Result<Self, ChangeGroupOpError> {
        Self::parse_pub_data(bytes, LEGACY_TOKEN_BIT_WIDTH, LEGACY_CHUNK_BYTES)
    }

    fn parse_pub_data(
        bytes: &[u8],
        token_bit_width: usize,
        chunk_bytes: usize,
    ) -> Result<Self, ChangeGroupOpError> {
        if bytes.len() != Self::CHUNKS * chunk_bytes {
            return Err(ChangeGroupOpError::PubdataSizeMismatch);
        }

        let account_offset = 1;
        let token_id_offset = account_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let amount_offset = token_id_offset + token_bit_width / 8;
        let fee_offset = amount_offset + BALANCE_BIT_WIDTH / 8;
        let eth_address_offset = fee_offset + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8;
        let group_offset = eth_address_offset + ETH_ADDRESS_BIT_WIDTH / 8;

        let account_id =
            u32::from_bytes(&bytes[account_offset..account_offset + ACCOUNT_ID_BIT_WIDTH / 8])
                .ok_or(ChangeGroupOpError::CannotGetAccountId)?;
        let from = Address::zero(); // From pubdata it is unknown
        let token = u32::from_bytes(&bytes[token_id_offset..token_id_offset + token_bit_width / 8])
            .ok_or(ChangeGroupOpError::CannotGetTokenId)?;
        let to = Address::from_slice(
            &bytes[eth_address_offset..eth_address_offset + ETH_ADDRESS_BIT_WIDTH / 8],
        );
        let amount = BigUint::from_u128(
            u128::from_bytes(&bytes[amount_offset..amount_offset + BALANCE_BIT_WIDTH / 8])
                .ok_or(ChangeGroupOpError::CannotGetAmount)?,
        )
        .unwrap();
        let fee = unpack_fee_amount(
            &bytes[fee_offset..fee_offset + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8],
        )
        .ok_or(ChangeGroupOpError::CannotGetFee)?;
        let group2 = u16::from_bytes(&bytes[group_offset..group_offset + GROUP_LEN])
            .ok_or(ChangeGroupOpError::CannotGetGroupId)?;
        let nonce = 0; // From pubdata it is unknown
        let time_range = Default::default();

        Ok(Self {
            tx: ChangeGroup::new(
                AccountId(account_id),
                from,
                to,
                TokenId(token),
                amount,
                fee,
                Default::default(),
                group2,
                Nonce(nonce),
                time_range,
                None,
            ),
            account_id: AccountId(account_id),
        })
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        vec![self.account_id]
    }
}
