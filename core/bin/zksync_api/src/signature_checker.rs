//! `signature_checker` module provides a detached thread routine
//! dedicated for checking the signatures of incoming transactions.
//! Main routine of this module operates a multithreaded event loop,
//! which is used to spawn concurrent tasks to efficiently check the
//! transactions signatures.

// Built-in uses
use std::collections::HashSet;
use std::time::Instant;

// External uses
use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use tokio::task::JoinHandle;

// Workspace uses
use zksync_eth_client::EthereumGateway;
use zksync_storage::StorageProcessor;
use zksync_types::{
    tx::{error::TxAddError, EthBatchSignData, EthSignData, TxEthSignature},
    Address, Order, SignedZkSyncTx, Token, ZkSyncTx,
};
// Local uses
use crate::eth_checker::EthereumChecker;
use zksync_types::tx::TransactionError;

/// `TxVariant` is used to form a verify request. It is possible to wrap
/// either a single transaction, or the transaction batch.
#[derive(Debug, Clone)]
pub enum TxVariant {
    Tx(SignedZkSyncTx),
    Batch(Vec<SignedZkSyncTx>, Option<EthBatchSignData>),
    Order(Box<Order>),
    Toggle2FA,
}

/// Wrapper on a `TxVariant` which guarantees that (a batch of)
/// transaction(s) was checked and signatures associated with
/// this transactions are correct.
///
/// Underlying `TxVariant` is a private field, thus no such
/// object can be created without verification.
#[derive(Debug, Clone)]
pub struct VerifiedTx(TxVariant);

impl VerifiedTx {
    /// Checks the (batch of) transaction(s) correctness by verifying its
    /// Ethereum signature (if required) and `ZKSync` signature.
    pub async fn verify(
        request_data: RequestData,
        eth_checker: &EthereumChecker,
    ) -> Result<Self, TxAddError> {
        verify_eth_signature(&request_data, eth_checker).await?;
        let mut tx_variant = request_data.get_tx_variant();
        verify_tx_correctness(&mut tx_variant).await?;

        Ok(Self(tx_variant))
    }

    /// Creates a verified wrapper without actually verifying the original data.
    #[cfg(test)]
    pub(crate) fn unverified(inner: TxVariant) -> Self {
        Self(inner)
    }

    /// Takes the `TxVariant` out of the wrapper.
    pub fn unwrap_tx(self) -> SignedZkSyncTx {
        match self.0 {
            TxVariant::Tx(tx) => tx,
            TxVariant::Batch(_, _) => panic!("called `unwrap_tx` on a `Batch` value"),
            TxVariant::Order(_) => panic!("called `unwrap_tx` on an `Order` value"),
            TxVariant::Toggle2FA => panic!("called `unwrap_tx` on an `Toggle2FA` value"),
        }
    }

    /// Takes the Vec of `SignedZkSyncTx` and the verified signature data out of the wrapper.
    pub fn unwrap_batch(self) -> (Vec<SignedZkSyncTx>, Option<EthBatchSignData>) {
        match self.0 {
            TxVariant::Batch(txs, batch_sign_data) => (txs, batch_sign_data),
            TxVariant::Tx(_) => panic!("called `unwrap_batch` on a `Tx` value"),
            TxVariant::Order(_) => panic!("called `unwrap_batch` on an `Order` value"),
            TxVariant::Toggle2FA => panic!("called `unwrap_batch` on an `Toggle2FA` value"),
        }
    }
}

/// Verifies the Ethereum signature of the (batch of) transaction(s).
async fn verify_eth_signature(
    request_data: &RequestData,
    eth_checker: &EthereumChecker,
) -> Result<(), TxAddError> {
    match request_data {
        RequestData::Tx(request) => {
            verify_eth_signature_single_tx(&request.tx, request.sender, eth_checker).await?;
        }
        RequestData::Batch(request) => {
            let accounts = &request.senders;
            let tokens = &request.tokens;
            let txs = &request.txs;

            if accounts.len() != request.txs.len() {
                return Err(TxAddError::Other);
            }
            if let Some(batch_sign_data) = &request.batch_sign_data {
                verify_eth_signature_txs_batch(accounts, batch_sign_data, eth_checker).await?;
            }
            // In case there're signatures provided for some of transactions
            // we still verify them.
            for ((tx, &account), _token) in
                txs.iter().zip(accounts.iter()).zip(tokens.iter().cloned())
            {
                verify_eth_signature_single_tx(tx, account, eth_checker).await?;
            }
        }
        RequestData::Order(request) => {
            let signature_correct = verify_ethereum_signature(
                &request.sign_data.signature,
                &request.sign_data.message,
                request.sender,
                eth_checker,
            )
            .await;
            if !signature_correct {
                return Err(TxAddError::IncorrectEthSignature);
            }
        }
        RequestData::Toggle2FA(request) => {
            let signature_correct = verify_ethereum_signature(
                &request.sign_data.signature,
                &request.sign_data.message,
                request.sender,
                eth_checker,
            )
            .await;
            if !signature_correct {
                return Err(TxAddError::IncorrectEthSignature);
            }
        }
    }

    Ok(())
}

/// Given a single Ethereum signature and a message, checks that it
/// was signed by an expected address.
async fn verify_ethereum_signature(
    eth_signature: &TxEthSignature,
    message: &[u8],
    sender_address: Address,
    eth_checker: &EthereumChecker,
) -> bool {
    let signer_account = match eth_signature {
        TxEthSignature::EthereumSignature(packed_signature) => {
            packed_signature.signature_recover_signer(message)
        }
        TxEthSignature::EIP1271Signature(signature) => {
            return eth_checker
                .is_eip1271_signature_correct(sender_address, message, signature.clone())
                .await
                .expect("Unable to check EIP1271 signature")
        }
    };
    match signer_account {
        Ok(address) => address == sender_address,
        Err(_) => false,
    }
}

async fn verify_eth_signature_single_tx(
    tx: &SignedZkSyncTx,
    sender_address: Address,
    eth_checker: &EthereumChecker,
) -> Result<(), TxAddError> {
    let start = Instant::now();
    // Check if the tx is a `ChangePubKey` operation without an Ethereum signature.
    if let ZkSyncTx::ChangePubKey(change_pk) = &tx.tx {
        if change_pk.is_onchain() {
            // Check that user is allowed to perform this operation.
            let is_authorized = eth_checker
                .is_new_pubkey_hash_authorized(
                    change_pk.account,
                    change_pk.nonce,
                    &change_pk.new_pk_hash,
                )
                .await
                .expect("Unable to check onchain ChangePubKey Authorization");

            if !is_authorized {
                return Err(TxAddError::ChangePkNotAuthorized);
            }
        }
    }
    if let ZkSyncTx::ChangeGroup(change_group) = &tx.tx {
        let is_authorized = eth_checker
            .is_user_allowed_to_transfer_to_this_group(change_group.group2, change_group.to)
            .await
            .expect("Unable to check onchain ChangeGroup Authorization");
        if !is_authorized {
            return Err(TxAddError::UserNotWhitelisted);
        }
    }

    // Check the signature.
    if let Some(sign_data) = &tx.eth_sign_data {
        let signature = &sign_data.signature;
        let signature_correct =
            verify_ethereum_signature(signature, &sign_data.message, sender_address, eth_checker)
                .await;
        if !signature_correct {
            return Err(TxAddError::IncorrectEthSignature);
        }
    }

    metrics::histogram!(
        "signature_checker.verify_eth_signature_single_tx",
        start.elapsed()
    );
    Ok(())
}

async fn verify_eth_signature_txs_batch(
    senders: &[Address],
    batch_sign_data: &EthBatchSignData,
    eth_checker: &EthereumChecker,
) -> Result<(), TxAddError> {
    let start = Instant::now();
    // Cache for verified senders.
    let mut signers = HashSet::with_capacity(senders.len());

    for sender in senders {
        if signers.contains(sender) {
            continue;
        }
        // All possible signers are cached already and this sender didn't match any of them.
        if signers.len() == batch_sign_data.signatures.len() {
            return Err(TxAddError::IncorrectEthSignature);
        }
        // This block will set the `sender_correct` variable to `true` at the first match.
        let mut sender_correct = false;
        for signature in &batch_sign_data.signatures {
            let signature_correct = verify_ethereum_signature(
                signature,
                &batch_sign_data.message,
                *sender,
                eth_checker,
            )
            .await;
            if signature_correct {
                signers.insert(sender);
                sender_correct = true;
                break;
            }
        }
        // No signature for this transaction found, return error.
        if !sender_correct {
            return Err(TxAddError::IncorrectEthSignature);
        }
    }
    metrics::histogram!(
        "signature_checker.verify_eth_signature_txs_batch",
        start.elapsed()
    );
    Ok(())
}

/// Verifies the correctness of the ZKSync transaction(s) (including the
/// signature check).
async fn verify_tx_correctness(tx: &mut TxVariant) -> Result<(), TxAddError> {
    match tx {
        TxVariant::Tx(tx) => {
            let sender_account = tx.tx.account();
            let receiver_account = tx.tx.to_account().unwrap();
            let mut storage = StorageProcessor::establish_connection().await.unwrap();
            let sender_autorisation = storage
                .chain()
                .account_schema()
                .is_account_autorised(sender_account)
                .await
                .unwrap();

            let receiver_autorisation = storage
                .chain()
                .account_schema()
                .is_account_autorised(receiver_account)
                .await
                .unwrap();

            tx.tx
                .check_correctness(sender_autorisation, receiver_autorisation)?;
        }
        TxVariant::Batch(batch, _) => {
            let mut storage = StorageProcessor::establish_connection().await.unwrap();
            for tx in batch.iter_mut() {
                let sender_account = tx.tx.account();
                let receiver_account = tx.tx.to_account().unwrap();

                let sender_autorisation = storage
                    .chain()
                    .account_schema()
                    .is_account_autorised(sender_account)
                    .await
                    .unwrap();

                let receiver_autorisation = storage
                    .chain()
                    .account_schema()
                    .is_account_autorised(receiver_account)
                    .await
                    .unwrap();

                tx.tx
                    .check_correctness(sender_autorisation, receiver_autorisation)?;
            }
        }
        TxVariant::Order(order) => order
            .check_correctness()
            .map_err(|err| TxAddError::IncorrectTx(TransactionError::OrderError(err)))?,
        TxVariant::Toggle2FA => {} // There is no data to check correctness of
    }
    Ok(())
}

#[derive(Debug)]
pub struct TxRequest {
    pub tx: SignedZkSyncTx,
    /// Sender of transaction. This field is needed since for `ForcedExit` account affected by
    /// the transaction and actual sender can be different. Thus, we require request sender to
    /// perform a database query and fetch actual addresses if necessary.
    pub sender: Address,
    /// Resolved token might be used to obtain old-formatted 2-FA messages.
    /// Needed for backwards compatibility.
    pub token: Token,
}

#[derive(Debug)]
pub struct BatchRequest {
    pub txs: Vec<SignedZkSyncTx>,
    pub batch_sign_data: Option<EthBatchSignData>,
    pub senders: Vec<Address>,
    pub tokens: Vec<Token>,
}

#[derive(Debug)]
pub struct OrderRequest {
    pub order: Box<Order>,
    pub sign_data: EthSignData,
    pub sender: Address,
}

#[derive(Debug)]
pub struct Toggle2FARequest {
    pub sign_data: EthSignData,
    pub sender: Address,
}

/// Request for the signature check.
#[derive(Debug)]
pub struct VerifySignatureRequest {
    pub data: RequestData,
    /// Channel for sending the check response.
    pub response: oneshot::Sender<Result<VerifiedTx, TxAddError>>,
}

#[derive(Debug)]
pub enum RequestData {
    Tx(TxRequest),
    Batch(BatchRequest),
    Order(OrderRequest),
    Toggle2FA(Toggle2FARequest),
}

impl RequestData {
    pub fn get_tx_variant(&self) -> TxVariant {
        match &self {
            RequestData::Tx(request) => TxVariant::Tx(request.tx.clone()),
            RequestData::Batch(request) => {
                TxVariant::Batch(request.txs.clone(), request.batch_sign_data.clone())
            }
            RequestData::Order(request) => TxVariant::Order(request.order.clone()),
            RequestData::Toggle2FA(_) => TxVariant::Toggle2FA,
        }
    }
}

/// Main routine of the concurrent signature checker.
/// See the module documentation for details.
pub fn start_sign_checker(
    client: EthereumGateway,
    input: mpsc::Receiver<VerifySignatureRequest>,
) -> JoinHandle<()> {
    let eth_checker = EthereumChecker::new(client);

    /// Basically it receives the requests through the channel and verifies signatures,
    /// notifying the request sender about the check result.
    async fn checker_routine(
        mut input: mpsc::Receiver<VerifySignatureRequest>,
        eth_checker: EthereumChecker,
    ) {
        while let Some(VerifySignatureRequest { data, response }) = input.next().await {
            let eth_checker = eth_checker.clone();
            tokio::spawn(async move {
                let resp = VerifiedTx::verify(data, &eth_checker).await;
                response.send(resp).unwrap_or_default();
            });
        }
    }
    tokio::spawn(checker_routine(input, eth_checker))
}
