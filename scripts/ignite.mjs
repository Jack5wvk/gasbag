#!/usr/bin/env node
// Starts a lung. The transaction stays open for as long as the lung lives, so this submits it and
// returns as soon as it is included. Waiting for it to finish would take hours.
//
//   node scripts/ignite.mjs <coin> --as <account> [--tgas 1000]
import { Account, JsonRpcProvider, actions } from "near-api-js";
import { EXPLORER, RPC, TGAS, argv, secretKeyFor, view } from "./lib.mjs";

const { positional, flags } = argv();
const coin = positional[0];
const signer = flags.as;
if (!coin || !signer) {
  console.error("usage: node scripts/ignite.mjs <coin> --as <account> [--tgas 1000]");
  process.exit(1);
}
const tgas = BigInt(flags.tgas ?? 1000);

const registered = await view(coin, "storage_balance_of", { account_id: signer });
if (!registered) {
  console.error(`${signer} is not registered on ${coin}. Buy any amount first, or call storage_deposit.`);
  process.exit(1);
}

const account = new Account(signer, new JsonRpcProvider({ url: RPC }), secretKeyFor(signer));
const result = await account.signAndSendTransaction({
  receiverId: coin,
  actions: [actions.functionCall("ignite", {}, tgas * TGAS, 0n)],
  waitUntil: "INCLUDED",
  throwOnFailure: false,
});
const hash = result.transaction?.hash ?? result.transaction_outcome?.id;
console.log(`Lung sent with ${tgas} TGas.`);
if (hash) console.log(`See it: ${EXPLORER}/txns/${hash}`);
console.log(`Check it: node scripts/status.mjs ${coin}`);
