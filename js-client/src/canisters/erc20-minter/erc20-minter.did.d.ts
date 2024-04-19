import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export type BridgeSide = { 'Base' : null } |
  { 'Wrapped' : null };
export type EvmLink = { 'Ic' : Principal } |
  { 'Http' : string };
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
export interface Settings {
  'signing_strategy' : SigningStrategy,
  'base_bridge_contract' : string,
  'wrapped_bridge_contract' : string,
  'wrapped_evm_link' : EvmLink,
  'log_settings' : [] | [LogSettings],
  'base_evm_link' : EvmLink,
}
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
  'admin_set_bft_bridge_address' : ActorMethod<
    [BridgeSide, string],
    [] | [null]
  >,
  'get_curr_metrics' : ActorMethod<[], MetricsData>,
  'get_evm_address' : ActorMethod<[], [] | [string]>,
  'get_metrics' : ActorMethod<[], MetricsStorage>,
  'get_mint_order' : ActorMethod<
    [Uint8Array | number[], Uint8Array | number[], number],
    [] | [Uint8Array | number[]]
  >,
  'list_mint_orders' : ActorMethod<
    [Uint8Array | number[], Uint8Array | number[]],
    Array<[number, Uint8Array | number[]]>
  >,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: ({ IDL }: { IDL: IDL }) => IDL.Type[];
