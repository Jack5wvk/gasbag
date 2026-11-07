#!/usr/bin/env node
// Buys GASBAG on the curve with test NEAR, and registers the buyer if needed.
//
//   node scripts/buy.mjs <coin> --as <account> --near 0.5
import { Account, JsonRpcProvider, actions } from "near-api-js";
import { NEAR, RPC, TGAS, argv, secretKeyFor, whole } from "./lib.mjs";

const { positional, flags } = argv();
const coin = positional[0];
if (!coin || !flags.as || !flags.near) {
  console.error("usage: node scripts/buy.mjs <coin> --as <account> --near <amount>");
  process.exit(1);
}
const deposit = BigInt(Math.round(Number(flags.near) * 1e6)) * (NEAR / 1_000_000n);
const account = new Account(flags.as, new JsonRpcProvider({ url: RPC }), secretKeyFor(flags.as));
const result = await account.signAndSendTransaction({
  receiverId: coin,
  actions: [actions.functionCall("buy", {}, 60n * TGAS, deposit)],
});
const value = result.status?.SuccessValue;
const got = value ? JSON.parse(Buffer.from(value, "base64").toString()) : "0";
console.log(`Bought ${whole(got)} tokens for ${flags.near} NEAR in ${result.transaction.hash}`);
