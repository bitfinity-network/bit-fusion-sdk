import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export interface AccessListItem {
  'storageKeys' : Array<string>,
  'address' : string,
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
export type Interval = { 'PerHour' : null } |
  { 'PerWeek' : null } |
  { 'PerDay' : null } |
  { 'Period' : { 'seconds' : bigint } } |
  { 'PerMinute' : null };
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
export type Result = { 'Ok' : null } |
  { 'Err' : SignatureVerificationError };
export type Result_1 = { 'Ok' : string } |
  { 'Err' : SignatureVerificationError };
export type SignatureVerificationError = {
    'RecoveryError' : { 'recovered' : string, 'expected' : string }
  } |
  { 'Unauthorized' : null } |
  { 'InternalError' : string };
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
export interface _SERVICE {
  'add_access' : ActorMethod<[Principal], Result>,
  'get_access_list' : ActorMethod<[], Array<Principal>>,
  'get_canister_build_data' : ActorMethod<[], BuildData>,
  'get_curr_metrics' : ActorMethod<[], MetricsData>,
  'get_metrics' : ActorMethod<[], MetricsStorage>,
  'get_owner' : ActorMethod<[], Principal>,
  'remove_access' : ActorMethod<[Principal], Result>,
  'set_owner' : ActorMethod<[Principal], Result>,
  'verify_signature' : ActorMethod<[Transaction], Result_1>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: ({ IDL }: { IDL: IDL }) => IDL.Type[];
