use jsonrpsee::core::{async_trait, RpcResult};
use log::error;
use mc_genesis_data_provider::GenesisProvider;
pub use mc_rpc_core::utils::*;
use mc_rpc_core::ChainIdServer;
pub use mc_rpc_core::{EstimateFeeServer, Felt, StarknetTraceRpcApiServer, StarknetWriteRpcApiServer};
use mp_felt::Felt252Wrapper;
use mp_hashers::HasherT;
use mp_transactions::UserTransaction;
use pallet_starknet_runtime_api::{ConvertTransactionRuntimeApi, StarknetRuntimeApi};
use sc_client_api::backend::{Backend, StorageProvider};
use sc_client_api::BlockBackend;
use sc_transaction_pool::ChainApi;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use starknet_core::types::{BlockId, BroadcastedTransaction, FeeEstimate};

use crate::errors::StarknetRpcApiError;
use crate::Starknet;

#[async_trait]
#[allow(unused_variables)]
impl<A, B, BE, G, C, P, H> EstimateFeeServer for Starknet<A, B, BE, G, C, P, H>
where
    A: ChainApi<Block = B> + 'static,
    B: BlockT,
    P: TransactionPool<Block = B> + 'static,
    BE: Backend<B> + 'static,
    C: HeaderBackend<B> + BlockBackend<B> + StorageProvider<B, BE> + 'static,
    C: ProvideRuntimeApi<B>,
    C::Api: StarknetRuntimeApi<B> + ConvertTransactionRuntimeApi<B>,
    G: GenesisProvider + Send + Sync + 'static,
    H: HasherT + Send + Sync + 'static,
{
    /// Estimate the fee associated with transaction
    ///
    /// # Arguments
    ///
    /// * `request` - starknet transaction request
    /// * `block_id` - hash of the requested block, number (height), or tag
    ///
    /// # Returns
    ///
    /// * `fee_estimate` - fee estimate in gwei
    async fn estimate_fee(
        &self,
        request: Vec<BroadcastedTransaction>,
        block_id: BlockId,
    ) -> RpcResult<Vec<FeeEstimate>> {
        let substrate_block_hash = self.substrate_block_hash_from_starknet_block(block_id).map_err(|e| {
            error!("'{e}'");
            StarknetRpcApiError::BlockNotFound
        })?;
        let best_block_hash = self.client.info().best_hash;
        let chain_id = Felt252Wrapper(self.chain_id()?.0);

        let transactions =
            request.into_iter().map(|tx| tx.try_into()).collect::<Result<Vec<UserTransaction>, _>>().map_err(|e| {
                error!("Failed to convert BroadcastedTransaction to UserTransaction: {e}");
                StarknetRpcApiError::InternalServerError
            })?;

        let fee_estimates = self
            .client
            .runtime_api()
            .estimate_fee(substrate_block_hash, transactions)
            .map_err(|e| {
                error!("Request parameters error: {e}");
                StarknetRpcApiError::InternalServerError
            })?
            .map_err(|e| {
                error!("Failed to call function: {:#?}", e);
                StarknetRpcApiError::ContractError
            })?;

        let estimates = fee_estimates
            .into_iter()
			// FIXME: https://github.com/keep-starknet-strange/madara/issues/329
            .map(|x| FeeEstimate { gas_price: 10, gas_consumed: x.1, overall_fee: x.0 })
            .collect();

        Ok(estimates)
    }
}