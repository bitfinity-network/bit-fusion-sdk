export const idlFactory = ({ IDL }) => {
  const SignatureVerificationError = IDL.Variant({
    'RecoveryError' : IDL.Record({
      'recovered' : IDL.Text,
      'expected' : IDL.Text,
    }),
    'Unauthorized' : IDL.Null,
    'InternalError' : IDL.Text,
  });
  const Result = IDL.Variant({
    'Ok' : IDL.Null,
    'Err' : SignatureVerificationError,
  });
  const BuildData = IDL.Record({
    'rustc_semver' : IDL.Text,
    'git_branch' : IDL.Text,
    'pkg_version' : IDL.Text,
    'cargo_target_triple' : IDL.Text,
    'cargo_debug' : IDL.Text,
    'pkg_name' : IDL.Text,
    'cargo_features' : IDL.Text,
    'build_timestamp' : IDL.Text,
    'git_sha' : IDL.Text,
    'git_commit_timestamp' : IDL.Text,
  });
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
  const AccessListItem = IDL.Record({
    'storageKeys' : IDL.Vec(IDL.Text),
    'address' : IDL.Text,
  });
  const Transaction = IDL.Record({
    'r' : IDL.Text,
    's' : IDL.Text,
    'v' : IDL.Text,
    'to' : IDL.Opt(IDL.Text),
    'gas' : IDL.Text,
    'maxFeePerGas' : IDL.Opt(IDL.Text),
    'gasPrice' : IDL.Opt(IDL.Text),
    'value' : IDL.Text,
    'blockNumber' : IDL.Opt(IDL.Text),
    'from' : IDL.Text,
    'hash' : IDL.Text,
    'blockHash' : IDL.Opt(IDL.Text),
    'type' : IDL.Opt(IDL.Text),
    'accessList' : IDL.Opt(IDL.Vec(AccessListItem)),
    'transactionIndex' : IDL.Opt(IDL.Text),
    'nonce' : IDL.Text,
    'maxPriorityFeePerGas' : IDL.Opt(IDL.Text),
    'input' : IDL.Text,
    'chainId' : IDL.Opt(IDL.Text),
  });
  const Result_1 = IDL.Variant({
    'Ok' : IDL.Text,
    'Err' : SignatureVerificationError,
  });
  return IDL.Service({
    'add_access' : IDL.Func([IDL.Principal], [Result], []),
    'get_access_list' : IDL.Func([], [IDL.Vec(IDL.Principal)], ['query']),
    'get_canister_build_data' : IDL.Func([], [BuildData], ['query']),
    'get_curr_metrics' : IDL.Func([], [MetricsData], ['query']),
    'get_metrics' : IDL.Func([], [MetricsStorage], ['query']),
    'get_owner' : IDL.Func([], [IDL.Principal], ['query']),
    'remove_access' : IDL.Func([IDL.Principal], [Result], []),
    'set_owner' : IDL.Func([IDL.Principal], [Result], []),
    'verify_signature' : IDL.Func([Transaction], [Result_1], ['query']),
  });
};
export const init = ({ IDL }) => { return [IDL.Vec(IDL.Principal)]; };
