use std::time::Instant;
use zksync_crypto::params::{self, max_account_id};
use zksync_types::{
    AccountUpdate, AccountUpdates, ChangeGroup, ChangeGroupOp, PubKeyHash, ZkSyncOp,
};

use crate::{
    handler::{error::ChangeGroupOpError, TxHandler},
    state::{CollectedFee, OpSuccess, ZkSyncState},
};
use num::{BigUint, Zero};

impl TxHandler<ChangeGroup> for ZkSyncState {
    type Op = ChangeGroupOp;

    type OpError = ChangeGroupOpError;

    fn create_op(&self, tx: ChangeGroup) -> Result<Self::Op, ChangeGroupOpError> {
        invariant!(
            tx.token <= params::max_fungible_token_id(),
            ChangeGroupOpError::InvalidTokenId
        );
        if tx.fee != BigUint::zero() {
            // Fee can only be paid in processable tokens
            invariant!(
                tx.token <= params::max_processable_token(),
                ChangeGroupOpError::InvalidFeeTokenId
            );
        }

        let (account_id, account) = self
            .get_account_by_address(&tx.from)
            .ok_or(ChangeGroupOpError::FromAccountNotFound)?;
        invariant!(
            account.pub_key_hash != PubKeyHash::default(),
            ChangeGroupOpError::FromAccountLocked
        );

        if let Some((pub_key_hash, _)) = tx.verify_signature() {
            if pub_key_hash != account.pub_key_hash {
                return Err(ChangeGroupOpError::InvalidSignature);
            }
        }
        invariant!(
            account_id == tx.account_id,
            ChangeGroupOpError::FromAccountIncorrect
        );
        let change_group_op = ChangeGroupOp { tx, account_id };

        Ok(change_group_op)
    }

    fn apply_tx(&mut self, tx: ChangeGroup) -> Result<OpSuccess, ChangeGroupOpError> {
        let op = self.create_op(tx)?;

        let (fee, updates) = <Self as TxHandler<ChangeGroup>>::apply_op(self, &op)?;
        Ok(OpSuccess {
            fee,
            updates,
            executed_op: ZkSyncOp::ChangeGroup(Box::new(op)),
        })
    }

    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), ChangeGroupOpError> {
        let start = Instant::now();
        invariant!(
            op.account_id <= max_account_id(),
            ChangeGroupOpError::FromAccountIncorrect
        );

        let mut updates = Vec::new();
        let mut from_account = self.get_account(op.account_id).unwrap();

        let from_old_balance = from_account.get_balance(op.tx.token);
        let from_old_nonce = from_account.nonce;
        invariant!(
            op.tx.nonce == from_old_nonce,
            ChangeGroupOpError::NonceMismatch
        );
        invariant!(
            from_old_balance >= &op.tx.amount + &op.tx.fee,
            ChangeGroupOpError::InsufficientBalance
        );

        from_account.sub_balance(op.tx.token, &(&op.tx.amount + &op.tx.fee));
        *from_account.nonce += 1;

        let from_new_balance = from_account.get_balance(op.tx.token);
        let from_new_nonce = from_account.nonce;

        self.insert_account(op.account_id, from_account);

        updates.push((
            op.account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.token, from_old_balance, from_new_balance),
                old_nonce: from_old_nonce,
                new_nonce: from_new_nonce,
            },
        ));

        let fee = CollectedFee {
            token: op.tx.token,
            amount: op.tx.fee.clone(),
        };

        metrics::histogram!("state.change_group", start.elapsed());
        Ok((Some(fee), updates))
    }
}
