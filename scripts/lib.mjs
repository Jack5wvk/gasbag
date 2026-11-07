// Shared helpers for the GASBAG scripts. Reads use plain JSON-RPC, so `verify` and `status` need
// no key and no account. Keys for the scripts that sign are read from the legacy NEAR CLI
// keychain (~/.near-credentials) and are never printed.
import { readFileSync, existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

export const NETWORK = process.env.NEAR_NETWORK ?? "testnet";
export const RPC = process.env.NEAR_RPC ?? `https://rpc.${NETWORK}.fastnear.com`;
// Public RPCs rate-limit, so reads fall back to a second endpoint.
const FALLBACK = NETWORK === "testnet" ? "https://test.rpc.fastnear.com" : "https://free.rpc.fastnear.com";
export const EXPLORER = NETWORK === "mainnet" ? "https://nearblocks.io" : "https://testnet.nearblocks.io";
export const BEAT = 200;
export const TGAS = 1_000_000_000_000n;
export const NEAR = 10n ** 24n;
export const ONE_TOKEN = 10n ** 18n;

export function argv() {
  const positional = [];
  const flags = {};
  const raw = process.argv.slice(2);
  for (let i = 0; i < raw.length; i++) {
    if (raw[i].startsWith("--")) {
      const key = raw[i].slice(2);
      const next = raw[i + 1];
      if (next === undefined || next.startsWith("--")) flags[key] = true;
      else flags[key] = raw[++i];
    } else positional.push(raw[i]);
  }
  return { positional, flags };
}

export async function rpc(method, params) {
  let lastError;
  for (const url of [RPC, FALLBACK, RPC]) {
    try {
      const response = await fetch(url, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: "gasbag", method, params }),
      });
      const body = await response.json();
      if (body.error) throw new Error(JSON.stringify(body.error));
      if (body.result?.error) throw new Error(body.result.error);
      return body.result;
    } catch (error) {
      lastError = error;
      await new Promise((resolve) => setTimeout(resolve, 800));
    }
  }
  throw lastError;
}

export async function view(contract, method, args = {}) {
  const result = await rpc("query", {
    request_type: "call_function",
    finality: "optimistic",
    account_id: contract,
    method_name: method,
    args_base64: Buffer.from(JSON.stringify(args)).toString("base64"),
  });
  return JSON.parse(Buffer.from(result.result).toString());
}

export const accountInfo = (id) => rpc("query", { request_type: "view_account", finality: "final", account_id: id });

export const accessKeys = async (id) =>
  (await rpc("query", { request_type: "view_access_key_list", finality: "final", account_id: id })).keys;

export function secretKeyFor(accountId) {
  const file = join(homedir(), ".near-credentials", NETWORK, `${accountId}.json`);
  if (!existsSync(file)) {
    throw new Error(`No key for ${accountId} at ${file}. Create the account with the NEAR CLI first.`);
  }
  return JSON.parse(readFileSync(file, "utf8")).private_key;
}

export const near = (yocto) => (Number(BigInt(yocto) / 10n ** 18n) / 1e6).toLocaleString("en-US", { maximumFractionDigits: 6 });

export const whole = (units) => Number(BigInt(units) / ONE_TOKEN).toLocaleString("en-US");

export function minutes(blocks, secondsPerBlock = 0.6) {
  const s = Number(blocks) * secondsPerBlock;
  if (s < 90) return `${Math.round(s)} s`;
  if (s < 5400) return `${Math.round(s / 60)} min`;
  if (s < 172800) return `${(s / 3600).toFixed(1)} h`;
  return `${(s / 86400).toFixed(1)} days`;
}
