import { Actor, HttpAgent } from '@dfinity/agent';

// Imports and re-exports candid interface
import { idlFactory } from './icrc2-minter.did.js';
export { idlFactory } from './icrc2-minter.did.js';
import { ICRC2_MINTER_CANISTER_ID } from '../../constants';

const canisterId = ICRC2_MINTER_CANISTER_ID;
const host = process.env.IC_HOST;

export const createActor = (canisterId, options = {}) => {
  const agent = options.agent || new HttpAgent({ ...options.agentOptions });

  if (options.agent && options.agentOptions) {
    console.warn(
      'Detected both agent and agentOptions passed to createActor. Ignoring agentOptions and proceeding with the provided agent.'
    );
  }

  // Fetch root key for certificate validation during development
  if (process.env.DFX_NETWORK !== 'ic') {
    agent.fetchRootKey().catch((err) => {
      console.warn(
        'Unable to fetch root key. Check to ensure that your local replica is running'
      );
      console.error(err);
    });
  }

  // Creates an actor with using the candid interface and the HttpAgent
  return Actor.createActor(idlFactory, {
    agent,
    canisterId,
    ...options.actorOptions
  });
};

export const ICRC2Minter = canisterId
  ? createActor(canisterId, host ? { agentOptions: { host } } : undefined)
  : undefined;
