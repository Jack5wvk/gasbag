import { BEAT_BLOCKS, NEAR, ONE_TOKEN, SECONDS_PER_BLOCK } from "./config.js";

/** yoctoNEAR to a readable NEAR figure. */
export function nearText(yocto, digits = 4) {
  const value = Number(BigInt(yocto) / 10n ** 16n) / 1e8;
  if (value === 0) return "0";
  if (value < 10 ** -digits) return `<${(10 ** -digits).toFixed(digits)}`;
  return value.toLocaleString("en-US", { maximumFractionDigits: value >= 100 ? 1 : digits });
}

/** Token units to a compact figure such as 12.3M. */
export function tokenText(units) {
  const value = Number(BigInt(units) / (ONE_TOKEN / 1000n)) / 1000;
  return compact(value);
}

export function compact(value) {
  const abs = Math.abs(value);
  if (abs >= 1e9) return `${(value / 1e9).toFixed(2)}B`;
  if (abs >= 1e6) return `${(value / 1e6).toFixed(2)}M`;
  if (abs >= 1e4) return `${(value / 1e3).toFixed(1)}K`;
  return value.toLocaleString("en-US", { maximumFractionDigits: abs >= 100 ? 0 : 2 });
}

/** A span of blocks as words: 11 h, 42 min, 3 days. */
export function spanText(blocks) {
  const seconds = Math.max(0, Number(blocks)) * SECONDS_PER_BLOCK;
  if (seconds < 90) return `${Math.round(seconds)} s`;
  if (seconds < 5400) return `${Math.round(seconds / 60)} min`;
  if (seconds < 172800) return `${(seconds / 3600).toFixed(1)} h`;
  return `${(seconds / 86400).toFixed(1)} days`;
}

/** A countdown clock from seconds. */
export function clock(seconds) {
  const s = Math.max(0, Math.floor(seconds));
  const pad = (n) => String(n).padStart(2, "0");
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return h >= 100 ? `${Math.floor(h / 24)} days` : `${h ? `${pad(h)}:` : ""}${pad(m)}:${pad(s % 60)}`;
}

export function shortAccount(id, max = 26) {
  if (!id) return "";
  return id.length <= max ? id : `${id.slice(0, 10)}…${id.slice(-10)}`;
}

/** Tokens bought by `near` yoctoNEAR at the current curve position. Mirrors contracts/gasbag/src/math.rs. */
export function tokensFor(near, bought, rules) {
  const step = BigInt(rules.curve_step) * ONE_TOKEN;
  const q = BigInt(bought) + step;
  const inner = q * q + (2n * step * BigInt(near) * ONE_TOKEN) / BigInt(rules.base_price);
  return isqrt(inner) - q;
}

function isqrt(value) {
  if (value < 2n) return value;
  let x = BigInt(Math.floor(Math.sqrt(Number(value))));
  for (;;) {
    const y = (x + value / x) >> 1n;
    if (y === x || y === x + 1n || y === x - 1n) {
      x = y;
      break;
    }
    x = y;
  }
  while (x * x > value) x -= 1n;
  while ((x + 1n) * (x + 1n) <= value) x += 1n;
  return x;
}

/** Gas per breath at an age. Mirrors `doubled` in contracts/gasbag/src/math.rs, in TGas. */
export function breathAt(ageBlocks, rules) {
  const period = Number(rules.doubling_blocks);
  const whole = Math.floor(ageBlocks / period);
  return Number(rules.breath_tgas) * 2 ** whole * (1 + (ageBlocks % period) / period);
}

/** How many more breaths a lung's fuel pays for. Each one costs more than the last as the coin ages. */
export function breathsLeft(fuelTgas, ageBlocks, rules) {
  let fuel = fuelTgas;
  let age = ageBlocks;
  let breaths = 0;
  while (breaths < 2000) {
    age += BEAT_BLOCKS;
    const cost = breathAt(age, rules);
    if (fuel < cost) break;
    fuel -= cost;
    breaths += 1;
  }
  return breaths;
}

export const parseNear = (text) => {
  const [whole = "0", fraction = ""] = String(text).trim().split(".");
  if (!/^\d*$/.test(whole) || !/^\d*$/.test(fraction)) return 0n;
  return BigInt(whole || "0") * NEAR + BigInt(fraction.padEnd(24, "0").slice(0, 24) || "0");
};
