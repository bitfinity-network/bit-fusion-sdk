import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export interface AccessListItem {
  'storageKeys' : Array<string>,
  'address' : string,
}
export interface BasicAccount { 'balance' : string, 'nonce' : string }
export interface Block {
  'miner' : string,
  'totalDifficulty' : string,
  'receiptsRoot' : string,
  'stateRoot' : string,
  'hash' : string,
  'difficulty' : string,
  'size' : [] | [string],
  'uncles' : Array<string>,
  'baseFeePerGas' : [] | [string],
  'extraData' : string,
  'sealFields' : Array<string>,
  'transactionsRoot' : string,
  'sha3Uncles' : string,
  'nonce' : string,
  'number' : string,
  'timestamp' : string,
  'transactions' : Array<string>,
  'gasLimit' : string,
  'logsBloom' : string,
  'parentHash' : string,
  'gasUsed' : string,
  'mixHash' : string,
}
export type BlockResult = { 'NoBlockFound' : null } |
  { 'WithHash' : Block } |
  { 'WithTransaction' : Block_1 };
export interface Block_1 {
  'miner' : string,
  'totalDifficulty' : string,
  'receiptsRoot' : string,
  'stateRoot' : string,
  'hash' : string,
  'difficulty' : string,
  'size' : [] | [string],
  'uncles' : Array<string>,
  'baseFeePerGas' : [] | [string],
  'extraData' : string,
  'sealFields' : Array<string>,
  'transactionsRoot' : string,
  'sha3Uncles' : string,
  'nonce' : string,
  'number' : string,
  'timestamp' : string,
  'transactions' : Array<Transaction>,
  'gasLimit' : string,
  'logsBloom' : string,
  'parentHash' : string,
  'gasUsed' : string,
  'mixHash' : string,
}
export interface BlockchainStorageLimits {
  'receipts_bytes_limit' : bigint,
  'blocks_and_transactions_bytes_limit' : bigint,
}
export interface BuildData {
  'rustc_semver' : string,
  'git_branch' : string,
  'pkg_version' : string,
  'cargo_target_triple' : string,
  'cargo_debug' : string,
  'pkg_name' : string,
  'cargo_features' : string,
  'build_timestamp' : string,
  'git_sha' : string,
  'git_commit_timestamp' : string,
}
export interface Duration { 'secs' : bigint, 'nanos' : number }
export interface EstimateGasRequest {
  'to' : [] | [string],
  'gas' : [] | [string],
  'maxFeePerGas' : [] | [string],
  'gasPrice' : [] | [string],
  'value' : [] | [string],
  'data' : [] | [string],
  'from' : [] | [string],
  'accessList' : [] | [Array<AccessListItem>],
  'nonce' : [] | [string],
  'maxPriorityFeePerGas' : [] | [string],
  'chainId' : [] | [string],
}
export interface EvmCanisterInitData {
  'permissions' : [] | [Array<[Principal, Array<Permission>]>],
  'owner' : Principal,
  'min_gas_price' : bigint,
  'chain_id' : bigint,
  'signature_verification_principal' : Principal,
  'reserve_memory_pages' : [] | [bigint],
  'genesis_accounts' : Array<[string, [] | [string]]>,
  'coinbase' : string,
  'transaction_processing_interval' : [] | [Duration],
  'log_settings' : [] | [LogSettings],
}
export type EvmError = { 'Internal' : string } |
  { 'TransactionSignature' : string } |
  { 'StableStorageError' : string } |
  { 'InsufficientBalance' : { 'actual' : string, 'expected' : string } } |
  { 'TransactionPool' : TransactionPoolError } |
  { 'NotAuthorized' : null } |
  { 'AnonymousPrincipal' : null } |
  { 'GasTooLow' : { 'minimum' : string } } |
  { 'BlockDoesNotExist' : string } |
  { 'NoHistoryDataForBlock' : string } |
  { 'TransactionReverted' : string } |
  { 'InvalidGasPrice' : string } |
  { 'NotProcessableTransactionError' : HaltError } |
  { 'ReservationFailed' : string } |
  { 'BadRequest' : string } |
  { 'FatalEvmExecutorError' : ExitFatal };
export interface EvmStats {
  'block_number' : bigint,
  'cycles' : bigint,
  'chain_id' : bigint,
  'pending_transactions' : Array<string>,
  'pending_transactions_count' : bigint,
  'block_gas_limit' : bigint,
  'state_root' : string,
}
export type ExeResult = {
    'Halt' : { 'error' : HaltError, 'gas_used' : string }
  } |
  {
    'Revert' : {
      'output' : string,
      'revert_message' : [] | [string],
      'gas_used' : string,
    }
  } |
  {
    'Success' : {
      'output' : TransactOut,
      'logs' : Array<TransactionExecutionLog>,
      'gas_used' : string,
      'logs_bloom' : string,
    }
  };
export type ExitFatal = { 'UnhandledInterrupt' : null } |
  { 'NotSupported' : null } |
  { 'Other' : string } |
  { 'CallErrorAsFatal' : HaltError };
export interface FeeHistory {
  'reward' : [] | [Array<Array<string>>],
  'base_fee_per_gas' : Array<string>,
  'oldest_block' : string,
  'gas_used_ratio' : Array<number>,
}
export interface FullStorageValue {
  'data' : Uint8Array | number[],
  'ref_count' : number,
  'removed_at_block' : bigint,
}
export type HaltError = { 'DesignatedInvalid' : null } |
  { 'OutOfOffset' : null } |
  { 'Continue' : null } |
  { 'PriorityFeeGreaterThanMaxFee' : null } |
  { 'CallGasCostMoreThanGasLimit' : null } |
  { 'InvalidChainId' : null } |
  { 'Revert' : [] | [string] } |
  { 'InvalidRange' : null } |
  { 'CreateContractLimit' : null } |
  { 'CallerGasLimitMoreThanBlock' : null } |
  { 'InvalidOpcode' : null } |
  { 'StateChangeDuringStaticCall' : null } |
  { 'LackOfFundForMaxFee' : { 'fee' : string, 'balance' : string } } |
  { 'CreateEmpty' : null } |
  { 'InvalidCode' : number } |
  { 'GasPriceLessThanBasefee' : null } |
  { 'InvalidJump' : null } |
  { 'OutOfFund' : null } |
  { 'NonceTooLow' : { 'tx' : bigint, 'state' : bigint } } |
  { 'PrecompileError' : null } |
  { 'OpcodeNotFound' : null } |
  { 'NotActivated' : null } |
  { 'PCUnderflow' : null } |
  { 'OverflowPayment' : null } |
  { 'PrevrandaoNotSet' : null } |
  { 'OutOfGas' : null } |
  { 'Other' : string } |
  { 'CallNotAllowedInsideStatic' : null } |
  { 'NonceTooHigh' : { 'tx' : bigint, 'state' : bigint } } |
  { 'RejectCallerWithCode' : null } |
  { 'CallTooDeep' : null } |
  { 'NonceOverflow' : null } |
  { 'FatalExternalError' : null } |
  { 'CreateContractWithEF' : null } |
  { 'CreateCollision' : null } |
  { 'StackOverflow' : null } |
  { 'CreateInitcodeSizeLimit' : null } |
  { 'StackUnderflow' : null };
export interface HttpRequest {
  'url' : string,
  'method' : string,
  'body' : Uint8Array | number[],
  'headers' : Array<[string, string]>,
}
export interface HttpResponse {
  'body' : Uint8Array | number[],
  'headers' : Array<[string, string]>,
  'upgrade' : [] | [boolean],
  'status_code' : number,
}
export interface Indices { 'history_size' : bigint, 'pending_block' : bigint }
export type Interval = { 'PerHour' : null } |
  { 'PerWeek' : null } |
  { 'PerDay' : null } |
  { 'Period' : { 'seconds' : bigint } } |
  { 'PerMinute' : null };
export interface Log { 'log' : string, 'offset' : bigint }
export interface LogSettings {
  'log_filter' : [] | [string],
  'in_memory_records' : [] | [bigint],
  'enable_console' : boolean,
}
export interface Logs { 'logs' : Array<Log>, 'all_logs_count' : bigint }
export interface MetricsData {
  'stable_memory_size' : bigint,
  'cycles' : bigint,
  'heap_memory_size' : bigint,
}
export interface MetricsMap {
  'map' : Array<[bigint, MetricsData]>,
  'interval' : Interval,
  'history_length_nanos' : bigint,
}
export interface MetricsStorage { 'metrics' : MetricsMap }
export type Permission = { 'ReadLogs' : null } |
  { 'Admin' : null } |
  { 'UpdateLogsConfiguration' : null } |
  { 'UpdateBlockchain' : null };
export interface PermissionList { 'permissions' : Array<Permission> }
export type Result = { 'Ok' : null } |
  { 'Err' : EvmError };
export type Result_1 = { 'Ok' : PermissionList } |
  { 'Err' : EvmError };
export type Result_10 = { 'Ok' : Array<[bigint, string]> } |
  { 'Err' : EvmError };
export type Result_11 = { 'Ok' : Array<[string, bigint]> } |
  { 'Err' : EvmError };
export type Result_12 = { 'Ok' : Logs } |
  { 'Err' : EvmError };
export type Result_13 = { 'Ok' : EvmStats } |
  { 'Err' : EvmError };
export type Result_14 = { 'Ok' : [string, string] } |
  { 'Err' : EvmError };
export type Result_15 = { 'Ok' : bigint } |
  { 'Err' : EvmError };
export type Result_2 = { 'Ok' : string } |
  { 'Err' : EvmError };
export type Result_3 = { 'Ok' : string } |
  { 'Err' : EvmError };
export type Result_4 = { 'Ok' : FeeHistory } |
  { 'Err' : EvmError };
export type Result_5 = { 'Ok' : BlockResult } |
  { 'Err' : EvmError };
export type Result_6 = { 'Ok' : bigint } |
  { 'Err' : EvmError };
export type Result_7 = { 'Ok' : Array<BlockResult> } |
  { 'Err' : EvmError };
export type Result_8 = { 'Ok' : [] | [Transaction] } |
  { 'Err' : EvmError };
export type Result_9 = { 'Ok' : [] | [TransactionReceipt] } |
  { 'Err' : EvmError };
export type StateUpdateAction = {
    'Replace' : { 'key' : [bigint, string], 'value' : null }
  } |
  { 'Removed' : { 'key' : [bigint, string] } };
export type StateUpdateAction_1 = {
    'Replace' : { 'key' : string, 'value' : FullStorageValue }
  } |
  { 'Removed' : { 'key' : string } };
export interface StorableExecutionResult {
  'to' : [] | [string],
  'transaction_hash' : string,
  'transaction_type' : [] | [string],
  'block_hash' : string,
  'max_priority_fee_per_gas' : [] | [string],
  'from' : string,
  'transaction_index' : string,
  'max_fee_per_gas' : [] | [string],
  'block_number' : string,
  'cumulative_gas_used' : string,
  'timestamp' : bigint,
  'exe_result' : ExeResult,
  'gas_price' : [] | [string],
}
export type TransactOut = { 'Call' : Uint8Array | number[] } |
  { 'None' : null } |
  { 'Create' : [Uint8Array | number[], [] | [string]] };
export interface Transaction {
  'r' : string,
  's' : string,
  'v' : string,
  'to' : [] | [string],
  'gas' : string,
  'maxFeePerGas' : [] | [string],
  'gasPrice' : [] | [string],
  'value' : string,
  'blockNumber' : [] | [string],
  'from' : string,
  'hash' : string,
  'blockHash' : [] | [string],
  'type' : [] | [string],
  'accessList' : [] | [Array<AccessListItem>],
  'transactionIndex' : [] | [string],
  'nonce' : string,
  'maxPriorityFeePerGas' : [] | [string],
  'input' : string,
  'chainId' : [] | [string],
}
export interface TransactionExecutionLog {
  'data' : string,
  'topics' : Array<string>,
  'address' : string,
}
export type TransactionPoolError = {
    'InvalidNonce' : { 'actual' : string, 'expected' : string }
  } |
  { 'TransactionAlreadyExists' : null } |
  { 'TxReplacementUnderpriced' : null } |
  { 'TooManyTransactions' : null };
export interface TransactionReceipt {
  'to' : [] | [string],
  'status' : [] | [string],
  'output' : [] | [Uint8Array | number[]],
  'transactionHash' : string,
  'cumulativeGasUsed' : string,
  'blockNumber' : string,
  'from' : string,
  'logs' : Array<TransactionReceiptLog>,
  'blockHash' : string,
  'root' : [] | [string],
  'type' : [] | [string],
  'transactionIndex' : string,
  'effectiveGasPrice' : [] | [string],
  'logsBloom' : string,
  'contractAddress' : [] | [string],
  'gasUsed' : [] | [string],
}
export interface TransactionReceiptLog {
  'transactionHash' : string,
  'blockNumber' : string,
  'data' : string,
  'blockHash' : string,
  'transactionIndex' : string,
  'topics' : Array<string>,
  'address' : string,
  'logIndex' : string,
  'removed' : boolean,
}
export interface _SERVICE {
  'account_basic' : ActorMethod<[string], BasicAccount>,
  'admin_allow_empty_blocks' : ActorMethod<[boolean], Result>,
  'admin_disable_evm' : ActorMethod<[boolean], Result>,
  'admin_ic_permissions_add' : ActorMethod<
    [Principal, Array<Permission>],
    Result_1
  >,
  'admin_ic_permissions_get' : ActorMethod<[Principal], Result_1>,
  'admin_ic_permissions_remove' : ActorMethod<
    [Principal, Array<Permission>],
    Result_1
  >,
  'admin_reserve_stable_storage_pages' : ActorMethod<[bigint], Result>,
  'admin_set_block_gas_limit' : ActorMethod<[bigint], Result>,
  'admin_set_block_size_limit' : ActorMethod<[bigint], Result>,
  'admin_set_blockchain_size_limit' : ActorMethod<
    [BlockchainStorageLimits],
    Result
  >,
  'admin_set_coinbase' : ActorMethod<[string], Result>,
  'admin_set_evm_state_history_size' : ActorMethod<[bigint], Result>,
  'admin_set_max_batch_requests' : ActorMethod<[number], Result>,
  'admin_set_max_tx_pool_size' : ActorMethod<[bigint], Result>,
  'admin_set_min_gas_price' : ActorMethod<[string], Result>,
  'admin_set_panic_transaction_from' : ActorMethod<[[] | [string]], Result>,
  'append_blockchain_blocks' : ActorMethod<
    [Array<[Block, Array<[Transaction, ExeResult]>]>],
    Result
  >,
  'apply_clear_info_changes' : ActorMethod<[Array<StateUpdateAction>], Result>,
  'apply_state_storage_changes' : ActorMethod<
    [Array<StateUpdateAction_1>],
    Result
  >,
  'blocks_and_transactions_history_total_bytes' : ActorMethod<[], bigint>,
  'eth_accounts' : ActorMethod<[], Array<string>>,
  'eth_block_number' : ActorMethod<[], bigint>,
  'eth_call' : ActorMethod<
    [
      [] | [string],
      [] | [string],
      [] | [string],
      bigint,
      [] | [string],
      [] | [string],
    ],
    Result_2
  >,
  'eth_chain_id' : ActorMethod<[], bigint>,
  'eth_estimate_gas' : ActorMethod<[EstimateGasRequest], Result_3>,
  'eth_fee_history' : ActorMethod<
    [bigint, string, [] | [Array<number>]],
    Result_4
  >,
  'eth_gas_price' : ActorMethod<[], Result_3>,
  'eth_get_balance' : ActorMethod<[string, string], Result_3>,
  'eth_get_block_by_hash' : ActorMethod<[string, boolean], Result_5>,
  'eth_get_block_by_number' : ActorMethod<[string, boolean], Result_5>,
  'eth_get_block_transaction_count_by_block_number' : ActorMethod<
    [string],
    Result_6
  >,
  'eth_get_block_transaction_count_by_hash' : ActorMethod<[string], bigint>,
  'eth_get_block_transaction_count_by_number' : ActorMethod<[string], Result_6>,
  'eth_get_blocks_by_number' : ActorMethod<[string, string, boolean], Result_7>,
  'eth_get_code' : ActorMethod<[string, string], Result_3>,
  'eth_get_storage_at' : ActorMethod<[string, string, string], Result_3>,
  'eth_get_transaction_by_block_hash_and_index' : ActorMethod<
    [string, bigint],
    [] | [Transaction]
  >,
  'eth_get_transaction_by_block_number_and_index' : ActorMethod<
    [string, bigint],
    Result_8
  >,
  'eth_get_transaction_by_hash' : ActorMethod<[string], [] | [Transaction]>,
  'eth_get_transaction_count' : ActorMethod<[string, string], Result_3>,
  'eth_get_transaction_receipt' : ActorMethod<[string], Result_9>,
  'eth_hashrate' : ActorMethod<[], bigint>,
  'eth_max_priority_fee_per_gas' : ActorMethod<[], Result_3>,
  'eth_mining' : ActorMethod<[], boolean>,
  'eth_protocol_version' : ActorMethod<[], bigint>,
  'eth_syncing' : ActorMethod<[], boolean>,
  'get_block_gas_limit' : ActorMethod<[], bigint>,
  'get_block_size_limit' : ActorMethod<[], bigint>,
  'get_blockchain_size_limit' : ActorMethod<[], BlockchainStorageLimits>,
  'get_canister_build_data' : ActorMethod<[], BuildData>,
  'get_clear_info_entries' : ActorMethod<
    [[] | [[bigint, string]], number],
    Result_10
  >,
  'get_curr_metrics' : ActorMethod<[], MetricsData>,
  'get_evm_state_history_size' : ActorMethod<[], bigint>,
  'get_genesis_accounts' : ActorMethod<[], Array<[string, string]>>,
  'get_max_batch_requests' : ActorMethod<[], number>,
  'get_max_tx_pool_size' : ActorMethod<[], bigint>,
  'get_metrics' : ActorMethod<[], MetricsStorage>,
  'get_min_gas_price' : ActorMethod<[], string>,
  'get_state_storage_item_hashes' : ActorMethod<
    [[] | [string], number],
    Result_11
  >,
  'get_tx_execution_result_by_hash' : ActorMethod<
    [string],
    [] | [StorableExecutionResult]
  >,
  'http_request' : ActorMethod<[HttpRequest], HttpResponse>,
  'http_request_update' : ActorMethod<[HttpRequest], HttpResponse>,
  'ic_logs' : ActorMethod<[bigint, bigint], Result_12>,
  'ic_stats' : ActorMethod<[], Result_13>,
  'is_address_reserved' : ActorMethod<[Principal, string], boolean>,
  'is_empty_block_enabled' : ActorMethod<[], boolean>,
  'is_evm_disabled' : ActorMethod<[], boolean>,
  'memory_pages_allocated_for_id' : ActorMethod<[number], bigint>,
  'mint_native_tokens' : ActorMethod<[string, string], Result_14>,
  'net_listening' : ActorMethod<[], boolean>,
  'net_peer_count' : ActorMethod<[], bigint>,
  'net_version' : ActorMethod<[], bigint>,
  'receipts_history_total_bytes' : ActorMethod<[], bigint>,
  'reserve_address' : ActorMethod<[Principal, string], Result>,
  'revert_blockchain_to_block' : ActorMethod<[bigint], Result_15>,
  'send_raw_transaction' : ActorMethod<[Transaction], Result_3>,
  'set_logger_filter' : ActorMethod<[string], Result>,
  'set_state_root' : ActorMethod<[string], Result>,
  'set_storage_indices' : ActorMethod<[Indices], Result>,
  'web3_client_version' : ActorMethod<[], string>,
  'web3_sha3' : ActorMethod<[string], Result_3>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: ({ IDL }: { IDL: IDL }) => IDL.Type[];
