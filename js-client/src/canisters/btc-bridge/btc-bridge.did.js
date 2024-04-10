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
  const BtcBridgeConfig = IDL.Record({
    'admin' : IDL.Principal,
    'signing_strategy' : SigningStrategy,
    'ck_btc_ledger_fee' : IDL.Nat64,
    'evm_link' : EvmLink,
    'ck_btc_minter' : IDL.Principal,
    'network' : BitcoinNetwork,
    'ck_btc_ledger' : IDL.Principal,
    'log_settings' : LogSettings,
  });
  const BftBridgeConfig = IDL.Record({
    'decimals' : IDL.Nat8,
    'token_symbol' : IDL.Vec(IDL.Nat8),
    'token_address' : IDL.Text,
    'bridge_address' : IDL.Text,
    'erc20_chain_id' : IDL.Nat32,
    'token_name' : IDL.Vec(IDL.Nat8),
  });
  const OutPoint = IDL.Record({
    'txid' : IDL.Vec(IDL.Nat8),
    'vout' : IDL.Nat32,
  });
  const PendingUtxo = IDL.Record({
    'confirmations' : IDL.Nat32,
    'value' : IDL.Nat64,
    'outpoint' : OutPoint,
  });
  const Erc20MintStatus = IDL.Variant({
    'Minted' : IDL.Record({ 'tx_id' : IDL.Text, 'amount' : IDL.Nat64 }),
    'Scheduled' : IDL.Record({
      'required_confirmations' : IDL.Nat32,
      'pending_utxos' : IDL.Opt(IDL.Vec(PendingUtxo)),
      'current_confirmations' : IDL.Nat32,
    }),
    'Signed' : IDL.Vec(IDL.Nat8),
  });
  const UpdateBalanceError = IDL.Variant({
    'GenericError' : IDL.Record({
      'error_message' : IDL.Text,
      'error_code' : IDL.Nat64,
    }),
    'TemporarilyUnavailable' : IDL.Text,
    'AlreadyProcessing' : IDL.Null,
    'NoNewUtxos' : IDL.Record({
      'required_confirmations' : IDL.Nat32,
      'pending_utxos' : IDL.Opt(IDL.Vec(PendingUtxo)),
      'current_confirmations' : IDL.Opt(IDL.Nat32),
    }),
  });
  const Utxo = IDL.Record({
    'height' : IDL.Nat32,
    'value' : IDL.Nat64,
    'outpoint' : OutPoint,
  });
  const TransferError = IDL.Variant({
    'GenericError' : IDL.Record({
      'message' : IDL.Text,
      'error_code' : IDL.Nat,
    }),
    'TemporarilyUnavailable' : IDL.Null,
    'BadBurn' : IDL.Record({ 'min_burn_amount' : IDL.Nat }),
    'Duplicate' : IDL.Record({ 'duplicate_of' : IDL.Nat }),
    'BadFee' : IDL.Record({ 'expected_fee' : IDL.Nat }),
    'CreatedInFuture' : IDL.Record({ 'ledger_time' : IDL.Nat64 }),
    'TooOld' : IDL.Null,
    'InsufficientFunds' : IDL.Record({ 'balance' : IDL.Nat }),
  });
  const Erc20MintError = IDL.Variant({
    'Evm' : IDL.Text,
    'CkBtcMinter' : UpdateBalanceError,
    'ValueTooSmall' : IDL.Null,
    'Tainted' : Utxo,
    'Sign' : IDL.Text,
    'CkBtcLedger' : TransferError,
    'NotInitialized' : IDL.Null,
    'NothingToMint' : IDL.Null,
  });
  const Result = IDL.Variant({
    'Ok' : Erc20MintStatus,
    'Err' : Erc20MintError,
  });
  const GetBtcAddressArgs = IDL.Record({
    'owner' : IDL.Opt(IDL.Principal),
    'subaccount' : IDL.Opt(IDL.Vec(IDL.Nat8)),
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
  return IDL.Service({
    'admin_configure_bft_bridge' : IDL.Func([BftBridgeConfig], [], []),
    'btc_to_erc20' : IDL.Func([IDL.Text], [IDL.Vec(Result)], []),
    'get_bft_bridge_contract' : IDL.Func([], [IDL.Opt(IDL.Text)], ['query']),
    'get_btc_address' : IDL.Func([GetBtcAddressArgs], [IDL.Text], []),
    'get_curr_metrics' : IDL.Func([], [MetricsData], ['query']),
    'get_evm_address' : IDL.Func([], [IDL.Opt(IDL.Text)], []),
    'get_metrics' : IDL.Func([], [MetricsStorage], ['query']),
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
  const BtcBridgeConfig = IDL.Record({
    'admin' : IDL.Principal,
    'signing_strategy' : SigningStrategy,
    'ck_btc_ledger_fee' : IDL.Nat64,
    'evm_link' : EvmLink,
    'ck_btc_minter' : IDL.Principal,
    'network' : BitcoinNetwork,
    'ck_btc_ledger' : IDL.Principal,
    'log_settings' : LogSettings,
  });
  return [BtcBridgeConfig];
};
