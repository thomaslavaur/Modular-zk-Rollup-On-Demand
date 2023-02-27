// External deps
use num::ToPrimitive;
use zksync_crypto::franklin_crypto::{
    bellman::pairing::{
        bn256::{Bn256, Fr},
        ff::Field,
    },
    rescue::RescueEngine,
};
// Workspace deps
use zksync_crypto::params::GROUP_LEN;
use zksync_crypto::{
    circuit::{
        account::CircuitAccountTree,
        utils::{append_be_fixed_width, eth_address_to_fr, le_bit_vector_into_field_element},
    },
    params::{
        account_tree_depth, ACCOUNT_ID_BIT_WIDTH, AMOUNT_EXPONENT_BIT_WIDTH,
        AMOUNT_MANTISSA_BIT_WIDTH, BALANCE_BIT_WIDTH, CHUNK_BIT_WIDTH, ETH_ADDRESS_BIT_WIDTH,
        FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, TOKEN_BIT_WIDTH, TX_TYPE_BIT_WIDTH,
    },
    primitives::FloatConversions,
};
use zksync_types::operations::ChangeGroupOp;
// Local deps
use crate::{
    operation::{Operation, OperationArguments, OperationBranch, OperationBranchWitness},
    utils::resize_grow_only,
    witness::{
        utils::{apply_leaf_operation, fr_from, get_audits, SigDataInput},
        Witness,
    },
};

pub struct ChangeGroupData {
    pub amount: u128,
    pub fee: u128,
    pub token: u32,
    pub account_address: u32,
    pub eth_address: Fr,
    pub group1: u16,
    pub group2: u16,
    pub valid_from: u64,
    pub valid_until: u64,
}

pub struct ChangeGroupWitness<E: RescueEngine> {
    pub before: OperationBranch<E>,
    pub after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}

impl Witness for ChangeGroupWitness<Bn256> {
    type OperationType = ChangeGroupOp;
    type CalculateOpsInput = SigDataInput;

    fn apply_tx(tree: &mut CircuitAccountTree, change_group: &ChangeGroupOp) -> Self {
        let (valid_from, valid_until) = {
            let time_range = change_group.tx.time_range.unwrap_or_default();
            (time_range.valid_from, time_range.valid_until)
        };
        let change_group_data = ChangeGroupData {
            amount: change_group.tx.amount.to_u128().unwrap(),
            fee: change_group.tx.fee.to_u128().unwrap(),
            token: *change_group.tx.token as u32,
            account_address: *change_group.account_id,
            eth_address: eth_address_to_fr(&change_group.tx.to),
            group1: change_group.tx.group1,
            group2: change_group.tx.group2,
            valid_from,
            valid_until,
        };
        // le_bit_vector_into_field_element()
        Self::apply_data(tree, &change_group_data)
    }

    fn get_pubdata(&self) -> Vec<bool> {
        let mut pubdata_bits = vec![];
        append_be_fixed_width(&mut pubdata_bits, &self.tx_type.unwrap(), TX_TYPE_BIT_WIDTH);

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.address.unwrap(),
            ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.token.unwrap(),
            TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.full_amount.unwrap(),
            BALANCE_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.fee.unwrap(),
            FEE_MANTISSA_BIT_WIDTH + FEE_EXPONENT_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.eth_address.unwrap(),
            ETH_ADDRESS_BIT_WIDTH,
        );

        append_be_fixed_width(&mut pubdata_bits, &self.args.group2.unwrap(), GROUP_LEN * 8);

        resize_grow_only(
            &mut pubdata_bits,
            ChangeGroupOp::CHUNKS * CHUNK_BIT_WIDTH,
            false,
        );
        pubdata_bits
    }

    fn get_offset_commitment_data(&self) -> Vec<bool> {
        let mut commitment = vec![false; ChangeGroupOp::CHUNKS * 8];
        commitment[7] = true;
        commitment
    }

    fn calculate_operations(&self, input: SigDataInput) -> Vec<Operation<Bn256>> {
        let pubdata_chunks: Vec<_> = self
            .get_pubdata()
            .chunks(CHUNK_BIT_WIDTH)
            .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
            .collect();

        let operation_zero = Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(fr_from(0)),
            pubdata_chunk: Some(pubdata_chunks[0]),
            first_sig_msg: Some(input.first_sig_msg),
            second_sig_msg: Some(input.second_sig_msg),
            third_sig_msg: Some(input.third_sig_msg),
            signature_data: input.signature.clone(),
            signer_pub_key_packed: input.signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.before.clone(),
            rhs: self.before.clone(),
        };

        let rest_operations = (1..ChangeGroupOp::CHUNKS).map(|chunk| Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(fr_from(chunk)),
            pubdata_chunk: Some(pubdata_chunks[chunk]),
            first_sig_msg: Some(input.first_sig_msg),
            second_sig_msg: Some(input.second_sig_msg),
            third_sig_msg: Some(input.third_sig_msg),
            signature_data: input.signature.clone(),
            signer_pub_key_packed: input.signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.after.clone(),
            rhs: self.after.clone(),
        });
        std::iter::once(operation_zero)
            .chain(rest_operations)
            .collect()
    }
}

impl ChangeGroupWitness<Bn256> {
    fn apply_data(tree: &mut CircuitAccountTree, change_group: &ChangeGroupData) -> Self {
        //preparing data and base witness
        let before_root = tree.root_hash();
        vlog::debug!("Initial root = {}", before_root);
        let (audit_path_before, audit_balance_path_before) =
            get_audits(tree, change_group.account_address, change_group.token);

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << account_tree_depth());
        let account_address_fe = fr_from(change_group.account_address);
        let token_fe = fr_from(change_group.token);
        let amount_as_field_element = fr_from(change_group.amount);
        let group1 = fr_from(change_group.group1);
        let group2 = fr_from(change_group.group2);
        let amount_bits = FloatConversions::to_float(
            change_group.amount,
            AMOUNT_EXPONENT_BIT_WIDTH,
            AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let amount_encoded: Fr = le_bit_vector_into_field_element(&amount_bits);

        let fee_as_field_element = fr_from(change_group.fee);

        let fee_bits = FloatConversions::to_float(
            change_group.fee,
            FEE_EXPONENT_BIT_WIDTH,
            FEE_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

        //calculate a and b

        //applying change group

        let (account_witness_before, account_witness_after, balance_before, balance_after) =
            apply_leaf_operation(
                tree,
                change_group.account_address,
                change_group.token,
                |acc| {
                    acc.nonce.add_assign(&fr_from(1));
                },
                |bal| {
                    bal.value.sub_assign(&amount_as_field_element);
                    bal.value.sub_assign(&fee_as_field_element);
                },
            );

        let after_root = tree.root_hash();
        vlog::debug!("After root = {}", after_root);
        let (audit_path_after, audit_balance_path_after) =
            get_audits(tree, change_group.account_address, change_group.token);

        let a = balance_before;
        let mut b = amount_as_field_element;
        b.add_assign(&fee_as_field_element);

        ChangeGroupWitness {
            before: OperationBranch {
                address: Some(account_address_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_before,
                    account_path: audit_path_before,
                    balance_value: Some(balance_before),
                    balance_subtree_path: audit_balance_path_before,
                },
            },
            after: OperationBranch {
                address: Some(account_address_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_after,
                    account_path: audit_path_after,
                    balance_value: Some(balance_after),
                    balance_subtree_path: audit_balance_path_after,
                },
            },
            args: OperationArguments {
                eth_address: Some(change_group.eth_address),
                amount_packed: Some(amount_encoded),
                full_amount: Some(amount_as_field_element),
                fee: Some(fee_encoded),
                a: Some(a),
                b: Some(b),
                group: Some(group1),
                group2: Some(group2),
                valid_from: Some(fr_from(change_group.valid_from)),
                valid_until: Some(fr_from(change_group.valid_until)),
                ..Default::default()
            },
            before_root: Some(before_root),
            after_root: Some(after_root),
            tx_type: Some(fr_from(ChangeGroupOp::OP_CODE)),
        }
    }
}
