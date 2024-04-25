import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export type ApproveError = {
    'GenericError' : { 'message' : string, 'error_code' : bigint }
  } |
  { 'TemporarilyUnavailable' : null } |
  { 'Duplicate' : { 'duplicate_of' : bigint } } |
  { 'BadFee' : { 'expected_fee' : bigint } } |
  { 'AllowanceChanged' : { 'current_allowance' : bigint } } |
  { 'CreatedInFuture' : { 'ledger_time' : bigint } } |
  { 'TooOld' : null } |
  { 'Expired' : { 'ledger_time' : bigint } } |
  { 'InsufficientFunds' : { 'balance' : bigint } };
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
export type Error = { 'Internal' : string } |
  { 'InvalidNonce' : { 'got' : bigint, 'minimum' : bigint } } |
  { 'Icrc2ApproveError' : ApproveError } |
  { 'InvalidTokenAddress' : null } |
  { 'BftBridgeAlreadyRegistered' : string } |
  { 'Icrc2TransferFromError' : TransferFromError } |
  { 'NotAuthorized' : null } |
  { 'AnonymousPrincipal' : null } |
  { 'BftBridgeDoesNotExist' : null } |
  { 'JsonRpcCallFailed' : string } |
  { 'InsufficientOperationPoints' : { 'got' : number, 'expected' : number } } |
  { 'InvalidBftBridgeContract' : null } |
  { 'InvalidBurnOperation' : string };
export interface HttpHeader { 'value' : string, 'name' : string }
export interface HttpResponse {
  'status' : bigint,
  'body' : Uint8Array | number[],
  'headers' : Array<HttpHeader>,
}
export interface Icrc2Burn {
  'operation_id' : number,
  'from_subaccount' : [] | [Uint8Array | number[]],
  'icrc2_token_principal' : Principal,
  'recipient_address' : string,
  'amount' : string,
}
export interface InitData {
  'evm_principal' : Principal,
  'signing_strategy' : SigningStrategy,
  'owner' : Principal,
  'spender_principal' : Principal,
  'log_settings' : [] | [LogSettings],
}
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
export type Result = { 'Ok' : number } |
  { 'Err' : Error };
export type Result_1 = { 'Ok' : string } |
  { 'Err' : Error };
export type Result_2 = { 'Ok' : Logs } |
  { 'Err' : Error };
export type Result_3 = { 'Ok' : null } |
  { 'Err' : Error };
export type SigningKeyId = { 'Dfx' : null } |
  { 'Production' : null } |
  { 'Test' : null } |
  { 'PocketIc' : null } |
  { 'Custom' : string };
export type SigningStrategy = {
    'Local' : { 'private_key' : Uint8Array | number[] }
  } |
  { 'ManagementCanister' : { 'key_id' : SigningKeyId } };
export type TransferFromError = {
    'GenericError' : { 'message' : string, 'error_code' : bigint }
  } |
  { 'TemporarilyUnavailable' : null } |
  { 'InsufficientAllowance' : { 'allowance' : bigint } } |
  { 'BadBurn' : { 'min_burn_amount' : bigint } } |
  { 'Duplicate' : { 'duplicate_of' : bigint } } |
  { 'BadFee' : { 'expected_fee' : bigint } } |
  { 'CreatedInFuture' : { 'ledger_time' : bigint } } |
  { 'TooOld' : null } |
  { 'InsufficientFunds' : { 'balance' : bigint } };
export interface TransformArgs {
  'context' : Uint8Array | number[],
  'response' : HttpResponse,
}
export interface _SERVICE {
  'burn_icrc2' : ActorMethod<[Icrc2Burn], Result>,
  'get_bft_bridge_contract' : ActorMethod<[], [] | [string]>,
  'get_canister_build_data' : ActorMethod<[], BuildData>,
  'get_curr_metrics' : ActorMethod<[], MetricsData>,
  'get_evm_principal' : ActorMethod<[], Principal>,
  'get_metrics' : ActorMethod<[], MetricsStorage>,
  'get_mint_order' : ActorMethod<
    [Uint8Array | number[], Uint8Array | number[], number],
    [] | [Uint8Array | number[]]
  >,
  'get_minter_canister_evm_address' : ActorMethod<[], Result_1>,
  'get_owner' : ActorMethod<[], Principal>,
  'ic_logs' : ActorMethod<[bigint, bigint], Result_2>,
  'list_mint_orders' : ActorMethod<
    [Uint8Array | number[], Uint8Array | number[]],
    Array<[number, Uint8Array | number[]]>
  >,
  'register_evmc_bft_bridge' : ActorMethod<[string], Result_3>,
  'set_evm_principal' : ActorMethod<[Principal], Result_3>,
  'set_logger_filter' : ActorMethod<[string], Result_3>,
  'set_owner' : ActorMethod<[Principal], Result_3>,
  'transform' : ActorMethod<[TransformArgs], HttpResponse>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: ({ IDL }: { IDL: IDL }) => IDL.Type[];
