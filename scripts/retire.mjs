#!/usr/bin/env node
// Deletes a throwaway test account and returns its NEAR. This only works on an account launched
// with --keep-key, because a locked coin has no key and can never be retired by anyone.
//
//   node scripts/retire.mjs <account> --to <beneficiary>
import { Account, JsonRpcProvider, actions } from "near-api-js";
import { RPC, argv, secretKeyFor } from "./lib.mjs";

const { positional, flags } = argv();
const id = positional[0];
if (!id || !flags.to) {
  console.error("usage: node scripts/retire.mjs <account> --to <beneficiary>");
  process.exit(1);
}
const account = new Account(id, new JsonRpcProvider({ url: RPC }), secretKeyFor(id));
const result = await account.signAndSendTransaction({ receiverId: id, actions: [actions.deleteAccount(flags.to)] });
console.log(`Retired ${id}, funds sent to ${flags.to} in ${result.transaction.hash}`);
