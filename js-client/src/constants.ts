import dotenv from 'dotenv';
import fs from 'fs';
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const canisters = JSON.parse(
  fs
    .readFileSync(
      `${dirname(fileURLToPath(import.meta.url))}/../../.dfx/local/canister_ids.json`
    )
    .toString()
);

dotenv.config();

export const RPC_URL = process.env.RPC_URL || 'http://127.0.0.1:8545';

export const LOCAL_TEST_SEED_PHRASE =
  process.env.LOCAL_TEST_SEED_PHRASE ||
  'piece cabin metal credit library hobby fetch nature topple region nominee always';

export const IC_HOST = process.env.IC_HOST || 'http://127.0.0.1:4943';

export const ICRC2_MINTER_CANISTER_ID =
  process.env.ICRC2_MINTER_CANISTER_ID || canisters['icrc2-minter']?.local;

export const ICRC2_TOKEN_CANISTER_ID =
  process.env.ICRC2_TOKEN_CANISTER_ID || canisters.token2?.local;

export const BTC_BRIDGE_CANISTER_ID = process.env.BTC_BRIDGE_CANISTER_ID || '';

export const IS_TEST = true;
