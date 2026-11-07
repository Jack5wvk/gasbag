#!/usr/bin/env node
// Gives the coin its voice. Reads the prompt and vitals from the contract, asks NEAR AI Cloud for
// one line, and stores the line on chain with a reference to the TEE signature over the response.
// This is an optional layer: if it never runs, every rule still works.
//
//   NEARAI_API_KEY=... node scripts/speak.mjs <coin> --as <voice account> [--model <id>]
import { Account, JsonRpcProvider, actions } from "near-api-js";
import { RPC, TGAS, argv, minutes, near, secretKeyFor, view, whole } from "./lib.mjs";

const API = "https://cloud-api.near.ai/v1";
const { positional, flags } = argv();
const coin = positional[0];
const voice = flags.as;
const key = process.env.NEARAI_API_KEY;
if (!coin || !voice || !key) {
  console.error("usage: NEARAI_API_KEY=... node scripts/speak.mjs <coin> --as <voice account> [--model <id>]");
  process.exit(1);
}
const headers = { authorization: `Bearer ${key}`, "content-type": "application/json" };

const [identity, vitals, lungs] = await Promise.all([view(coin, "identity"), view(coin, "vitals"), view(coin, "lungs", {})]);
const situation = [
  `Status: ${vitals.status}${vitals.cause ? ` (${vitals.cause})` : ""}.`,
  `Age: ${minutes(vitals.age_blocks)}. Flatline in ${minutes(Math.max(0, vitals.deadline_block - vitals.block))}.`,
  `A breath costs ${vitals.breath_tgas} TGas. ${vitals.breaths} breaths so far.`,
  `Streak ${minutes(vitals.streak_blocks)}, best ${minutes(vitals.best_streak_blocks)}.`,
  `Estate ${near(vitals.estate_left)} NEAR. Supply ${whole(vitals.supply)}.`,
  lungs.length ? `Kept alive by: ${lungs.map(([owner]) => owner).join(", ")}.` : "Nobody is keeping you alive.",
].join(" ");

let model = flags.model;
if (!model) {
  const list = await (await fetch(`${API}/models`, { headers })).json();
  model = list.data?.[0]?.id;
}
const chat = await (await fetch(`${API}/chat/completions`, {
  method: "POST",
  headers,
  body: JSON.stringify({
    model,
    max_tokens: 120,
    messages: [
      { role: "system", content: identity.soul },
      { role: "user", content: `${situation} Say one line.` },
    ],
  }),
})).json();
const text = chat.choices?.[0]?.message?.content?.trim().slice(0, 280);
if (!text) {
  console.error("No line came back:", JSON.stringify(chat).slice(0, 300));
  process.exit(1);
}

// The signature is made inside the TEE, so anyone can check this line came from attested hardware.
let proof = null;
try {
  const sig = await (await fetch(`${API}/signature/${chat.id}?model=${encodeURIComponent(model)}&signing_algo=ecdsa`, { headers })).json();
  if (sig.signature) proof = `nearai:${chat.id}:${sig.signing_address}:${sig.signature}`.slice(0, 300);
} catch {
  // A missing proof just means the page shows the line as unverified.
}

const account = new Account(voice, new JsonRpcProvider({ url: RPC }), secretKeyFor(voice));
await account.signAndSendTransaction({
  receiverId: coin,
  actions: [actions.functionCall("say", { text, proof }, 30n * TGAS, 0n)],
});
console.log(`Said: ${text}`);
console.log(proof ? "With a TEE signature reference." : "No signature reference was available.");
