#!/usr/bin/env node
// Registers a job on the pacemaker. You must be the target contract or its parent account.
//
//   node scripts/schedule.mjs <pacemaker> --as <account> --target <contract> --method tick [--every 1] [--call-tgas 30] [--tgas 1000]
import { Account, JsonRpcProvider, actions } from "near-api-js";
import { EXPLORER, NEAR, RPC, TGAS, argv, secretKeyFor } from "./lib.mjs";

const { positional, flags } = argv();
const pacemaker = positional[0];
if (!pacemaker || !flags.as || !flags.target || !flags.method) {
  console.error("usage: node scripts/schedule.mjs <pacemaker> --as <account> --target <contract> --method <name> [--every 1] [--call-tgas 30] [--tgas 1000]");
  process.exit(1);
}

const account = new Account(flags.as, new JsonRpcProvider({ url: RPC }), secretKeyFor(flags.as));
const args = {
  target: flags.target,
  method: flags.method,
  args: Buffer.from(flags.args ?? "{}").toString("base64"),
  every_beats: Number(flags.every ?? 1),
  call_tgas: Number(flags["call-tgas"] ?? 30),
};
const result = await account.signAndSendTransaction({
  receiverId: pacemaker,
  actions: [actions.functionCall("schedule", args, BigInt(flags.tgas ?? 1000) * TGAS, NEAR / 50n)],
  waitUntil: "INCLUDED",
  throwOnFailure: false,
});
console.log(`Job sent: ${EXPLORER}/txns/${result.transaction?.hash}`);
console.log(`Read it: node scripts/status-job.mjs ${pacemaker}`);
