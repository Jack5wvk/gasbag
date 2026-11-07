// Addresses of the live deployment. Change them here after a new launch.
export const NETWORK = "testnet";
export const RPC_URLS = ["https://test.rpc.fastnear.com", "https://rpc.testnet.fastnear.com", "https://near-testnet.drpc.org"];
export const EXPLORER = "https://testnet.nearblocks.io";
export const MAIN_COIN = "coin.gasbag-hq.testnet";
export const HATCHERY = "hatchery.gasbag-hq.testnet";

export const SECONDS_PER_BLOCK = 0.6;
export const BEAT_BLOCKS = 200;
export const MAX_TX_TGAS = 1000;
export const GAS_PRICE_NEAR_PER_TGAS = 0.0001;
export const TGAS = 1_000_000_000_000n;
export const NEAR = 10n ** 24n;
export const HELD_PER_TGAS = NEAR / 1000n; // Prepaid gas is held at 10x the burn price, then refunded.
export const STORAGE_PRICE_PER_BYTE = 10n ** 19n;
export const ONE_TOKEN = 10n ** 18n;
