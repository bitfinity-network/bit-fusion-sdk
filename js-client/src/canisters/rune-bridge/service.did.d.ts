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
export interface CreateEdictTxArgs {
  'destination' : string,
  'rune_name' : string,
  'change_address' : [] | [string],
  'from_address' : string,
  'amount' : bigint,
}
export type DepositError = { 'Evm' : string } |
  { 'Sign' : string } |
  { 'NoRunesToDeposit' : null } |
  { 'NotingToDeposit' : null } |
  { 'NotInitialized' : null } |
  { 'NotEnoughBtc' : { 'minimum' : bigint, 'received' : bigint } } |
  { 'Unavailable' : string } |
  {
    'Pending' : {
      'current_confirmations' : number,
      'min_confirmations' : number,
    }
  };
export type Erc20MintStatus = {
    'Minted' : { 'tx_id' : string, 'amount' : bigint }
  } |
  {
    'Scheduled' : {
      'required_confirmations' : number,
      'pending_utxos' : [] | [Array<{}>],
      'current_confirmations' : number,
    }
  } |
  { 'Signed' : Uint8Array | number[] };
export type EvmLink = { 'Ic' : Principal } |
  { 'Http' : string };
export type GetAddressError = { 'Derivation' : null };
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
export type Result = { 'Ok' : Erc20MintStatus } |
  { 'Err' : DepositError };
export type Result_1 = { 'Ok' : string } |
  { 'Err' : GetAddressError };
export interface RuneBridgeConfig {
  'admin' : Principal,
  'signing_strategy' : SigningStrategy,
  'indexer_url' : string,
  'evm_link' : EvmLink,
  'rune_info' : RuneInfo,
  'network' : BitcoinNetwork,
  'min_confirmations' : number,
  'log_settings' : LogSettings,
  'deposit_fee' : bigint,
}
export interface RuneInfo { 'tx' : number, 'name' : string, 'block' : bigint }
export type SigningKeyId = { 'Dfx' : null } |
  { 'Production' : null } |
  { 'Test' : null } |
  { 'PocketIc' : null } |
  { 'Custom' : string };
export type SigningStrategy = {
    'Local' : { 'private_key' : Uint8Array | number[] }
  } |
  { 'ManagementCanister' : { 'key_id' : SigningKeyId } };
export interface _SERVICE {
  'admin_configure_bft_bridge' : ActorMethod<[BftBridgeConfig], undefined>,
  'admin_configure_ecdsa' : ActorMethod<[], undefined>,
  'create_edict_tx' : ActorMethod<[CreateEdictTxArgs], Uint8Array | number[]>,
  'deposit' : ActorMethod<[string], Result>,
  'get_curr_metrics' : ActorMethod<[], MetricsData>,
  'get_deposit_address' : ActorMethod<[string], Result_1>,
  'get_evm_address' : ActorMethod<[], [] | [string]>,
  'get_metrics' : ActorMethod<[], MetricsStorage>,
  'get_rune_balances' : ActorMethod<[string], Array<[string, bigint]>>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
