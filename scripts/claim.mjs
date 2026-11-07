#!/usr/bin/env node
// The will. After death, burns all your tokens for your share of the estate.
//
//   node scripts/claim.mjs <coin> --as <account>
import { Account, JsonRpcProvider, actions } from "near-api-js";
import { RPC, TGAS, argv, near, secretKeyFor } from "./lib.mjs";

const { positional, flags } = argv();
const coin = positional[0];
if (!coin || !flags.as) {
  console.error("usage: node scripts/claim.mjs <coin> --as <account>");
  process.exit(1);
}
const account = new Account(flags.as, new JsonRpcProvider({ url: RPC }), secretKeyFor(flags.as));
const result = await account.signAndSendTransaction({
  receiverId: coin,
  actions: [actions.functionCall("claim", {}, 60n * TGAS, 0n)],
});
const value = result.status?.SuccessValue;
const paid = value ? JSON.parse(Buffer.from(value, "base64").toString()) : "0";
console.log(`Paid ${near(paid)} NEAR from the estate in ${result.transaction.hash}`);
