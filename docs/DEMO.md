# Demo

Call the script to deploy all the canisters locally:

```sh
./scripts/dfx/deploy_local.sh create
```

Setup a minimal gas price:

```sh
dfx canister call evmc admin_set_min_gas_price '("10")'
```

Get a balance by the current identity:

```sh
dfx canister call token icrc1_balance_of "(record {owner=principal \"$(dfx identity get-principal)\"; subaccount=null})"
```

Get an account information:

```sh
dfx canister call evmc account_basic '("0x0000000000000000000000000000000000000002")'
```

Use a deposit account:

```sh
dfx canister call token icrc1_transfer '(record {to=record {owner = principal "rrkah-fqaaa-aaaaa-aaaaq-cai"; subaccount=opt blob "\1di\fb\8b\c5\c0\a6\00{\e2\99T\b0\00i\f99z\86\ef\df//\81\b7\b5\9e\b2\ec\02\00\00"}; fee=null; memo=null; from_subaccount=null; created_at_time=null;amount=1_100_000_000})'

dfx canister call token icrc1_balance_of '(record {owner = principal "rrkah-fqaaa-aaaaa-aaaaq-cai"; subaccount=opt blob "\1di\fb\8b\c5\c0\a6\00{\e2\99T\b0\00i\f99z\86\ef\df//\81\b7\b5\9e\b2\ec\02\00\00"})'
```
