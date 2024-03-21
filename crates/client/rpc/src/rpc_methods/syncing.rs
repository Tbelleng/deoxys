use jsonrpsee::core::{async_trait, RpcResult};
use mc_genesis_data_provider::GenesisProvider;
pub use mc_rpc_core::utils::*;
pub use mc_rpc_core::{Felt, StarknetTraceRpcApiServer, StarknetWriteRpcApiServer, SyncingServer};
use mc_sync::l2::get_highest_block_hash_and_number;
use mp_hashers::HasherT;
use pallet_starknet_runtime_api::{ConvertTransactionRuntimeApi, StarknetRuntimeApi};
use sc_client_api::backend::{Backend, StorageProvider};
use sc_client_api::BlockBackend;
use sc_transaction_pool::ChainApi;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_arithmetic::traits::UniqueSaturatedInto;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use starknet_core::types::{SyncStatus, SyncStatusType};

use super::super::madara_backend_client;
use crate::Starknet;

#[async_trait]
#[allow(unused_variables)]
impl<A, B, BE, G, C, P, H> SyncingServer for Starknet<A, B, BE, G, C, P, H>
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
    /// Returns an object about the sync status, or false if the node is not synching
    ///
    /// ### Arguments
    ///
    /// This function does not take any arguments.
    ///
    /// ### Returns
    ///
    /// * `Syncing` - An Enum that can either be a `mc_rpc_core::SyncStatus` struct representing the
    ///   sync status, or a `Boolean` (`false`) indicating that the node is not currently
    ///   synchronizing.
    ///
    /// This is an asynchronous function due to its reliance on `sync_service.best_seen_block()`,
    /// which potentially involves network communication and processing to determine the best block
    /// seen by the sync service.
    async fn syncing(&self) -> RpcResult<SyncStatusType> {
        // obtain best seen (highest) block number
        match self.sync_service.best_seen_block().await {
            Ok(best_seen_block) => {
                let best_number = self.client.info().best_number;
                let highest_number = best_seen_block.unwrap_or(best_number);

                // get a starknet block from the starting substrate block number
                let starting_block = madara_backend_client::starknet_block_from_substrate_hash(
                    self.client.as_ref(),
                    self.starting_block,
                );

                // get a starknet block from the current substrate block number
                let current_block =
                    madara_backend_client::starknet_block_from_substrate_hash(self.client.as_ref(), best_number);

                // get a starknet block from the highest substrate block number
                let highest_block =
                    madara_backend_client::starknet_block_from_substrate_hash(self.client.as_ref(), highest_number);

                if starting_block.is_ok() && current_block.is_ok() && highest_block.is_ok() {
                    // Convert block numbers and hashes to the respective type required by the `syncing` endpoint.
                    let starting_block_num = UniqueSaturatedInto::<u64>::unique_saturated_into(self.starting_block);
                    let starting_block_hash = starting_block?.header().hash::<H>().0;

                    let current_block_num = UniqueSaturatedInto::<u64>::unique_saturated_into(best_number);
                    let current_block_hash = current_block?.header().hash::<H>().0;

                    // Get the highest block number and hash from the global variable update in l2 sync()
                    let (highest_block_hash, highest_block_num) = get_highest_block_hash_and_number();

                    // Build the `SyncStatus` struct with the respective syn information
                    Ok(SyncStatusType::Syncing(SyncStatus {
                        starting_block_num,
                        starting_block_hash,
                        current_block_num,
                        current_block_hash,
                        highest_block_num,
                        highest_block_hash,
                    }))
                } else {
                    // If there was an error when getting a starknet block, then we return `false`,
                    // as per the endpoint specification
                    log::error!("Failed to load Starknet block");
                    Ok(SyncStatusType::NotSyncing)
                }
            }
            Err(_) => {
                // If there was an error when getting a starknet block, then we return `false`,
                // as per the endpoint specification
                log::error!("`SyncingEngine` shut down");
                Ok(SyncStatusType::NotSyncing)
            }
        }
    }
}