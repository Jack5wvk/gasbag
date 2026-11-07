// A pretend chain for previewing the page with `?mock=breathing`, `?mock=apnea` or `?mock=dead`.
// The page shows a banner whenever this is active, so it can't be mistaken for live data.
import { BEAT_BLOCKS, NEAR, ONE_TOKEN } from "./config.js";

const RULES = {
  base_price: (NEAR / 1_000_000n).toString(), curve_step: "50000000", wage_bps_per_day: 50, blocks_per_day: 144000,
  doubling_blocks: 1008000, grace_blocks: 36000, min_grace_blocks: 400, bounty_bps: 5000, breath_tgas: 5,
  max_breath_tgas: 990, voice_account: null, voice_bps: 0,
};

const tokens = (n) => (BigInt(n) * ONE_TOKEN).toString();

export function mockChain(state) {
  const start = 276_700_000;
  const opened = Date.now();
  const block = () => start + Math.floor((Date.now() - opened) / 600);
  const dead = state === "dead";
  const breathing = state === "breathing";
  const beats = () => {
    const now = block();
    const list = [];
    const offset = (now + 60) % BEAT_BLOCKS;
    if (!breathing) return list;
    for (let i = 0; i < 24; i++) list.push(now - offset - i * BEAT_BLOCKS);
    return list;
  };

  const views = {
    rules: () => RULES,
    identity: () => ({ name: "Gasbag", symbol: "GASBAG", soul: "You are Gasbag, a talkative balloon on life support." }),
    ft_metadata: () => ({ spec: "ft-1.0.0", name: "Gasbag", symbol: "GASBAG", decimals: 18 }),
    ft_balance_of: () => tokens(1_250_000),
    storage_balance_of: () => ({ total: "5000000000000000000000", available: "0" }),
    storage_balance_bounds: () => ({ min: "5000000000000000000000", max: "5000000000000000000000" }),
    payout_of: () => (NEAR * 3n) / 2n,
    speech: () => [],
    death: () => (dead ? { cause: "Flatline", block: block() - 900, breaths: "4312", best_streak_blocks: "612000", last_lung: "maya.testnet", supply: tokens(312_000_000), estate: (NEAR * 1052n).toString() } : null),
    vitals: () => {
      const now = block();
      return {
        status: state, cause: dead ? "Flatline" : null, block: String(now), born_block: String(start - 1_600_000),
        age_blocks: "1600000", old_age_block: String(start + 6_750_000),
        last_breath_block: String(breathing ? now - ((now + 60) % BEAT_BLOCKS) : now - 7_000),
        deadline_block: String(breathing ? now + 9_000 : dead ? now - 900 : now + 5_400),
        grace_blocks: "12600", breath_tgas: "15", lungs: breathing ? 3 : 0,
        last_lung: "maya.testnet", breaths: String(4312 + Math.floor((now - start) / BEAT_BLOCKS)),
        streak_blocks: breathing ? "412000" : "0", best_streak_blocks: "612000",
        supply: tokens(312_000_000), bought: tokens(278_000_000), wages: tokens(34_000_000), burned: "0",
        lost: tokens(2_100_000), rescue_bounty: breathing ? "0" : tokens(184_000),
        estate: (NEAR * 1052n).toString(), estate_left: (NEAR * 1052n).toString(),
        price: (NEAR * 66n / 10_000_000n).toString(), floor: (NEAR * 34n / 10_000_000n).toString(),
        voice_budget: "0", gas_burned: "412000000000000000", holders: "87",
      };
    },
    lungs: () => breathing ? [
      ["maya.testnet", { id: 91, started_block: start - 8000, last_breath_block: block(), breaths: 40, fuel_tgas: 610, paid_index: "0", paid_total: tokens(180_000) }],
      ["night-shift.testnet", { id: 92, started_block: start - 4000, last_breath_block: block(), breaths: 20, fuel_tgas: 820, paid_index: "0", paid_total: tokens(90_000) }],
      ["3f1a9c0e2b7d4c88a1e6f00d9b1c7e23aa4410fd9e6c2b5d7f81c3e9a0b4d612", { id: 93, started_block: start - 900, last_breath_block: block(), breaths: 4, fuel_tgas: 960, paid_index: "0", paid_total: tokens(12_000) }],
    ] : [],
    log: () => {
      const entries = beats().map((b, i) => ({ n: 9000 - i, block: b, kind: "Wage", who: ["maya.testnet", "night-shift.testnet"][i % 2], amount: tokens(1200) }));
      entries.splice(3, 0, { n: 8990, block: block() - 900, kind: "Buy", who: "tomas.testnet", amount: tokens(420_000) });
      entries.splice(7, 0, { n: 8980, block: block() - 1800, kind: "Ignite", who: "maya.testnet", amount: "0" });
      if (!breathing) entries.unshift({ n: 9100, block: block() - 7000, kind: "LastGasp", who: "maya.testnet", amount: "0" });
      if (dead) entries.unshift({ n: 9200, block: block() - 900, kind: "Death", who: "maya.testnet", amount: (NEAR * 1052n).toString() }, { n: 9201, block: block() - 400, kind: "Claim", who: "tomas.testnet", amount: tokens(420_000) });
      return entries.sort((a, b) => b.block - a.block);
    },
    days: () => Array.from({ length: 12 }, (_, d) => [d, {
      breaths: 700, bought: tokens(22_000_000), near_in: (NEAR * 80n).toString(), wages: tokens(1_400_000),
      burned: "0", gas: "2200000000000000", supply_end: tokens(26_000_000 * (d + 1)),
    }]),
    coins: () => [
      { coin: "run3.hatchery.gasbag-hq.testnet", hatcher: "tomas.testnet", name: "Tiny Gasbag", symbol: "TINY", block: start - 900 },
      { coin: "run2.hatchery.gasbag-hq.testnet", hatcher: "maya.testnet", name: "Short Fuse", symbol: "FUSE", block: start - 9000 },
    ],
  };

  return {
    live: false,
    async view(_contract, method, args) {
      if (!views[method]) throw new Error(`no mock for ${method}`);
      return views[method](args);
    },
    keys: async () => [],
    account: async () => ({ code_hash: "zmzEm862wTcnENmKihPMWSZTjBwrCwwHGD4vBCpYn2z", amount: "0", locked: "0" }),
  };
}
