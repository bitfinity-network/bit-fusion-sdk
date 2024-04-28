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
  const RuneInfo = IDL.Record({
    'tx' : IDL.Nat32,
    'name' : IDL.Text,
    'block' : IDL.Nat64,
  });
  const BitcoinNetwork = IDL.Variant({
    'mainnet' : IDL.Null,
    'regtest' : IDL.Null,
    'testnet' : IDL.Null,
  });
  const LogSettings = IDL.Record({
    'log_filter' : IDL.Opt(IDL.Text),
    'in_memory_records' : IDL.Opt(IDL.Nat64),
    'enable_console' : IDL.Bool,
  });
  const RuneBridgeConfig = IDL.Record({
    'admin' : IDL.Principal,
    'signing_strategy' : SigningStrategy,
    'indexer_url' : IDL.Text,
    'evm_link' : EvmLink,
    'rune_info' : RuneInfo,
    'network' : BitcoinNetwork,
    'min_confirmations' : IDL.Nat32,
    'log_settings' : LogSettings,
    'deposit_fee' : IDL.Nat64,
  });
  const BftBridgeConfig = IDL.Record({
    'decimals' : IDL.Nat8,
    'token_symbol' : IDL.Vec(IDL.Nat8),
    'token_address' : IDL.Text,
    'bridge_address' : IDL.Text,
    'erc20_chain_id' : IDL.Nat32,
    'token_name' : IDL.Vec(IDL.Nat8),
  });
  const CreateEdictTxArgs = IDL.Record({
    'destination' : IDL.Text,
    'rune_name' : IDL.Text,
    'change_address' : IDL.Opt(IDL.Text),
    'from_address' : IDL.Text,
    'amount' : IDL.Nat,
  });
  const Erc20MintStatus = IDL.Variant({
    'Minted' : IDL.Record({ 'tx_id' : IDL.Text, 'amount' : IDL.Nat }),
    'Scheduled' : IDL.Record({
      'required_confirmations' : IDL.Nat32,
      'pending_utxos' : IDL.Opt(IDL.Vec(IDL.Record({}))),
      'current_confirmations' : IDL.Nat32,
    }),
    'Signed' : IDL.Vec(IDL.Nat8),
  });
  const DepositError = IDL.Variant({
    'Evm' : IDL.Text,
    'Sign' : IDL.Text,
    'NoRunesToDeposit' : IDL.Null,
    'NotingToDeposit' : IDL.Null,
    'NotInitialized' : IDL.Null,
    'NotEnoughBtc' : IDL.Record({
      'minimum' : IDL.Nat64,
      'received' : IDL.Nat64,
    }),
    'Unavailable' : IDL.Text,
    'Pending' : IDL.Record({
      'current_confirmations' : IDL.Nat32,
      'min_confirmations' : IDL.Nat32,
    }),
  });
  const Result = IDL.Variant({ 'Ok' : Erc20MintStatus, 'Err' : DepositError });
  const MetricsData = IDL.Record({
    'stable_memory_size' : IDL.Nat64,
    'cycles' : IDL.Nat64,
    'heap_memory_size' : IDL.Nat64,
  });
  const GetAddressError = IDL.Variant({ 'Derivation' : IDL.Null });
  const Result_1 = IDL.Variant({ 'Ok' : IDL.Text, 'Err' : GetAddressError });
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
    'admin_configure_bft_bridge' : IDL.Func([BftBridgeConfig], [], []),
    'admin_configure_ecdsa' : IDL.Func([], [], []),
    'create_edict_tx' : IDL.Func([CreateEdictTxArgs], [IDL.Vec(IDL.Nat8)], []),
    'deposit' : IDL.Func([IDL.Text], [Result], []),
    'get_curr_metrics' : IDL.Func([], [MetricsData], ['query']),
    'get_deposit_address' : IDL.Func([IDL.Text], [Result_1], ['query']),
    'get_evm_address' : IDL.Func([], [IDL.Opt(IDL.Text)], []),
    'get_metrics' : IDL.Func([], [MetricsStorage], ['query']),
    'get_rune_balances' : IDL.Func(
        [IDL.Text],
        [IDL.Vec(IDL.Tuple(IDL.Text, IDL.Nat))],
        [],
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
  const RuneInfo = IDL.Record({
    'tx' : IDL.Nat32,
    'name' : IDL.Text,
    'block' : IDL.Nat64,
  });
  const BitcoinNetwork = IDL.Variant({
    'mainnet' : IDL.Null,
    'regtest' : IDL.Null,
    'testnet' : IDL.Null,
  });
  const LogSettings = IDL.Record({
    'log_filter' : IDL.Opt(IDL.Text),
    'in_memory_records' : IDL.Opt(IDL.Nat64),
    'enable_console' : IDL.Bool,
  });
  const RuneBridgeConfig = IDL.Record({
    'admin' : IDL.Principal,
    'signing_strategy' : SigningStrategy,
    'indexer_url' : IDL.Text,
    'evm_link' : EvmLink,
    'rune_info' : RuneInfo,
    'network' : BitcoinNetwork,
    'min_confirmations' : IDL.Nat32,
    'log_settings' : LogSettings,
    'deposit_fee' : IDL.Nat64,
  });
  return [RuneBridgeConfig];
};
