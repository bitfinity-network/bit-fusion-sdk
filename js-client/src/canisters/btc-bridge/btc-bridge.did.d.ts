import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export interface BftBridgeConfig {
  'decimals' : number,
  'token_symbol' : Uint8Array | number[],
  'token_address' : string,
  'bridge_address' : string,
  'erc20_chain_id' : number,
  'token_name' : Uint8Array | number[],
}
export type BitcoinNetwork = { 'mainnet' : null } |
  { 'regtest' : null } |
  { 'testnet' : null };
export interface BtcBridgeConfig {
  'admin' : Principal,
  'signing_strategy' : SigningStrategy,
  'ck_btc_ledger_fee' : bigint,
  'evm_link' : EvmLink,
  'ck_btc_minter' : Principal,
  'network' : BitcoinNetwork,
  'ck_btc_ledger' : Principal,
  'log_settings' : LogSettings,
}
export type Erc20MintError = { 'Evm' : string } |
  { 'CkBtcMinter' : UpdateBalanceError } |
  { 'ValueTooSmall' : null } |
  { 'Tainted' : Utxo } |
  { 'Sign' : string } |
  { 'CkBtcLedger' : TransferError } |
  { 'NotInitialized' : null } |
  { 'NothingToMint' : null };
export type Erc20MintStatus = {
    'Minted' : { 'tx_id' : string, 'amount' : bigint }
  } |
  {
    'Scheduled' : {
      'required_confirmations' : number,
      'pending_utxos' : [] | [Array<PendingUtxo>],
      'current_confirmations' : number,
    }
  } |
  { 'Signed' : Uint8Array | number[] };
export type EvmLink = { 'Ic' : Principal } |
  { 'Http' : string };
export interface GetBtcAddressArgs {
  'owner' : [] | [Principal],
  'subaccount' : [] | [Uint8Array | number[]],
}
export type Interval = { 'PerHour' : null } |
  { 'PerWeek' : null } |
  { 'PerDay' : null } |
  { 'Period' : { 'seconds' : bigint } } |
  { 'PerMinute' : null };
export interface LogSettings {
  'log_filter' : [] | [string],
  'in_memory_records' : [] | [bigint],
  'enable_console' : boolean,
}
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
export interface OutPoint { 'txid' : Uint8Array | number[], 'vout' : number }
export interface PendingUtxo {
  'confirmations' : number,
  'value' : bigint,
  'outpoint' : OutPoint,
}
export type Result = { 'Ok' : Erc20MintStatus } |
  { 'Err' : Erc20MintError };
export type SigningKeyId = { 'Dfx' : null } |
  { 'Production' : null } |
  { 'Test' : null } |
  { 'PocketIc' : null } |
  { 'Custom' : string };
export type SigningStrategy = {
    'Local' : { 'private_key' : Uint8Array | number[] }
  } |
  { 'ManagementCanister' : { 'key_id' : SigningKeyId } };
export type TransferError = {
    'GenericError' : { 'message' : string, 'error_code' : bigint }
  } |
  { 'TemporarilyUnavailable' : null } |
  { 'BadBurn' : { 'min_burn_amount' : bigint } } |
  { 'Duplicate' : { 'duplicate_of' : bigint } } |
  { 'BadFee' : { 'expected_fee' : bigint } } |
  { 'CreatedInFuture' : { 'ledger_time' : bigint } } |
  { 'TooOld' : null } |
  { 'InsufficientFunds' : { 'balance' : bigint } };
export type UpdateBalanceError = {
    'GenericError' : { 'error_message' : string, 'error_code' : bigint }
  } |
  { 'TemporarilyUnavailable' : string } |
  { 'AlreadyProcessing' : null } |
  {
    'NoNewUtxos' : {
      'required_confirmations' : number,
      'pending_utxos' : [] | [Array<PendingUtxo>],
      'current_confirmations' : [] | [number],
    }
  };
export interface Utxo {
  'height' : number,
  'value' : bigint,
  'outpoint' : OutPoint,
}
export interface _SERVICE {
  'admin_configure_bft_bridge' : ActorMethod<[BftBridgeConfig], undefined>,
  'btc_to_erc20' : ActorMethod<[string], Array<Result>>,
  'get_bft_bridge_contract' : ActorMethod<[], [] | [string]>,
  'get_btc_address' : ActorMethod<[GetBtcAddressArgs], string>,
  'get_curr_metrics' : ActorMethod<[], MetricsData>,
  'get_evm_address' : ActorMethod<[], [] | [string]>,
  'get_metrics' : ActorMethod<[], MetricsStorage>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: ({ IDL }: { IDL: IDL }) => IDL.Type[];
