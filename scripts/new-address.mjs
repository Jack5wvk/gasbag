#!/usr/bin/env node
// Generates a key on this computer and prints the NEAR address it controls. Nothing touches the
// network: the address starts to exist on chain when someone first sends it NEAR, for example from
// https://near-faucet.io. The secret goes into the NEAR CLI keychain and is never printed.
//
//   node scripts/new-address.mjs
import { mkdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { KeyPair } from "near-api-js";
import { NETWORK } from "./lib.mjs";

const pair = KeyPair.fromRandom("ed25519");
const publicKey = pair.getPublicKey();
const address = Buffer.from(publicKey.data).toString("hex");

const folder = join(homedir(), ".near-credentials", NETWORK);
mkdirSync(folder, { recursive: true });
writeFileSync(
  join(folder, `${address}.json`),
  JSON.stringify({ account_id: address, public_key: publicKey.toString(), private_key: pair.toString() }),
  { mode: 0o600 },
);
console.log(address);
