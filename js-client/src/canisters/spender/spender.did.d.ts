import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

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
export type Result = { 'Ok' : bigint } |
  { 'Err' : TransferFromError };
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
export interface _SERVICE {
  'finish_icrc2_mint' : ActorMethod<
    [Principal, Principal, Uint8Array | number[], bigint, bigint],
    Result
  >,
  'get_curr_metrics' : ActorMethod<[], MetricsData>,
  'get_metrics' : ActorMethod<[], MetricsStorage>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: ({ IDL }: { IDL: IDL }) => IDL.Type[];
