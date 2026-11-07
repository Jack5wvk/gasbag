import "./style.css";
import { BEAT_BLOCKS, EXPLORER, GAS_PRICE_NEAR_PER_TGAS, HATCHERY, MAIN_COIN, MAX_TX_TGAS, NEAR, ONE_TOKEN, SECONDS_PER_BLOCK } from "./config.js";
import { liveChain } from "./chain.js";
import { mockChain } from "./mock.js";
import { drawEcg } from "./ecg.js";
import { breathsLeft, clock, compact, nearText, parseNear, shortAccount, spanText, tokenText, tokensFor } from "./format.js";
import { line } from "./lines.js";
import { petMarkup, puff, setPet } from "./pet.js";

const params = new URLSearchParams(location.search);
const mock = params.get("mock");
const chain = mock ? mockChain(["breathing", "apnea", "dead"].includes(mock) ? mock : "breathing") : liveChain();
const requested = params.get("coin");
const coin = requested && /^[a-z0-9._-]{2,64}$/.test(requested) ? requested : MAIN_COIN;

const $ = (id) => document.getElementById(id);
const set = (id, value) => { $(id).textContent = value; };

let vitals = null;
let lungs = [];
let rules = null;
let account = null;
let wallet = null;
let shownBreaths = null;
let fetchedAt = 0;

$("pet").innerHTML = petMarkup;
$("mock-banner").hidden = chain.live;
set("coin-id", coin);
$("explorer-link").href = `${EXPLORER}/address/${coin}`;
set("cmd-verify", `git clone <this repo> && cd gasbag && npm install\nnode scripts/verify.mjs ${coin}\nnode scripts/status.mjs ${coin}`);

function toast(message, href) {
  const box = $("toast");
  box.replaceChildren(document.createTextNode(message));
  if (href) {
    const link = document.createElement("a");
    link.href = href;
    link.target = "_blank";
    link.rel = "noopener";
    link.textContent = " See it";
    box.appendChild(link);
  }
  box.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => { box.hidden = true; }, 7000);
}

const blockNow = () => (vitals ? Number(vitals.block) + (Date.now() - fetchedAt) / 1000 / SECONDS_PER_BLOCK : 0);

function fullness() {
  if (!vitals) return 1;
  if (vitals.status === "dead") return 0;
  if (lungs.length) {
    const most = Math.max(...lungs.map(([, l]) => l.fuel_tgas));
    return most / MAX_TX_TGAS;
  }
  const left = Number(vitals.deadline_block) - blockNow();
  return Math.max(0, (left / Number(vitals.grace_blocks)) * 0.2);
}

const breathsFor = (fuel) => (rules ? breathsLeft(fuel, Number(vitals.age_blocks), rules) : 0);

function bestFuelBreaths() {
  if (!lungs.length) return 0;
  return Math.max(...lungs.map(([, l]) => breathsFor(l.fuel_tgas)));
}

// Second-by-second clocks, between chain reads.
function tick() {
  if (!vitals) return;
  const now = blockNow();
  const dead = vitals.status === "dead";
  $("clock-label").textContent = dead ? "Time of death" : "Flatline in";
  if (dead) set("clock", spanText(Math.max(0, now - Number(vitals.deadline_block))) + " ago");
  else set("clock", clock((Number(vitals.deadline_block) - now) * SECONDS_PER_BLOCK));
  if (vitals.status === "breathing") {
    const last = Math.max(...lungs.map(([, l]) => l.last_breath_block), Number(vitals.last_breath_block));
    set("next-breath", `about ${clock((last + BEAT_BLOCKS - now) * SECONDS_PER_BLOCK)}`);
  } else set("next-breath", dead ? "never" : "none scheduled");
}

function renderVitals(v) {
  const words = { breathing: "Breathing", apnea: "Not breathing", dead: v.cause === "OldAge" ? "Dead of old age" : "Flatlined" };
  const status = $("status");
  status.dataset.state = v.status;
  status.textContent = words[v.status] ?? v.status;
  set("lung-count", String(v.lungs));
  set("breath-gas", `${v.breath_tgas} TGas`);
  set("age", spanText(v.age_blocks));
  const breathsLeft = bestFuelBreaths();
  set("fuel-left", lungs.length ? `about ${spanText(breathsLeft * BEAT_BLOCKS)}` : "none");

  const bounty = $("bounty");
  bounty.hidden = v.status !== "apnea" || BigInt(v.rescue_bounty) === 0n;
  bounty.textContent = `Rescue bounty: ${tokenText(v.rescue_bounty)} GASBAG for the next lung. It grows while nobody moves. Wait too long and it dies.`;

  set("streak", spanText(v.streak_blocks));
  set("best-streak", spanText(v.best_streak_blocks));
  set("estate", `${nearText(v.estate_left, 3)} NEAR`);
  set("floor", `${nearText(BigInt(v.floor) * 1_000_000n, 3)} NEAR`);
  set("supply", tokenText(v.supply));
  set("wages", tokenText(v.wages));
  set("burned-near", nearText(BigInt(v.gas_burned) * BigInt(Math.round(GAS_PRICE_NEAR_PER_TGAS * 1e12)), 4));

  set("lung-life", spanText(breathsFor(MAX_TX_TGAS) * BEAT_BLOCKS));
  set("lung-cost", `${(MAX_TX_TGAS * GAS_PRICE_NEAR_PER_TGAS).toFixed(2)} NEAR`);
  set("lung-wage", `your share of ${(Number(rules?.wage_bps_per_day ?? 0) / 100).toFixed(2)}% a day`);

  $("ignite").disabled = v.status === "dead";
  $("buy").disabled = v.status === "dead";
  $("claim").disabled = v.status !== "dead";
  $("claim").textContent = v.status === "dead" ? "Claim your share" : "Opens at death";
  $("speech").textContent = line(fullness() < 0.25 && v.status === "breathing" ? "worried" : v.status, shortAccount(v.last_lung, 20));
  updateBuy();
  setPet($("pet"), v.status, fullness());
}

function renderLungs(list) {
  const host = $("lungs");
  host.replaceChildren();
  $("lungs-empty").hidden = list.length > 0;
  for (const [owner, lung] of list) {
    const li = document.createElement("li");
    li.className = "lung";
    const top = document.createElement("div");
    top.className = "lung-top";
    const name = document.createElement("span");
    name.className = "lung-name";
    name.textContent = shortAccount(owner, 34);
    const eta = document.createElement("span");
    eta.className = "lung-eta";
    eta.textContent = `${lung.breaths} breaths, ${spanText(breathsFor(lung.fuel_tgas) * BEAT_BLOCKS)} left`;
    top.append(name, eta);
    const meter = document.createElement("div");
    meter.className = "meter";
    meter.setAttribute("role", "img");
    meter.setAttribute("aria-label", `${lung.fuel_tgas} of ${MAX_TX_TGAS} TGas left`);
    const fill = document.createElement("i");
    fill.style.width = `${Math.min(100, (lung.fuel_tgas / MAX_TX_TGAS) * 100)}%`;
    meter.appendChild(fill);
    li.append(top, meter);
    host.appendChild(li);
  }
}

const KIND = {
  Buy: ["mint", "Bought"], Wage: ["mint", "Wage"], Bounty: ["mint", "Rescue bounty"], Claim: ["burn", "Burned for estate"],
  Ignite: ["", "Lung started"], LastGasp: ["", "Last gasp"], Death: ["burn", "Death"],
};

function renderLog(entries) {
  const body = $("log");
  body.replaceChildren();
  for (const entry of entries) {
    const row = document.createElement("tr");
    const [tone, label] = KIND[entry.kind] ?? ["", entry.kind];
    const kind = document.createElement("td");
    kind.className = `kind ${tone}`;
    kind.textContent = label;
    const who = document.createElement("td");
    who.className = "who";
    who.textContent = shortAccount(entry.who, 24);
    const amount = document.createElement("td");
    amount.className = "amt";
    if (entry.kind === "Death") amount.textContent = `${nearText(entry.amount, 2)} NEAR`;
    else amount.textContent = BigInt(entry.amount) > 0n ? tokenText(entry.amount) : "";
    row.append(kind, who, amount);
    body.appendChild(row);
  }
}

function renderDays(rows) {
  const today = vitals ? Math.floor(Number(vitals.age_blocks) / (rules?.blocks_per_day ?? 144000)) : 0;
  const todayRow = rows.find(([d]) => d === today);
  set("today-breaths", todayRow ? String(todayRow[1].breaths) : "0");

  const host = $("supply-chart");
  host.replaceChildren();
  if (rows.length < 2) {
    const empty = document.createElement("p");
    empty.className = "empty";
    empty.textContent = rows.length ? `Day 1: ${tokenText(rows[0][1].supply_end)} GASBAG. The chart starts on day 2.` : "No supply yet.";
    host.appendChild(empty);
    return;
  }
  const W = 1000, H = 220, L = 58, R = 12, T = 12, B = 26;
  const values = rows.map(([, r]) => Number(BigInt(r.supply_end) / ONE_TOKEN));
  const top = Math.max(...values) * 1.1 || 1;
  const lastDay = rows[rows.length - 1][0] || 1;
  const x = (d) => L + (d / lastDay) * (W - L - R);
  const y = (v) => H - B - (v / top) * (H - T - B);
  const NS = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
  svg.setAttribute("role", "img");
  svg.setAttribute("aria-label", `Supply by day, ending at ${compact(values[values.length - 1])} GASBAG`);
  const add = (tag, attrs, text) => {
    const n = document.createElementNS(NS, tag);
    for (const k in attrs) n.setAttribute(k, attrs[k]);
    if (text !== undefined) n.textContent = text;
    svg.appendChild(n);
    return n;
  };
  for (const v of [0, top / 2, top]) {
    add("line", { x1: L, x2: W - R, y1: y(v), y2: y(v), class: v === 0 ? "axis-line" : "grid-line" });
    add("text", { x: L - 8, y: y(v) + 4, "text-anchor": "end", class: "tick" }, compact(Math.round(v)));
  }
  add("text", { x: L, y: H - 6, class: "tick" }, "day 1");
  add("text", { x: W - R, y: H - 6, "text-anchor": "end", class: "tick" }, `day ${lastDay + 1}`);
  const pts = rows.map(([d], i) => `${x(d).toFixed(1)} ${y(values[i]).toFixed(1)}`);
  add("path", { class: "area", d: `M${pts.join(" L")} L${x(lastDay)} ${y(0)} L${x(rows[0][0])} ${y(0)} Z` });
  add("path", { class: "line", d: `M${pts.join(" L")}` });
  host.appendChild(svg);
}

function renderRules(r) {
  const body = $("rules");
  body.replaceChildren();
  const day = Number(r.blocks_per_day);
  const rows = [
    ["Mint price at birth", `${nearText(BigInt(r.base_price) * 1_000_000n, 2)} NEAR per million`],
    ["Price rises by that much every", `${compact(Number(r.curve_step))} tokens`],
    ["Wages while breathing", `${(r.wage_bps_per_day / 100).toFixed(2)}% of supply a day`],
    ["Gas per breath at birth", `${r.breath_tgas} TGas`],
    ["Doubles, and grace halves, every", spanText(r.doubling_blocks)],
    ["Grace at birth", spanText(r.grace_blocks)],
    ["Grace never below", spanText(r.min_grace_blocks)],
    ["Rescue bounty", `${(r.bounty_bps / 100).toFixed(0)}% of missed wages`],
    ["Dies of old age at", `${r.max_breath_tgas} TGas a breath`],
    ["One coin-day", spanText(day)],
  ];
  for (const [label, value] of rows) {
    const tr = document.createElement("tr");
    const a = document.createElement("td");
    a.textContent = label;
    const b = document.createElement("td");
    b.textContent = value;
    tr.append(a, b);
    body.appendChild(tr);
  }
}

function updateBuy() {
  if (!vitals || !rules) return;
  const spend = parseNear($("buy-near").value);
  set("buy-price", `${nearText(BigInt(vitals.price) * 1_000_000n, 3)} NEAR per 1M`);
  if (spend <= 0n) {
    set("buy-out", "--");
    set("buy-even", "--");
    return;
  }
  const got = tokensFor(spend, vitals.bought, rules);
  set("buy-out", `${tokenText(got)} GASBAG`);
  const after = BigInt(vitals.estate) + spend;
  set("buy-even", `the estate reaches ${nearText(after * 4n, 1)} NEAR`);
}

async function renderUser() {
  if (!account || !vitals) {
    set("my-bag", "--");
    set("my-share", "--");
    set("my-payout", "--");
    $("my-lung").hidden = true;
    return;
  }
  const [balance, payout, lung] = await Promise.all([
    chain.view(coin, "ft_balance_of", { account_id: account }),
    chain.view(coin, "payout_of", { account_id: account }),
    chain.view(coin, "lung_of", { account_id: account }).catch(() => null),
  ]);
  set("my-bag", `${tokenText(balance)} GASBAG`);
  const share = BigInt(vitals.supply) > 0n ? Number((BigInt(balance) * 100000n) / BigInt(vitals.supply)) / 1000 : 0;
  set("my-share", `${share.toFixed(3)}%`);
  set("my-payout", `${nearText(payout, 3)} NEAR`);
  const box = $("my-lung");
  box.hidden = !lung;
  if (lung) box.textContent = `Your lung: ${lung.breaths} breaths, ${lung.fuel_tgas} TGas left.`;
  $("ignite").disabled = vitals.status === "dead" || Boolean(lung);
  $("ignite").textContent = lung ? "Your lung is running" : "Start a lung";
}

let lastLog = [];
let polls = 0;

// Public RPCs rate-limit, so only the vitals are read every time. The heavier reads wait until
// something has changed, or for a periodic catch-up.
async function refresh() {
  try {
    const v = await chain.view(coin, "vitals");
    const changed = !vitals || v.breaths !== vitals.breaths || v.lungs !== vitals.lungs || v.supply !== vitals.supply || v.status !== vitals.status;
    polls += 1;
    if (changed || polls % 5 === 0) {
      const [l, log] = await Promise.all([chain.view(coin, "lungs", {}), chain.view(coin, "log", { limit: 128 })]);
      lungs = l;
      lastLog = log;
      renderLungs(l);
      renderLog(log);
      chain.view(coin, "speech", { limit: 1 }).then((speech) => {
        if (speech.length) $("speech").textContent = speech[0].text;
      }).catch(() => {});
    }
    fetchedAt = Date.now();
    vitals = v;
    $("notice").hidden = true;
    if (shownBreaths !== null && v.breaths !== shownBreaths) puff($("pet"));
    shownBreaths = v.breaths;
    renderVitals(v);
    const beats = lastLog.filter((e) => e.kind === "Wage" || e.kind === "Bounty" || e.kind === "Ignite");
    drawEcg($("ecg"), { block: Number(v.block), beats, status: v.status });
    if (changed) renderUser().catch(() => {});
    tick();
  } catch (error) {
    const box = $("notice");
    box.hidden = false;
    box.textContent = String(error.message ?? error).includes("does not exist") || String(error).includes("UNKNOWN_ACCOUNT")
      ? `${coin} isn't on ${chain.live ? "testnet" : "this preview"} yet.`
      : "Can't reach the chain right now. Retrying.";
  }
}

async function refreshSlow() {
  try {
    renderDays(await chain.view(coin, "days", {}));
  } catch { /* the fast loop reports connection trouble */ }
  try {
    await renderCoins();
  } catch {
    $("coins-empty").hidden = false;
  }
}

async function renderCoins() {
  const coins = await chain.view(HATCHERY, "coins", { limit: 12 });
  const host = $("coins");
  host.replaceChildren();
  $("coins-empty").hidden = coins.length > 0;
  const states = await Promise.all(coins.map((c) => chain.view(c.coin, "vitals").then((v) => v.status).catch(() => "unknown")));
  coins.forEach((c, i) => {
    const li = document.createElement("li");
    li.className = "coin-row";
    const link = document.createElement("a");
    link.href = `?coin=${encodeURIComponent(c.coin)}`;
    link.textContent = `${c.name} ($${c.symbol})`;
    const chip = document.createElement("span");
    chip.className = "chip";
    chip.dataset.state = states[i];
    chip.textContent = states[i];
    li.append(link, chip);
    host.appendChild(li);
  });
}

async function loadOnce() {
  try {
    rules = await chain.view(coin, "rules");
    renderRules(rules);
    const id = await chain.view(coin, "identity");
    set("patient-name", id.name);
    document.title = `${id.name} ${vitals?.status === "dead" ? "(dead)" : ""}`.trim();
    set("soul", id.soul);
    const [keys, info] = await Promise.all([chain.keys(coin), chain.account(coin)]);
    set("proof-keys", keys.length === 0 ? "No access keys. Nobody can change this contract." : `${keys.length} access key(s) still on this account.`);
    $("proof-keys").closest("li").classList.toggle("fail", keys.length > 0);
    set("proof-code", `Code hash ${info.code_hash}`);
  } catch { /* retried on the next reload; the notice covers connection trouble */ }
}

async function needWallet() {
  if (!wallet) wallet = await import("./wallet.js");
  if (!account) {
    await wallet.connect();
    return false;
  }
  return true;
}

function guarded(button, action) {
  button.addEventListener("click", async (event) => {
    event.preventDefault();
    if (!chain.live) return toast("This is a simulated preview. Open the live page to act.");
    try {
      button.disabled = true;
      await action();
    } catch (error) {
      toast(String(error.message ?? error).slice(0, 160));
    } finally {
      button.disabled = false;
      refresh();
    }
  });
}

guarded($("wallet"), async () => {
  if (account) {
    await wallet.disconnect();
    return;
  }
  await needWallet();
});

guarded($("buy"), async () => {
  if (!(await needWallet())) return;
  const spend = parseNear($("buy-near").value);
  if (spend <= 0n) return toast("Enter how much NEAR to spend.");
  const expected = tokensFor(spend, vitals.bought, rules);
  await wallet.send(coin, "buy", { min_tokens: ((expected * 97n) / 100n).toString() }, 60, spend);
  toast("Bought. It's in your bag.");
});

guarded($("ignite"), async () => {
  if (!(await needWallet())) return;
  const registered = await chain.view(coin, "storage_balance_of", { account_id: account });
  if (!registered) {
    const bounds = await chain.view(coin, "storage_balance_bounds");
    await wallet.send(coin, "storage_deposit", {}, 30, BigInt(bounds.min));
  }
  const hash = await wallet.ignite(account, coin);
  toast("Lung started. Its first breath comes in about 2 minutes.", hash ? `${EXPLORER}/txns/${hash}` : undefined);
});

guarded($("claim"), async () => {
  if (!(await needWallet())) return;
  await wallet.send(coin, "claim", {}, 60);
  toast("Claimed. Your share of the estate is on its way.");
});

$("buy-near").addEventListener("input", updateBuy);

$("hatch-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!chain.live) return toast("This is a simulated preview. Open the live page to act.");
  const label = $("hatch-label").value.trim();
  if (!/^[a-z0-9-]{1,24}$/.test(label)) return toast("Use up to 24 lowercase letters, digits and hyphens.");
  try {
    if (!(await needWallet())) return;
    const price = await chain.view(HATCHERY, "hatch_price");
    await wallet.send(HATCHERY, "hatch", { label, name: `Tiny ${label}`.slice(0, 32), symbol: "TINY" }, 200, BigInt(price));
    location.search = `?coin=${encodeURIComponent(`${label}.${HATCHERY}`)}`;
  } catch (error) {
    toast(String(error.message ?? error).slice(0, 160));
  }
});

async function start() {
  await loadOnce();
  await refresh();
  refreshSlow();
  setInterval(tick, 1000);
  setInterval(refresh, 6000);
  setInterval(refreshSlow, 30000);
  if (chain.live) {
    // The wallet code is heavy, so it loads after the monitor is on screen.
    wallet = await import("./wallet.js");
    wallet.onAccount((id) => {
      account = id;
      $("wallet").textContent = id ? shortAccount(id, 20) : "Connect wallet";
      renderUser().catch(() => {});
    });
    wallet.restore();
  }
}

start();
