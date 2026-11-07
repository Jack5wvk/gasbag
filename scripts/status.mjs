#!/usr/bin/env node
// Prints a coin's vitals. Read-only: needs no key, no account and no wallet.
//
//   node scripts/status.mjs <coin>
import { argv, minutes, near, view, whole } from "./lib.mjs";

const coin = argv().positional[0];
if (!coin) {
  console.error("usage: node scripts/status.mjs <coin>");
  process.exit(1);
}

const v = await view(coin, "vitals");
const lungs = await view(coin, "lungs", {});
const left = Number(v.deadline_block) - Number(v.block);

console.log(`${coin}: ${v.status.toUpperCase()}${v.cause ? ` (${v.cause})` : ""}`);
console.log(`  age            ${minutes(v.age_blocks)}, old age in ${minutes(Number(v.old_age_block) - Number(v.block))}`);
console.log(`  breaths        ${v.breaths}, last at block ${v.last_breath_block}, now ${v.block}`);
console.log(`  flatline in    ${left > 0 ? minutes(left) : "passed"}`);
console.log(`  breath costs   ${v.breath_tgas} TGas, grace ${minutes(v.grace_blocks)}`);
console.log(`  streak         ${minutes(v.streak_blocks)}, best ${minutes(v.best_streak_blocks)}`);
console.log(`  supply         ${whole(v.supply)} GASBAG (${whole(v.bought)} bought, ${whole(v.wages)} wages, ${whole(v.burned)} burned)`);
console.log(`  estate         ${near(v.estate_left)} NEAR, floor ${near(v.floor)} NEAR per token`);
console.log(`  price          ${near(v.price)} NEAR per token`);
console.log(`  rescue bounty  ${whole(v.rescue_bounty)} GASBAG`);
console.log(`  gas burned     ${near(BigInt(v.gas_burned) * 100_000_000n)} NEAR by breaths`);
console.log(`  lungs          ${v.lungs}`);
for (const [owner, lung] of lungs) {
  const breathsLeft = Math.floor(lung.fuel_tgas / Number(v.breath_tgas));
  console.log(`    ${owner}: ${lung.breaths} breaths, ${lung.fuel_tgas} TGas left, about ${breathsLeft} more`);
}
