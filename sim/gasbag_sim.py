#!/usr/bin/env python3
"""
GASBAG 60-day economy simulator (standard library only).

Mirrors the proposed contract rules:

  BREATHE  One transaction attaches a "lung" (up to 1000 TGas). The lung wakes itself
           every 200 blocks (about 2 minutes) until its gas runs out. No keeper.
  GET PAID While at least one lung is alive, wage_pct_day % of supply per day is minted
           to the lungs, split equally per lung. One lung per account.
  BUY IN   Anyone mints on a linear bonding curve. 100% of the NEAR goes to the estate.
           There is no sell-back and no withdraw path while it lives.
  AGE      Gas per breath doubles every doubling_days. Grace halves on the same clock.
           When one breath no longer fits in one transaction (990 TGas, leaving room
           for the call itself), it dies of old age.
  APNEA    No lungs: wages stop. bounty_share of the missed wages becomes a rescue
           bounty, paid to whoever restarts it. The rest is never minted.
  WILL     No breath for the grace period: dead. Anyone burns GASBAG for a pro-rata
           share of the estate.

Protocol constants come from the live testnet/mainnet config read on 2026-11-07
(protocol version 88): yield timeout 200 blocks, max prepaid gas 1 PGas, gas price
0.0001 NEAR per TGas, about 0.6 s per block. Agent behaviour is invented for stress
testing. It is not a forecast of what real people would do.
"""
from __future__ import annotations

import heapq
import json
import math
import os
import random
import statistics
import sys
from dataclasses import asdict, dataclass, field
from typing import Callable, Dict, List, Optional

# Protocol facts (verified against EXPERIMENTAL_protocol_config, protocol 88).
BREATH_MINUTES = 2.0            # 200 blocks at about 0.6 s
GAS_PRICE_NEAR_PER_TGAS = 1e-4  # 100 Mgas-price: 1 TGas costs 0.0001 NEAR
MAX_TX_TGAS = 1000.0            # max_total_prepaid_gas = 1 PGas
MAX_BREATH_TGAS = 990.0         # the contract's old-age limit, which leaves room for the call itself
STEP_MINUTES = 10.0
BREATHS_PER_STEP = STEP_MINUTES / BREATH_MINUTES
STEPS_PER_DAY = int(24 * 60 / STEP_MINUTES)
HORIZON_DAYS = 60
IGNITE_OVERHEAD_TGAS = 2.0      # gas of the ignite call itself, before the first breath


@dataclass(frozen=True)
class Params:
    name: str = "launch"
    p0: float = 1e-6              # NEAR per GASBAG at Q = 0
    k: float = 2e-14              # curve slope, NEAR per GASBAG per GASBAG
    doubling_days: float = 7.0    # gas per breath doubles and grace halves on this clock
    wage_pct_day: float = 0.5     # % of supply minted per day to lungs
    grace_hours: float = 6.0      # grace at birth
    min_grace_minutes: float = 4.0
    bounty_share: float = 0.5     # share of missed wages that becomes the bounty
    breath0_tgas: float = 3.0     # gas per breath at birth


@dataclass
class Owner:
    name: str
    kind: str = "holder"
    tokens: float = 0.0
    paid_near: float = 0.0        # NEAR spent on the curve
    bought_tokens: float = 0.0
    gas_near: float = 0.0         # NEAR burned as gas
    wage_tokens: float = 0.0
    bounty_tokens: float = 0.0


@dataclass
class Lung:
    owner: str
    fuel_tgas: float


@dataclass
class Purchase:
    day: float
    near: float
    tokens: float
    estate_before: float
    price_after: float


class World:
    def __init__(self, p: Params, seed: int):
        self.p = p
        self.rng = random.Random(seed)
        self.q = 0.0                # tokens minted on the curve
        self.wages = 0.0            # tokens minted as wages and bounties
        self.estate = 0.0
        self.pot = 0.0
        self.lost = 0.0             # wages never minted because nobody was breathing
        self.lungs: Dict[str, Lung] = {}
        self.owners: Dict[str, Owner] = {}
        self.purchases: List[Purchase] = []
        self.step = 0
        self.last_breath_step = 0
        self.streak_start = 0
        self.best_streak_steps = 0
        self.gas_near = 0.0
        self.dead = False
        self.death_cause: Optional[str] = None
        self.death_day: Optional[float] = None
        self.apnea_steps = 0
        self.apnea_events = 0
        self.rescues = 0
        self.series: List[dict] = []
        self.breathing_prev = False
        self.heartbeats = 0
        self.born = False

    # Time and ageing -------------------------------------------------------------
    @property
    def day(self) -> float:
        return self.step / STEPS_PER_DAY

    @property
    def supply(self) -> float:
        return self.q + self.wages

    def _doublings(self):
        whole = int(self.day // self.p.doubling_days)
        return whole, (self.day % self.p.doubling_days) / self.p.doubling_days

    def breath_tgas(self) -> float:
        # Mirrors `doubled` in contracts/gasbag/src/math.rs: doubles on schedule, and rises in a
        # straight line between doublings.
        whole, part = self._doublings()
        return self.p.breath0_tgas * 2 ** whole * (1 + part)

    def grace_steps(self) -> float:
        # Mirrors `halved`: halves on schedule, and falls in a straight line between halvings.
        whole, part = self._doublings()
        minutes = self.p.grace_hours * 60 / 2 ** whole * (1 - part / 2)
        return max(minutes, self.p.min_grace_minutes) / STEP_MINUTES

    def floor(self) -> float:
        return self.estate / self.supply if self.supply > 0 else 0.0

    def price(self) -> float:
        return self.p.p0 + self.p.k * self.q

    def owner(self, name: str, kind: str = "holder") -> Owner:
        if name not in self.owners:
            self.owners[name] = Owner(name, kind)
        return self.owners[name]

    # Actions ---------------------------------------------------------------------
    def buy(self, name: str, near: float, kind: str = "holder") -> float:
        if self.dead or near <= 0:
            return 0.0
        p = self.p
        a = p.p0 + p.k * self.q
        tokens = (-a + math.sqrt(a * a + 2 * p.k * near)) / p.k
        before = self.estate
        self.q += tokens
        self.estate += near
        o = self.owner(name, kind)
        o.tokens += tokens
        o.bought_tokens += tokens
        o.paid_near += near
        self.purchases.append(Purchase(self.day, near, tokens, before, self.price()))
        return tokens

    def ignite(self, name: str, fuel_tgas: float = MAX_TX_TGAS, kind: str = "nurse") -> bool:
        if self.dead or name in self.lungs:
            return False
        if self.breath_tgas() + IGNITE_OVERHEAD_TGAS > fuel_tgas:
            return False
        if not self.lungs and self.step - self.last_breath_step > 1:
            self.streak_start = self.step
        self.lungs[name] = Lung(name, fuel_tgas - IGNITE_OVERHEAD_TGAS)
        o = self.owner(name, kind)
        o.gas_near += IGNITE_OVERHEAD_TGAS * GAS_PRICE_NEAR_PER_TGAS
        self.gas_near += IGNITE_OVERHEAD_TGAS * GAS_PRICE_NEAR_PER_TGAS
        return True

    # One 10-minute step ----------------------------------------------------------
    def tick(self) -> None:
        if self.dead:
            return
        breath = self.breath_tgas()
        if breath >= MAX_BREATH_TGAS:
            self.die("old age")
            return

        wage_total = self.supply * (self.p.wage_pct_day / 100.0) / STEPS_PER_DAY
        expired = []
        any_breath = False
        for name, lung in self.lungs.items():
            burned = min(lung.fuel_tgas, breath * BREATHS_PER_STEP)
            whole_breaths = math.floor(burned / breath)
            if whole_breaths > 0:
                any_breath = True
                self.heartbeats += whole_breaths
                gas = whole_breaths * breath
                lung.fuel_tgas -= gas
                cost = gas * GAS_PRICE_NEAR_PER_TGAS
                self.owners[name].gas_near += cost
                self.gas_near += cost
            if lung.fuel_tgas < breath:
                expired.append(name)

        if any_breath and self.lungs:
            share = wage_total / len(self.lungs)
            for name in self.lungs:
                o = self.owners[name]
                o.tokens += share
                o.wage_tokens += share
            self.wages += wage_total
            if self.pot > 0:
                rescuer = self.rng.choice(list(self.lungs))
                o = self.owners[rescuer]
                o.tokens += self.pot
                o.bounty_tokens += self.pot
                self.wages += self.pot
                self.pot = 0.0
                self.rescues += 1
            self.last_breath_step = self.step
        else:
            self.pot += wage_total * self.p.bounty_share
            self.lost += wage_total * (1 - self.p.bounty_share)
            self.apnea_steps += 1

        if self.breathing_prev and not any_breath:
            self.apnea_events += 1
            self.best_streak_steps = max(self.best_streak_steps, self.step - self.streak_start)
        self.breathing_prev = any_breath

        for name in expired:
            del self.lungs[name]

        # The coin is only born at its first breath, so nothing can flatline before that.
        self.born = self.born or any_breath
        if self.born and self.step - self.last_breath_step > self.grace_steps():
            self.die("flatline")

    def die(self, cause: str) -> None:
        self.dead = True
        self.death_cause = cause
        self.death_day = self.day
        self.best_streak_steps = max(self.best_streak_steps, self.step - self.streak_start)
        self.lungs.clear()

    def record(self) -> None:
        self.series.append({
            "day": round(self.day, 3),
            "supply": self.supply,
            "bought": self.q,
            "wages": self.wages,
            "estate": self.estate,
            "floor": self.floor(),
            "price": self.price(),
            "lungs": len(self.lungs),
            "gas_near": self.gas_near,
            "breath_tgas": self.breath_tgas(),
            "alive": not self.dead,
        })


# Agents ---------------------------------------------------------------------------

class Agent:
    """Wakes at intervals and may act."""
    interval_hours = 1.0
    jitter = 0.5

    def __init__(self, name: str):
        self.name = name

    def next_delay_steps(self, rng: random.Random) -> int:
        base = self.interval_hours * 6
        return max(1, int(base * rng.uniform(1 - self.jitter, 1 + self.jitter)))

    def act(self, w: World) -> None:
        raise NotImplementedError


class DevotedNurse(Agent):
    """A person who shows up on a rhythm and pays gas up to a daily budget."""

    def __init__(self, name, interval_hours=10.0, budget_near_day=1.0, quit_hazard_day=0.0):
        super().__init__(name)
        self.interval_hours = interval_hours
        self.budget = budget_near_day
        self.quit_hazard_day = quit_hazard_day
        self.quit = False

    def act(self, w):
        if self.quit:
            return
        if w.rng.random() < self.quit_hazard_day * self.interval_hours / 24:
            self.quit = True
            return
        if self.name in w.lungs:
            return
        cost_day = 720 * w.breath_tgas() * GAS_PRICE_NEAR_PER_TGAS
        if cost_day <= self.budget:
            w.ignite(self.name, kind="devoted")


class RationalNurse(Agent):
    """Ignites only if its share of wages, at floor value, beats the gas."""

    def __init__(self, name, interval_hours=0.5, premium=1.0):
        super().__init__(name)
        self.interval_hours = interval_hours
        self.premium = premium

    def act(self, w):
        if self.name in w.lungs:
            return
        n = len(w.lungs) + 1
        wage_tokens_day = w.supply * w.p.wage_pct_day / 100.0 / n
        value_day = wage_tokens_day * w.floor() * self.premium
        cost_day = 720 * w.breath_tgas() * GAS_PRICE_NEAR_PER_TGAS
        if value_day > cost_day:
            w.ignite(self.name, kind="rational")


class SybilSwarm(Agent):
    """One operator running many accounts. Each account is one lung."""

    def __init__(self, name, count=50, interval_hours=0.5, premium=1.0):
        super().__init__(name)
        self.count = count
        self.interval_hours = interval_hours
        self.premium = premium

    def act(self, w):
        for i in range(self.count):
            who = f"{self.name}#{i}"
            if who in w.lungs:
                continue
            n = len(w.lungs) + 1
            value_day = w.supply * w.p.wage_pct_day / 100.0 / n * w.floor() * self.premium
            cost_day = 720 * w.breath_tgas() * GAS_PRICE_NEAR_PER_TGAS
            if value_day > cost_day:
                w.ignite(who, kind="sybil")
            else:
                break


class Hatcher(Agent):
    """Runs the very first lung once, so the coin starts breathing, then walks away."""

    def __init__(self, name):
        super().__init__(name)
        self.interval_hours = 1000.0
        self.done = False

    def act(self, w):
        if not self.done:
            self.done = w.ignite(self.name, kind="devoted")


class Rescuer(Agent):
    """A bot that only moves in apnea, waiting for the bounty to fatten."""

    def __init__(self, name, wait_frac=0.8, interval_hours=0.25, poke_breaths=2):
        super().__init__(name)
        self.wait_frac = wait_frac
        self.interval_hours = interval_hours
        self.poke_breaths = poke_breaths

    def act(self, w):
        if w.lungs or w.supply <= 0:
            return
        waited = w.step - w.last_breath_step
        grace = w.grace_steps()
        if waited < self.wait_frac * grace:
            return
        fuel = w.breath_tgas() * self.poke_breaths + IGNITE_OVERHEAD_TGAS
        value = w.pot * w.floor()
        if value > fuel * GAS_PRICE_NEAR_PER_TGAS and fuel <= MAX_TX_TGAS:
            w.ignite(self.name, fuel_tgas=fuel, kind="rescuer")


# Scenarios ------------------------------------------------------------------------

@dataclass
class Scenario:
    key: str
    label: str
    note: str
    build: Callable[[World], List[Agent]]
    flow: Callable[[World], None]


def poisson(rng: random.Random, lam: float) -> int:
    if lam <= 0:
        return 0
    l, k, p = math.exp(-lam), 0, 1.0
    while True:
        k += 1
        p *= rng.random()
        if p <= l:
            return k - 1


def retail(w: World, near_per_day: float, ticket: float, who: str = "retail") -> None:
    if w.dead or near_per_day <= 0:
        return
    if not w.lungs:
        near_per_day *= 0.3       # fewer people buy into a coin that is not breathing
    lam = near_per_day / ticket / STEPS_PER_DAY
    for _ in range(poisson(w.rng, lam)):
        w.buy(f"{who}{w.rng.randint(0, 400)}", w.rng.lognormvariate(math.log(ticket), 0.8))


def s_cult_build(w):
    a: List[Agent] = [DevotedNurse(f"nurse{i}", w.rng.uniform(6, 14), w.rng.uniform(0.5, 6)) for i in range(12)]
    a += [RationalNurse(f"bot{i}", 0.5) for i in range(3)]
    a += [Rescuer(f"medic{i}", w.rng.uniform(0.3, 0.8)) for i in range(2)]
    return a


def s_cult_flow(w):
    retail(w, 40 if w.day < 7 else 20, 5)


def s_hype_build(w):
    a: List[Agent] = [DevotedNurse(f"nurse{i}", w.rng.uniform(8, 16), w.rng.uniform(0.3, 2), 0.06) for i in range(5)]
    a += [RationalNurse(f"bot{i}", 0.5) for i in range(3)]
    a += [Rescuer("medic0", 0.8)]
    return a


def s_hype_flow(w):
    retail(w, 500 * 0.5 ** (w.day / 4), 10)


def s_whale_build(w):
    a: List[Agent] = [DevotedNurse(f"nurse{i}", w.rng.uniform(8, 14), w.rng.uniform(0.3, 2)) for i in range(5)]
    a += [RationalNurse(f"bot{i}", 0.5) for i in range(3)]
    a += [Rescuer("medic0", 0.7)]
    return a


def s_whale_flow(w):
    if w.step == 6:
        w.buy("WHALE", 1000, "whale")
    retail(w, 15, 3)


def s_sybil_build(w):
    a: List[Agent] = [DevotedNurse(f"nurse{i}", w.rng.uniform(8, 14), w.rng.uniform(0.5, 3)) for i in range(6)]
    a += [RationalNurse("honest-bot", 0.5)]
    a += [SybilSwarm("sybil", 50, 0.5)]
    return a


def s_sybil_flow(w):
    retail(w, 60 if w.day < 7 else 25, 5)


def s_ghost_build(w):
    return [DevotedNurse("founder", 10, 1, 0.33), Rescuer("medic0", 0.9, 1.0)]


def s_ghost_flow(w):
    if w.step == 3:
        w.buy("founder", 15, "holder")
    if w.day < 2:
        retail(w, 3, 1)


def s_small_build(w):
    return [DevotedNurse(f"nurse{i}", w.rng.uniform(6, 12), 0.5) for i in range(3)] + [Rescuer("medic0", 0.6, 0.5)]


def s_small_flow(w):
    if w.step == 3:
        w.buy("dev", 10, "holder")
    retail(w, 3, 1)


def s_chicken_build(w):
    return [Hatcher("founder")] + [Rescuer(f"medic{i}", w.rng.uniform(0.5, 0.97), 0.25) for i in range(5)]


def s_chicken_flow(w):
    retail(w, 25 if w.day < 14 else 8, 5)


def s_hospice_build(w):
    # Automated nurses that check every few minutes and value the coin well above floor.
    return [RationalNurse(f"bot{i}", 0.05, 4.0) for i in range(6)] + [Rescuer(f"medic{i}", 0.3, 0.05) for i in range(3)]


def s_hospice_flow(w):
    retail(w, 200 if w.day < 14 else 60, 10)


SCENARIOS = [
    Scenario("cult", "Steady community", "12 people on a rota, 3 profit bots, buying stays steady", s_cult_build, s_cult_flow),
    Scenario("hype", "Hype then fade", "500 NEAR/day decays with a 4-day half-life, nurses quit over time", s_hype_build, s_hype_flow),
    Scenario("whale", "Whale never helps", "Whale buys 1000 NEAR on day 0 and never runs a lung", s_whale_build, s_whale_flow),
    Scenario("sybil", "Sybil swarm", "One operator runs up to 50 accounts, one lung each", s_sybil_build, s_sybil_flow),
    Scenario("chicken", "Pure chicken", "After one founder lung, 5 bots only rescue, each waiting 50-97% of grace", s_chicken_build, s_chicken_flow),
    Scenario("hospice", "Bots to the end", "Bots check every 3 minutes and value it at 4x floor", s_hospice_build, s_hospice_flow),
    Scenario("small", "Hackathon-sized", "3 people and a small estate", s_small_build, s_small_flow),
    Scenario("ghost", "Ghost town", "One founder who quits on average after 3 days", s_ghost_build, s_ghost_flow),
]


def run(p: Params, sc: Scenario, seed: int) -> World:
    w = World(p, seed)
    agents = sc.build(w)
    queue = [(w.rng.randint(0, 6), i) for i in range(len(agents))]
    heapq.heapify(queue)
    total_steps = HORIZON_DAYS * STEPS_PER_DAY
    while w.step <= total_steps:
        sc.flow(w)
        while queue and queue[0][0] <= w.step:
            when, i = heapq.heappop(queue)
            if not w.dead:
                agents[i].act(w)
            heapq.heappush(queue, (w.step + agents[i].next_delay_steps(w.rng), i))
        w.tick()
        if w.step % STEPS_PER_DAY == 0:
            w.record()
        if w.dead:
            w.record()
            break
        w.step += 1
    if not w.dead:
        w.die("still alive at day 60")
    return w


def outcome(w: World) -> dict:
    s = w.supply or 1.0
    floor = w.floor()
    nurse_kinds = {"devoted", "rational", "sybil", "rescuer"}
    wage_share = w.wages / s
    gas = w.gas_near
    whale = w.owners.get("WHALE")
    whale_payoff = whale.tokens * floor / whale.paid_near if whale else None

    # Cohorts by when in the money they bought.
    total_paid = sum(x.near for x in w.purchases) or 1.0
    cum = 0.0
    cohorts: Dict[str, List[float]] = {"first 10%": [], "middle 80%": [], "last 10%": []}
    for x in w.purchases:
        mid = (cum + x.near / 2) / total_paid
        cum += x.near
        key = "first 10%" if mid < 0.1 else ("last 10%" if mid >= 0.9 else "middle 80%")
        cohorts[key].append(x.tokens * floor / x.near)
    cohort_ratio = {k: (statistics.mean(v) if v else None) for k, v in cohorts.items()}
    last_ratio = None
    if w.purchases:
        x = w.purchases[-1]
        last_ratio = x.tokens * floor / x.near

    # Return per NEAR of gas: is a Sybil swarm better than one honest lung?
    def gas_return(names: List[Owner]) -> Optional[float]:
        g = sum(o.gas_near for o in names)
        v = sum((o.wage_tokens + o.bounty_tokens) * floor for o in names)
        return v / g if g > 0 else None

    swarm = [o for o in w.owners.values() if o.kind == "sybil"]
    honest = [o for o in w.owners.values() if o.kind == "rational"]
    return {
        "death_day": w.death_day,
        "cause": w.death_cause,
        "supply": w.supply,
        "bought": w.q,
        "wages": w.wages,
        "wage_share": wage_share,
        "estate": w.estate,
        "floor": floor,
        "last_price": w.price(),
        "floor_to_last_price": floor / w.price() if w.price() else None,
        "gas_near": gas,
        "heartbeats": w.heartbeats,
        "apnea_events": w.apnea_events,
        "apnea_days": w.apnea_steps / STEPS_PER_DAY,
        "rescues": w.rescues,
        "lost_wage_share": w.lost / (w.lost + w.wages) if (w.lost + w.wages) else 0.0,
        "best_streak_days": w.best_streak_steps / STEPS_PER_DAY,
        "whale_payoff": whale_payoff,
        "whale_gas": whale.gas_near if whale else None,
        "cohort_first10": cohort_ratio["first 10%"],
        "cohort_mid80": cohort_ratio["middle 80%"],
        "cohort_last10": cohort_ratio["last 10%"],
        "last_buyer_ratio": last_ratio,
        "sybil_gas_return": gas_return(swarm),
        "honest_gas_return": gas_return(honest),
        "lungs_nurse_kinds": sorted({o.kind for o in w.owners.values() if o.kind in nurse_kinds}),
    }


def med(values):
    vals = [v for v in values if v is not None]
    return statistics.median(vals) if vals else None


def pct(values, q):
    vals = sorted(v for v in values if v is not None)
    if not vals:
        return None
    i = min(len(vals) - 1, max(0, int(round(q * (len(vals) - 1)))))
    return vals[i]


def evaluate(p: Params, seeds: int, scenarios=SCENARIOS) -> dict:
    out = {}
    for sc in scenarios:
        rows = [outcome(run(p, sc, 1000 + s * 7)) for s in range(seeds)]
        keys = ["death_day", "supply", "wages", "wage_share", "estate", "floor", "floor_to_last_price", "gas_near",
                "heartbeats", "apnea_events", "apnea_days", "rescues", "lost_wage_share", "best_streak_days",
                "whale_payoff", "cohort_first10", "cohort_mid80", "cohort_last10", "last_buyer_ratio",
                "sybil_gas_return", "honest_gas_return"]
        s = {k: med([r[k] for r in rows]) for k in keys}
        s["death_day_p10"] = pct([r["death_day"] for r in rows], 0.1)
        s["death_day_p90"] = pct([r["death_day"] for r in rows], 0.9)
        s["min_last_buyer_ratio"] = min((r["last_buyer_ratio"] for r in rows if r["last_buyer_ratio"] is not None), default=None)
        causes: Dict[str, int] = {}
        for r in rows:
            causes[r["cause"]] = causes.get(r["cause"], 0) + 1
        s["causes"] = causes
        s["runs"] = len(rows)
        out[sc.key] = s
    return out


def median_series(p: Params, sc: Scenario, seeds: int) -> List[dict]:
    """Daily median series, carrying each run's final value forward after death."""
    runs = [run(p, sc, 1000 + s * 7).series for s in range(seeds)]
    out = []
    for day in range(0, HORIZON_DAYS + 1):
        row = {"day": day}
        for key in ["supply", "bought", "wages", "estate", "floor", "gas_near", "lungs", "breath_tgas"]:
            vals = []
            for series in runs:
                match = None
                for item in series:
                    if item["day"] <= day:
                        match = item
                    else:
                        break
                if match:
                    vals.append(match[key])
            row[key] = statistics.median(vals) if vals else 0
        row["alive_share"] = sum(1 for series in runs if series and (series[-1]["alive"] or series[-1]["day"] >= day)) / len(runs)
        out.append(row)
    return out


def first_design_exploit() -> dict:
    """
    Compare the two designs for one attack: a lone early actor on day 1, then 1000 NEAR
    from everyone else.

    Design A (dropped): emission is fixed per block and split by NEAR attached to lungs.
    A lone miner attaches 0.01 NEAR while nobody is watching.

    Design B (kept): the bonding curve sells tokens, wages are a small % of supply.
    The same early actor spends the same 0.01 NEAR on the curve and runs one lung.
    """
    # Design A: emission halves every 10 days, so the first day carries about 6.7% of all
    # supply. The attacker takes all of day 1 for 0.01 NEAR.
    era_share = 0.5
    first_day_share = era_share * (1 / 10)
    design_a_estate = 1000.01
    design_a_take = design_a_estate * first_day_share
    # Keep that lone position alive for five days and it holds about a quarter of supply.
    five_day_share = 0.25
    design_a_take_5 = design_a_estate * five_day_share

    # Design B on launch parameters.
    p = Params()
    w = World(p, 1)
    w.buy("attacker", 0.01)
    w.buy("everyone", 1000)
    attacker_tokens = w.owners["attacker"].tokens
    # Attacker also nurses alone for one day: one day of wages at 0.5% of supply.
    attacker_tokens += w.supply * p.wage_pct_day / 100
    wage_supply = w.supply * (1 + p.wage_pct_day / 100)
    design_b_take = attacker_tokens / wage_supply * w.estate
    return {
        "design_a_cost_near": 0.01,
        "design_a_take_1_day_near": design_a_take,
        "design_a_take_5_days_near": design_a_take_5,
        "design_b_cost_near": 0.01,
        "design_b_take_1_day_near": design_b_take,
    }


SWEEP = {
    "doubling_days": [5.0, 7.0, 10.0],
    "wage_pct_day": [0.25, 0.5, 1.0, 2.0],
    "grace_hours": [3.0, 6.0, 12.0],
    "bounty_share": [0.25, 0.5, 0.75],
}


def sweep(seeds: int) -> List[dict]:
    keys = {"cult", "hype", "chicken", "small"}
    use = [s for s in SCENARIOS if s.key in keys]
    rows = []
    for d in SWEEP["doubling_days"]:
        for wg in SWEEP["wage_pct_day"]:
            for g in SWEEP["grace_hours"]:
                for b in SWEEP["bounty_share"]:
                    p = Params(name="sweep", doubling_days=d, wage_pct_day=wg, grace_hours=g, bounty_share=b)
                    ev = evaluate(p, seeds, use)
                    rows.append({
                        "doubling_days": d, "wage_pct_day": wg, "grace_hours": g, "bounty_share": b,
                        "cult_life": ev["cult"]["death_day"],
                        "hype_life": ev["hype"]["death_day"],
                        "hype_wage_share": ev["hype"]["wage_share"],
                        "chicken_life": ev["chicken"]["death_day"],
                        "chicken_p10": ev["chicken"]["death_day_p10"],
                        "small_life": ev["small"]["death_day"],
                        "cult_floor_ratio": ev["cult"]["floor_to_last_price"],
                        "hype_last10": ev["hype"]["cohort_last10"],
                    })
    return rows


def fmt(x, digits=2):
    if x is None:
        return "-"
    if isinstance(x, float):
        if abs(x) >= 1e6:
            return f"{x / 1e6:,.1f}M"
        if abs(x) >= 100:
            return f"{x:,.0f}"
        return f"{x:.{digits}f}"
    return str(x)


def main(argv: List[str]) -> int:
    mode = argv[1] if len(argv) > 1 else "baseline"
    base = os.path.join(os.path.dirname(os.path.abspath(__file__)), "results")
    os.makedirs(base, exist_ok=True)
    if mode == "sweep":
        seeds = int(argv[2]) if len(argv) > 2 else 8
        rows = sweep(seeds)
        with open(os.path.join(base, "sweep.json"), "w") as f:
            json.dump({"seeds": seeds, "rows": rows}, f, indent=1)
        print(f"wrote {len(rows)} rows")
        return 0

    seeds = int(argv[2]) if len(argv) > 2 else 60
    p = Params()
    ev = evaluate(p, seeds)
    series = {sc.key: median_series(p, sc, min(seeds, 30)) for sc in SCENARIOS}
    exploit = first_design_exploit()
    with open(os.path.join(base, "baseline.json"), "w") as f:
        json.dump({"params": asdict(p), "seeds": seeds,
                   "scenarios": {s.key: {"label": s.label, "note": s.note} for s in SCENARIOS},
                   "summary": ev, "series": series, "first_design_exploit": exploit}, f, indent=1)

    print(f"Params: {p}\nSeeds per scenario: {seeds}\n")
    head = ["scenario", "life", "p10-p90", "supply", "wage%", "estate", "floor/last", "first10", "last10", "gasNEAR", "rescue", "apneaDays"]
    print(" | ".join(head))
    for sc in SCENARIOS:
        s = ev[sc.key]
        print(" | ".join([
            sc.key, fmt(s["death_day"], 1), f"{fmt(s['death_day_p10'], 1)}-{fmt(s['death_day_p90'], 1)}",
            fmt(s["supply"]), fmt((s["wage_share"] or 0) * 100, 1), fmt(s["estate"]), fmt(s["floor_to_last_price"]),
            fmt(s["cohort_first10"]), fmt(s["cohort_last10"]), fmt(s["gas_near"]), fmt(s["rescues"], 0), fmt(s["apnea_days"], 1),
        ]))
    print("\nDeath causes:")
    for sc in SCENARIOS:
        print(f"  {sc.key}: {ev[sc.key]['causes']}")
    print(f"\nWhale payoff per NEAR at death: {fmt(ev['whale']['whale_payoff'])}")
    print(f"Sybil gas return vs honest bot: {fmt(ev['sybil']['sybil_gas_return'])} vs {fmt(ev['sybil']['honest_gas_return'])}")
    print(f"Lowest last-buyer payoff in any run: " + ", ".join(f"{k}={fmt(v['min_last_buyer_ratio'])}" for k, v in ev.items()))
    print(f"First design exploit: {exploit}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
