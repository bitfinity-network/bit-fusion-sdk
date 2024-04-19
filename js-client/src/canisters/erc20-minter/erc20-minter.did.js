export const idlFactory = ({ IDL }) => {
  const SigningKeyId = IDL.Variant({
    'Dfx' : IDL.Null,
    'Production' : IDL.Null,
    'Test' : IDL.Null,
    'PocketIc' : IDL.Null,
    'Custom' : IDL.Text,
  });
  const SigningStrategy = IDL.Variant({
    'Local' : IDL.Record({ 'private_key' : IDL.Vec(IDL.Nat8) }),
    'ManagementCanister' : IDL.Record({ 'key_id' : SigningKeyId }),
  });
  const EvmLink = IDL.Variant({ 'Ic' : IDL.Principal, 'Http' : IDL.Text });
  const LogSettings = IDL.Record({
    'log_filter' : IDL.Opt(IDL.Text),
    'in_memory_records' : IDL.Opt(IDL.Nat64),
    'enable_console' : IDL.Bool,
  });
  const Settings = IDL.Record({
    'signing_strategy' : SigningStrategy,
    'base_bridge_contract' : IDL.Text,
    'wrapped_bridge_contract' : IDL.Text,
    'wrapped_evm_link' : EvmLink,
    'log_settings' : IDL.Opt(LogSettings),
    'base_evm_link' : EvmLink,
  });
  const BridgeSide = IDL.Variant({ 'Base' : IDL.Null, 'Wrapped' : IDL.Null });
  const MetricsData = IDL.Record({
    'stable_memory_size' : IDL.Nat64,
    'cycles' : IDL.Nat64,
    'heap_memory_size' : IDL.Nat64,
  });
  const Interval = IDL.Variant({
    'PerHour' : IDL.Null,
    'PerWeek' : IDL.Null,
    'PerDay' : IDL.Null,
    'Period' : IDL.Record({ 'seconds' : IDL.Nat64 }),
    'PerMinute' : IDL.Null,
  });
  const MetricsMap = IDL.Record({
    'map' : IDL.Vec(IDL.Tuple(IDL.Nat64, MetricsData)),
    'interval' : Interval,
    'history_length_nanos' : IDL.Nat64,
  });
  const MetricsStorage = IDL.Record({ 'metrics' : MetricsMap });
  return IDL.Service({
    'admin_set_bft_bridge_address' : IDL.Func(
        [BridgeSide, IDL.Text],
        [IDL.Opt(IDL.Null)],
        [],
      ),
    'get_curr_metrics' : IDL.Func([], [MetricsData], ['query']),
    'get_evm_address' : IDL.Func([], [IDL.Opt(IDL.Text)], []),
    'get_metrics' : IDL.Func([], [MetricsStorage], ['query']),
    'get_mint_order' : IDL.Func(
        [IDL.Vec(IDL.Nat8), IDL.Vec(IDL.Nat8), IDL.Nat32],
        [IDL.Opt(IDL.Vec(IDL.Nat8))],
        ['query'],
      ),
    'list_mint_orders' : IDL.Func(
        [IDL.Vec(IDL.Nat8), IDL.Vec(IDL.Nat8)],
        [IDL.Vec(IDL.Tuple(IDL.Nat32, IDL.Vec(IDL.Nat8)))],
        ['query'],
      ),
  });
};
export const init = ({ IDL }) => {
  const SigningKeyId = IDL.Variant({
    'Dfx' : IDL.Null,
    'Production' : IDL.Null,
    'Test' : IDL.Null,
    'PocketIc' : IDL.Null,
    'Custom' : IDL.Text,
  });
  const SigningStrategy = IDL.Variant({
    'Local' : IDL.Record({ 'private_key' : IDL.Vec(IDL.Nat8) }),
    'ManagementCanister' : IDL.Record({ 'key_id' : SigningKeyId }),
  });
  const EvmLink = IDL.Variant({ 'Ic' : IDL.Principal, 'Http' : IDL.Text });
  const LogSettings = IDL.Record({
    'log_filter' : IDL.Opt(IDL.Text),
    'in_memory_records' : IDL.Opt(IDL.Nat64),
    'enable_console' : IDL.Bool,
  });
  const Settings = IDL.Record({
    'signing_strategy' : SigningStrategy,
    'base_bridge_contract' : IDL.Text,
    'wrapped_bridge_contract' : IDL.Text,
    'wrapped_evm_link' : EvmLink,
    'log_settings' : IDL.Opt(LogSettings),
    'base_evm_link' : EvmLink,
  });
  return [Settings];
};
