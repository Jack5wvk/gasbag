#!/usr/bin/env node
// Checks a live coin against what the README claims. Read-only: needs no key, no account and no
// wallet. Exits non-zero if any check fails.
//
//   node scripts/verify.mjs <coin>
import { accessKeys, accountInfo, argv, minutes, near, view } from "./lib.mjs";

const coin = argv().positional[0];
if (!coin) {
  console.error("usage: node scripts/verify.mjs <coin>");
  process.exit(1);
}

const BEAT = 200;
let failed = 0;
function check(name, pass, detail = "") {
  console.log(`${pass ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
  if (!pass) failed++;
}
function note(name, detail) {
  console.log(`INFO  ${name}: ${detail}`);
}

const [meta, supply, rules, vitals, identity, info, keys, lungs, log, days] = await Promise.all([
  view(coin, "ft_metadata"),
  view(coin, "ft_total_supply"),
  view(coin, "rules"),
  view(coin, "vitals"),
  view(coin, "identity"),
  accountInfo(coin),
  accessKeys(coin),
  view(coin, "lungs", {}),
  view(coin, "log", { limit: 128 }),
  view(coin, "days", {}),
]);

console.log(`Checking ${coin}\n`);

// The token standard.
check("NEP-148 metadata", meta.spec === "ft-1.0.0" && meta.decimals === 18, `${meta.symbol}, ${meta.decimals} decimals`);
check("NEP-141 supply matches vitals", supply === vitals.supply, supply);

// No admin path: the track's central rule.
check("No access keys on the coin", keys.length === 0, `${keys.length} key(s)`);
for (const key of keys) note("Key still on the account", key.public_key);
note("Code hash", info.code_hash);
const required = [
  "base_price", "curve_step", "wage_bps_per_day", "blocks_per_day", "doubling_blocks", "grace_blocks",
  "min_grace_blocks", "bounty_bps", "breath_tgas", "max_breath_tgas",
];
check("Every parameter is public", required.every((k) => k in rules), required.length + " parameters");
check("The prompt behind its voice is on chain", typeof identity.soul === "string", `${identity.soul.length} bytes`);

// The keeper-free heartbeat.
const now = Number(vitals.block);
if (vitals.status === "breathing") {
  const overdue = now - Number(vitals.last_breath_block);
  check("Breathing on schedule", overdue <= 2 * BEAT, `last breath ${overdue} blocks ago, a beat is ${BEAT}`);
  check("Lungs have fuel on record", lungs.every(([, lung]) => lung.fuel_tgas > 0), `${lungs.length} lung(s)`);
} else {
  note("Not breathing right now", vitals.status);
}
const beats = log.filter((e) => e.kind === "Wage").map((e) => e.block).sort((a, b) => a - b);
if (beats.length > 2 && lungs.length <= 1) {
  const gaps = beats.slice(1).map((b, i) => b - beats[i]).filter((g) => g < 3 * BEAT);
  if (gaps.length) {
    const mean = gaps.reduce((a, b) => a + b, 0) / gaps.length;
    check("Beats arrive about every 200 blocks", Math.abs(mean - BEAT) < 20, `mean gap ${mean.toFixed(1)} blocks over ${gaps.length} beats`);
  }
}

// Money.
check("Curve price is at least the base price", BigInt(vitals.price) >= BigInt(rules.base_price));
if (BigInt(vitals.wages) === 0n) {
  check("Floor is at least half the price, before wages", BigInt(vitals.floor) * 2n >= BigInt(vitals.price) || vitals.supply === "0");
} else {
  // Measured against everything ever minted, so a coin whose holders have all claimed still works.
  const minted = BigInt(vitals.bought) + BigInt(vitals.wages);
  note("Wage dilution so far", `${Number((BigInt(vitals.wages) * 1000n) / minted) / 10}% of all tokens minted`);
}
check("The contract holds enough NEAR for the estate", BigInt(info.amount) + BigInt(info.locked) >= BigInt(vitals.estate_left), `${near(vitals.estate_left)} NEAR owed`);
check("Supply equals bought plus wages minus burned",
  BigInt(vitals.supply) === BigInt(vitals.bought) + BigInt(vitals.wages) - BigInt(vitals.burned));

// The log the track asks for.
const kinds = [...new Set(log.map((e) => e.kind))].join(", ");
check("The contract keeps its own log", Array.isArray(log), `${log.length} recent entries: ${kinds || "none yet"}`);
check("Daily table present", Array.isArray(days), `${days.length} day(s)`);
note("Streak", `${minutes(vitals.streak_blocks)} now, ${minutes(vitals.best_streak_blocks)} best`);

console.log(failed ? `\n${failed} check(s) failed.` : "\nEverything matches.");
process.exit(failed ? 1 : 0);
