use blockifier::execution::contract_class::ClassInfo;
use blockifier::transaction::transaction_execution as btx;
use dc_db::storage_handler::StorageView;
use dp_class::to_blockifier_class;
use dp_convert::ToFelt;
use jsonrpsee::core::RpcResult;
use starknet_api::transaction::{Transaction, TransactionHash};

use crate::errors::StarknetRpcApiError;
use crate::Starknet;

/// Convert an starknet-api Transaction to a blockifier Transaction
///
/// **note:** this function does not support deploy transaction
/// because it is not supported by blockifier
pub(crate) fn to_blockifier_transactions(
    starknet: &Starknet,
    transaction: &dp_transactions::Transaction,
    tx_hash: &TransactionHash,
) -> RpcResult<btx::Transaction> {
    let transaction: Transaction = transaction.try_into().map_err(|e| {
        log::warn!("Failed to convert transaction to blockifier transaction: {e}");
        StarknetRpcApiError::InternalServerError
    })?;

    let paid_fee_on_l1 = match transaction {
        Transaction::L1Handler(_) => Some(starknet_api::transaction::Fee(1_000_000_000_000)),
        _ => None,
    };

    let class_info = match transaction {
        Transaction::Declare(ref declare_tx) => {
            let class_hash = declare_tx.class_hash();

            let Ok(Some(compiled_class)) = starknet.backend.compiled_contract_class().get(&class_hash.to_felt()) else {
                log::error!("Failed to retrieve class from class_hash '{class_hash}'");
                return Err(StarknetRpcApiError::ContractNotFound.into());
            };

            let blockifier_contract_class = to_blockifier_class(compiled_class).map_err(|e| {
                log::error!("Failed to convert contract class to blockifier contract class: {e}");
                StarknetRpcApiError::InternalServerError
            })?;

            let Ok(Some(contract_class_data)) = starknet.backend.contract_class_data().get(&class_hash.to_felt())
            else {
                log::error!("Failed to retrieve class from class_hash '{class_hash}'");
                return Err(StarknetRpcApiError::ContractNotFound.into());
            };
            let sierra_program_length = contract_class_data.contract_class.sierra_program_length();
            let abi_length = contract_class_data.contract_class.abi_length();

            Some(ClassInfo::new(&blockifier_contract_class, sierra_program_length, abi_length).map_err(|_| {
                log::error!("Mismatch between the length of the sierra program and the class version");
                StarknetRpcApiError::InternalServerError
            })?)
        }
        _ => None,
    };

    btx::Transaction::from_api(transaction.clone(), *tx_hash, class_info, paid_fee_on_l1, None, false).map_err(|_| {
        log::error!("Failed to convert transaction to blockifier transaction");
        StarknetRpcApiError::InternalServerError.into()
    })
}
