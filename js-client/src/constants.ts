import canistersLocal from '../../.dfx/local/canister_ids.json';

const canisters: Record<string, Record<string, string>> = canistersLocal;

export const RPC_URL = process.env.RPC_URL || 'http://127.0.0.1:8545';

export const LOCAL_TEST_SEED_PHRASE =
  process.env.LOCAL_TEST_SEED_PHRASE ||
  'piece cabin metal credit library hobby fetch nature topple region nominee always';

export const IC_HOST = process.env.IC_HOST || 'http://127.0.0.1:4943';

export const ICRC2_MINTER_CANISTER_ID =
  process.env.ICRC2_MINTER_CANISTER_ID || canisters['icrc2-minter']?.local;

export const ICRC2_TOKEN_CANISTER_ID =
  process.env.ICRC2_TOKEN_CANISTER_ID || canisters.token2?.local;

export const BTC_BRIDGE_CANISTER_ID =
  process.env.BTC_BRIDGE_CANISTER_ID || canisters['btc-bridge']?.local;

export const CKBTC_TOKEN_CANISTER_ID =
  process.env.CKBTC_TOKEN_CANISTER_ID || canisters.token?.local;

export const CK_BTC_CANISTER_ID =
  process.env.CK_BTC_CANISTER_ID || canisters['btc-bridge']?.local;

export const CHAIN_ID = process.env.CHAIN_ID || 355113;

export const IS_TEST = process.env.IS_TEST || false;

export const BTC_BRIDGE_ETH_ADDRESS = process.env.BTC_BRIDGE_ETH_ADDRESS;

export const BFT_ETH_ADDRESS = process.env.BFT_ETH_ADDRESS;

export const BTC_TOKEN_WRAPPED_ADDRESS = process.env.BTC_TOKEN_WRAPPED_ADDRESS;
