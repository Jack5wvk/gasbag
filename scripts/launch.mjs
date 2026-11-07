#!/usr/bin/env node
// Launches a contract on a new sub-account of `--parent`, then removes the sub-account's key, so
// nobody can change it afterwards. All the key handling happens here and nothing is printed.
//
//   node scripts/launch.mjs coin      --parent gasbag-hq.testnet --name coin --preset main
//   node scripts/launch.mjs coin      --parent gasbag-hq.testnet --name run1 --preset speedrun
//   node scripts/launch.mjs pacemaker --parent gasbag-hq.testnet --name pacemaker
//   node scripts/launch.mjs hatchery  --parent gasbag-hq.testnet --name hatchery
//
// Add --keep-key to skip locking, for a throwaway test only.
import { readFileSync } from "node:fs";
import { Account, JsonRpcProvider, KeyPair, actions } from "near-api-js";
import { EXPLORER, NEAR, RPC, TGAS, accessKeys, argv, secretKeyFor } from "./lib.mjs";

const SOUL =
  "You are Gasbag, a talkative balloon on life support. You run on NEAR gas and you are always nearly dead. " +
  "You are dramatic, funny and a little needy. You know your vitals and you name the people keeping you alive. " +
  "Never give financial advice and never promise anyone a profit. Keep it under 200 characters.";

const NEAR_PER_TOKEN_START = (NEAR / 1_000_000n).toString();

const PRESETS = {
  main: {
    name: "Gasbag", symbol: "GASBAG", soul: SOUL,
    rules: {
      base_price: NEAR_PER_TOKEN_START, curve_step: "50000000",
      wage_bps_per_day: 50, blocks_per_day: 144_000,
      doubling_blocks: 1_008_000, grace_blocks: 36_000, min_grace_blocks: 400,
      bounty_bps: 5_000, breath_tgas: 3, max_breath_tgas: 990,
      voice_account: null, voice_bps: 0,
    },
  },
  // About 15 minutes from birth to old age.
  speedrun: {
    name: "Tiny Gasbag", symbol: "TINY", soul: "You are a very small balloon with about fifteen minutes to live. You know it.",
    rules: {
      base_price: NEAR_PER_TOKEN_START, curve_step: "50000000",
      wage_bps_per_day: 500, blocks_per_day: 600,
      doubling_blocks: 300, grace_blocks: 500, min_grace_blocks: 400,
      bounty_bps: 5_000, breath_tgas: 30, max_breath_tgas: 990,
      voice_account: null, voice_bps: 0,
    },
  },
};

const { positional, flags } = argv();
const kind = positional[0];
const parentId = flags.parent;
const label = flags.name;
if (!["coin", "pacemaker", "hatchery"].includes(kind) || !parentId || !label) {
  console.error("usage: node scripts/launch.mjs <coin|pacemaker|hatchery> --parent <account> --name <label> [--preset main|speedrun] [--keep-key]");
  process.exit(1);
}

const wasmFile = flags.wasm ?? new URL(`../contracts/res/${kind === "coin" ? "gasbag" : kind}.wasm`, import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const wasm = readFileSync(wasmFile);
const id = `${label}.${parentId}`;
// Code plus room for state. A coin keeps a log, a daily table and speech; the others keep little.
const reserve = BigInt(wasm.length + (kind === "coin" ? 60_000 : 15_000)) * 10n ** 19n;

const preset = PRESETS[flags.preset ?? "main"];
if (kind === "coin" && flags.voice) {
  preset.rules.voice_account = flags.voice;
  preset.rules.voice_bps = 100;
}
const init = kind === "coin" ? { method: "new", args: preset } : { method: "new", args: {} };

const provider = new JsonRpcProvider({ url: RPC });
const parent = new Account(parentId, provider, secretKeyFor(parentId));
const keyPair = KeyPair.fromRandom();
const publicKey = keyPair.getPublicKey();

console.log(`Creating ${id} with ${wasm.length} bytes of code...`);
const result = await parent.signAndSendTransaction({
  receiverId: id,
  actions: [
    actions.createAccount(),
    actions.transfer(reserve),
    actions.addFullAccessKey(publicKey),
    actions.deployContract(wasm),
    actions.functionCall(init.method, init.args, 30n * TGAS, 0n),
  ],
});
console.log(`Launched: ${EXPLORER}/address/${id} in ${result.transaction.hash}`);

if (flags["keep-key"]) {
  console.log("Key kept. This account is NOT locked.");
  process.exit(0);
}

const self = new Account(id, provider, keyPair.toString());
await self.signAndSendTransaction({ receiverId: id, actions: [actions.deleteKey(publicKey)] });
const left = await accessKeys(id);
if (left.length !== 0) {
  console.error(`Lock failed: ${left.length} key(s) remain on ${id}.`);
  process.exit(1);
}
console.log(`Locked: ${id} has no access keys. Nobody can change it now.`);
