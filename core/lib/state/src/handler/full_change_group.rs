use num::BigUint;
use std::convert::Infallible;
use std::time::Instant;

use zksync_crypto::params;
use zksync_types::{AccountUpdate, AccountUpdates, FullChangeGroup, FullChangeGroupOp, ZkSyncOp};
use zksync_utils::BigUintSerdeWrapper;

use crate::{
    handler::TxHandler,
    state::{CollectedFee, OpSuccess, ZkSyncState},
};

impl TxHandler<FullChangeGroup> for ZkSyncState {
    type Op = FullChangeGroupOp;

    type OpError = Infallible;

    fn create_op(&self, priority_op: FullChangeGroup) -> Result<Self::Op, Infallible> {
        // NOTE: Authorization of the FullChangeGroup is verified on the contract.
        assert!(
            priority_op.token <= params::max_token_id(),
            "Full change group token is out of range, this should be enforced by contract"
        );
        let account_balance = self
            .get_account(priority_op.account_id)
            .filter(|account| account.address == priority_op.eth_address)
            .map(|acccount| acccount.get_balance(priority_op.token))
            .map(BigUintSerdeWrapper);

        let op = if priority_op.token > params::max_fungible_token_id()
            && self.nfts.get(&priority_op.token).is_some()
        {
            let nft = self.nfts.get(&priority_op.token).unwrap();
            FullChangeGroupOp {
                priority_op,
                withdraw_amount: account_balance,
                creator_account_id: Some(nft.creator_id),
                creator_address: Some(nft.creator_address),
                serial_id: Some(nft.serial_id),
                content_hash: Some(nft.content_hash),
            }
        } else {
            FullChangeGroupOp {
                priority_op,
                withdraw_amount: account_balance,
                creator_account_id: None,
                creator_address: None,
                serial_id: None,
                content_hash: None,
            }
        };

        Ok(op)
    }

    fn apply_tx(&mut self, priority_op: FullChangeGroup) -> Result<OpSuccess, Infallible> {
        let op = self.create_op(priority_op)?;

        let (fee, updates) = <Self as TxHandler<FullChangeGroup>>::apply_op(self, &op)?;
        let result = OpSuccess {
            fee,
            updates,
            executed_op: ZkSyncOp::FullChangeGroup(Box::new(op)),
        };

        Ok(result)
    }

    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), Infallible> {
        let start = Instant::now();
        let mut updates = Vec::new();
        let amount = if let Some(amount) = &op.withdraw_amount {
            amount.clone()
        } else {
            return Ok((None, updates));
        };

        let account_id = op.priority_op.account_id;

        // expect is ok since account's existence was verified before
        let mut account = self
            .get_account(account_id)
            .expect("Full exit account not found");

        let old_balance = account.get_balance(op.priority_op.token);
        let old_nonce = account.nonce;

        account.sub_balance(op.priority_op.token, &amount.0);

        let new_balance = account.get_balance(op.priority_op.token);
        assert_eq!(
            new_balance,
            BigUint::from(0u32),
            "Full change group amount is incorrect"
        );
        let new_nonce = account.nonce;

        self.insert_account(account_id, account);
        updates.push((
            account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (op.priority_op.token, old_balance, new_balance),
                old_nonce,
                new_nonce,
            },
        ));

        let fee = None;

        metrics::histogram!("state.full_change_group", start.elapsed());
        Ok((fee, updates))
    }
}
