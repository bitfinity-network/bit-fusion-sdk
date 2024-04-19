export const idlFactory = ({ IDL }) => {
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
  const Result = IDL.Variant({ 'Ok' : IDL.Nat, 'Err' : TransferFromError });
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
    'finish_icrc2_mint' : IDL.Func(
        [IDL.Principal, IDL.Principal, IDL.Vec(IDL.Nat8), IDL.Nat, IDL.Nat],
        [Result],
        [],
      ),
    'get_curr_metrics' : IDL.Func([], [MetricsData], ['query']),
    'get_metrics' : IDL.Func([], [MetricsStorage], ['query']),
  });
};
export const init = ({ IDL }) => { return [IDL.Principal]; };
