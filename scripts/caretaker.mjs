#!/usr/bin/env node
// Keeps a coin fuelled. This is an ordinary participant with no special powers: it signs the same
// `ignite` anyone can send, and if it stops, the coin simply lives or dies by its rules.
//
//   node scripts/caretaker.mjs <coin> --as nurse-a.testnet,nurse-b.testnet [--hours 2] [--watch]
//
// One account can run one lung, so two accounts let a new lung overlap the old one and keep the
// streak unbroken. Keys come from the NEAR CLI keychain, or from a CARETAKER_KEYS environment
// variable holding {"account.testnet":"ed25519:..."} for CI.
import { Account, JsonRpcProvider, actions } from "near-api-js";
import { BEAT, EXPLORER, RPC, TGAS, argv, secretKeyFor, view } from "./lib.mjs";

const { positional, flags } = argv();
const coin = positional[0];
const nurses = String(flags.as ?? "").split(",").filter(Boolean);
if (!coin || !nurses.length) {
  console.error("usage: node scripts/caretaker.mjs <coin> --as a.testnet,b.testnet [--hours 2] [--watch]");
  process.exit(1);
}
const minHours = Number(flags.hours ?? 2);
const keys = process.env.CARETAKER_KEYS ? JSON.parse(process.env.CARETAKER_KEYS) : {};
const provider = new JsonRpcProvider({ url: RPC });

async function round() {
  const vitals = await view(coin, "vitals");
  if (vitals.status === "dead") {
    console.log(`${coin} is dead. Nothing to do.`);
    return false;
  }
  const lungs = await view(coin, "lungs", {});
  const perBreath = Number(vitals.breath_tgas);
  const hoursLeft = (fuel) => (Math.floor(fuel / perBreath) * BEAT * 0.6) / 3600;
  const best = lungs.length ? Math.max(...lungs.map(([, l]) => hoursLeft(l.fuel_tgas))) : 0;
  const stamp = new Date().toISOString();
  if (best >= minHours) {
    console.log(`${stamp} fine: ${best.toFixed(1)} h of fuel left across ${lungs.length} lung(s)`);
    return true;
  }
  const busy = new Set(lungs.map(([owner]) => owner));
  const nurse = nurses.find((n) => !busy.has(n));
  if (!nurse) {
    console.log(`${stamp} low on fuel, but every nurse already has a lung running`);
    return true;
  }
  const account = new Account(nurse, provider, keys[nurse] ?? secretKeyFor(nurse));
  const result = await account.signAndSendTransaction({
    receiverId: coin,
    actions: [actions.functionCall("ignite", {}, 1000n * TGAS, 0n)],
    waitUntil: "INCLUDED",
    throwOnFailure: false,
  });
  console.log(`${stamp} started a lung as ${nurse}: ${EXPLORER}/txns/${result.transaction?.hash}`);
  return true;
}

if (flags.watch) {
  for (;;) {
    try {
      if (!(await round())) break;
    } catch (error) {
      console.error(new Date().toISOString(), error.message);
    }
    await new Promise((resolve) => setTimeout(resolve, 60_000));
  }
} else {
  await round();
}
