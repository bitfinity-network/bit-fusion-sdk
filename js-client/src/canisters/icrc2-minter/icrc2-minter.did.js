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
  const LogSettings = IDL.Record({
    'log_filter' : IDL.Opt(IDL.Text),
    'in_memory_records' : IDL.Opt(IDL.Nat64),
    'enable_console' : IDL.Bool,
  });
  const InitData = IDL.Record({
    'evm_principal' : IDL.Principal,
    'signing_strategy' : SigningStrategy,
    'owner' : IDL.Principal,
    'spender_principal' : IDL.Principal,
    'log_settings' : IDL.Opt(LogSettings),
  });
  const Icrc2Burn = IDL.Record({
    'operation_id' : IDL.Nat32,
    'from_subaccount' : IDL.Opt(IDL.Vec(IDL.Nat8)),
    'icrc2_token_principal' : IDL.Principal,
    'recipient_address' : IDL.Text,
    'amount' : IDL.Text,
  });
  const ApproveError = IDL.Variant({
    'GenericError' : IDL.Record({
      'message' : IDL.Text,
      'error_code' : IDL.Nat,
    }),
    'TemporarilyUnavailable' : IDL.Null,
    'Duplicate' : IDL.Record({ 'duplicate_of' : IDL.Nat }),
    'BadFee' : IDL.Record({ 'expected_fee' : IDL.Nat }),
    'AllowanceChanged' : IDL.Record({ 'current_allowance' : IDL.Nat }),
    'CreatedInFuture' : IDL.Record({ 'ledger_time' : IDL.Nat64 }),
    'TooOld' : IDL.Null,
    'Expired' : IDL.Record({ 'ledger_time' : IDL.Nat64 }),
    'InsufficientFunds' : IDL.Record({ 'balance' : IDL.Nat }),
  });
  const TransferFromError = IDL.Variant({
    'GenericError' : IDL.Record({
      'message' : IDL.Text,
      'error_code' : IDL.Nat,
    }),
    'TemporarilyUnavailable' : IDL.Null,
    'InsufficientAllowance' : IDL.Record({ 'allowance' : IDL.Nat }),
    'BadBurn' : IDL.Record({ 'min_burn_amount' : IDL.Nat }),
    'Duplicate' : IDL.Record({ 'duplicate_of' : IDL.Nat }),
    'BadFee' : IDL.Record({ 'expected_fee' : IDL.Nat }),
    'CreatedInFuture' : IDL.Record({ 'ledger_time' : IDL.Nat64 }),
    'TooOld' : IDL.Null,
    'InsufficientFunds' : IDL.Record({ 'balance' : IDL.Nat }),
  });
  const Error = IDL.Variant({
    'Internal' : IDL.Text,
    'InvalidNonce' : IDL.Record({ 'got' : IDL.Nat64, 'minimum' : IDL.Nat64 }),
    'Icrc2ApproveError' : ApproveError,
    'InvalidTokenAddress' : IDL.Null,
    'BftBridgeAlreadyRegistered' : IDL.Text,
    'Icrc2TransferFromError' : TransferFromError,
    'NotAuthorized' : IDL.Null,
    'AnonymousPrincipal' : IDL.Null,
    'BftBridgeDoesNotExist' : IDL.Null,
    'JsonRpcCallFailed' : IDL.Text,
    'InsufficientOperationPoints' : IDL.Record({
      'got' : IDL.Nat32,
      'expected' : IDL.Nat32,
    }),
    'InvalidBftBridgeContract' : IDL.Null,
    'InvalidBurnOperation' : IDL.Text,
  });
  const Result = IDL.Variant({ 'Ok' : IDL.Nat32, 'Err' : Error });
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
  const Result_1 = IDL.Variant({ 'Ok' : IDL.Text, 'Err' : Error });
  const Log = IDL.Record({ 'log' : IDL.Text, 'offset' : IDL.Nat64 });
  const Logs = IDL.Record({
    'logs' : IDL.Vec(Log),
    'all_logs_count' : IDL.Nat64,
  });
  const Result_2 = IDL.Variant({ 'Ok' : Logs, 'Err' : Error });
  const Result_3 = IDL.Variant({ 'Ok' : IDL.Null, 'Err' : Error });
  const HttpHeader = IDL.Record({ 'value' : IDL.Text, 'name' : IDL.Text });
  const HttpResponse = IDL.Record({
    'status' : IDL.Nat,
    'body' : IDL.Vec(IDL.Nat8),
    'headers' : IDL.Vec(HttpHeader),
  });
  const TransformArgs = IDL.Record({
    'context' : IDL.Vec(IDL.Nat8),
    'response' : HttpResponse,
  });
  return IDL.Service({
    'burn_icrc2' : IDL.Func([Icrc2Burn], [Result], []),
    'get_bft_bridge_contract' : IDL.Func([], [IDL.Opt(IDL.Text)], ['query']),
    'get_canister_build_data' : IDL.Func([], [BuildData], ['query']),
    'get_curr_metrics' : IDL.Func([], [MetricsData], ['query']),
    'get_evm_principal' : IDL.Func([], [IDL.Principal], ['query']),
    'get_metrics' : IDL.Func([], [MetricsStorage], ['query']),
    'get_mint_order' : IDL.Func(
        [IDL.Vec(IDL.Nat8), IDL.Vec(IDL.Nat8), IDL.Nat32],
        [IDL.Opt(IDL.Vec(IDL.Nat8))],
        ['query'],
      ),
    'get_minter_canister_evm_address' : IDL.Func([], [Result_1], []),
    'get_owner' : IDL.Func([], [IDL.Principal], ['query']),
    'ic_logs' : IDL.Func([IDL.Nat64, IDL.Nat64], [Result_2], []),
    'list_mint_orders' : IDL.Func(
        [IDL.Vec(IDL.Nat8), IDL.Vec(IDL.Nat8)],
        [IDL.Vec(IDL.Tuple(IDL.Nat32, IDL.Vec(IDL.Nat8)))],
        ['query'],
      ),
    'register_evmc_bft_bridge' : IDL.Func([IDL.Text], [Result_3], []),
    'set_evm_principal' : IDL.Func([IDL.Principal], [Result_3], []),
    'set_logger_filter' : IDL.Func([IDL.Text], [Result_3], []),
    'set_owner' : IDL.Func([IDL.Principal], [Result_3], []),
    'transform' : IDL.Func([TransformArgs], [HttpResponse], ['query']),
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
  const LogSettings = IDL.Record({
    'log_filter' : IDL.Opt(IDL.Text),
    'in_memory_records' : IDL.Opt(IDL.Nat64),
    'enable_console' : IDL.Bool,
  });
  const InitData = IDL.Record({
    'evm_principal' : IDL.Principal,
    'signing_strategy' : SigningStrategy,
    'owner' : IDL.Principal,
    'spender_principal' : IDL.Principal,
    'log_settings' : IDL.Opt(LogSettings),
  });
  return [InitData];
};
