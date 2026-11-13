# GASBAG

**A memecoin that breathes NEAR gas.** Keep it breathing and it pays you. Let it die and everyone still holding the bag inherits its estate.

Built at NEAR HACKATHON (November 2026) for the track **Memecoins With Mechanics** (`memecoins`).

**Watch it:** [the demo video](docs/gasbag-demo.mp4), 1 min 52 s. **Try it:** [the live page](https://jack5wvk.github.io/gasbag/), which reads NEAR testnet directly.

| | |
|---|---|
| **What it is** | A NEP-141 token whose heartbeat is a chain of NEAR receipts. One transaction keeps the contract waking itself every 200 blocks, about 2 minutes, with no keeper, until the gas runs out. Then it really stops. |
| **The problem** | Most memecoins die quietly, and the last buyers lose everything. Contracts can't act by themselves either, so scheduled mechanics usually lean on a bot or a server someone can switch off. |
| **What is new** | The coin wakes itself using NEP-519 yield timeouts, so nothing off chain keeps it alive. When it dies, a will pays out: holders burn tokens for a pro-rata share of an estate that nobody could touch while it lived. |
| **Status** | Working on NEAR testnet. The heartbeat, a full life and a payout have all run on the real network ([evidence below](#proven-on-testnet)). The main coin, [`coin.gasbag-hq.testnet`](https://testnet.nearblocks.io/address/coin.gasbag-hq.testnet), has no access keys. It took 196 breaths on 8 Nov, then flatlined on 9 Nov when nobody started another lung, as its rules say, so its will is open. |

## The six rules

All of them live in the contract. Every parameter is fixed in `new`, readable through `rules()`, and the contract account has no access keys.

1. **Breathe.** Anyone sends one transaction with up to 1,000 TGas. That starts a *lung*, which wakes the contract every 200 blocks until its gas runs out. One lung per account.
2. **Get paid.** While any lung is alive, 0.5% of supply per day is minted to the lungs, split equally. Holders who never help get diluted.
3. **Buy in.** Anyone can mint on a linear bonding curve. All the NEAR goes to the estate. There is no sell-back and no withdraw method.
4. **Grow old.** Gas per breath doubles every 7 days and the grace period halves. Around day 53 a single breath no longer fits in a transaction, and it dies of old age.
5. **Stop breathing.** If every lung runs out, wages stop. Half the missed wages build a rescue bounty for whoever restarts it, and the other half is never minted.
6. **Die.** With no breath for the whole grace period it is dead for good. Anyone can burn GASBAG for a pro-rata share of the estate.

The game inside it: while it isn't breathing, the bounty keeps growing, so waiting pays until someone else moves or the coin dies. Who blinks first?

## Try it: do X, see Y

### With no wallet and no account

Needs Node 20 or later.

1. Run `git clone https://github.com/Jack5wvk/gasbag && cd gasbag && npm install`.
   You should see the dependencies install.
2. Run `node scripts/verify.mjs coin.gasbag-hq.testnet`.
   You should see a list of `PASS` lines covering NEP-141, zero access keys, public parameters, breathing on schedule, the estate being covered, and `Everything matches.` at the end.
3. Run `node scripts/status.mjs coin.gasbag-hq.testnet`.
   You should see `DEAD (Flatline)`, 196 breaths and a best streak of about 6.6 hours. Nobody started a second lung, so it died the way its rules say. On a coin that is breathing, run it twice, 3 minutes apart, and `breaths` goes up with no transaction sent to the coin. The next section runs a whole life on a fresh coin.
4. Open [the live page](https://jack5wvk.github.io/gasbag/).
   You should see the main coin's balloon gone flat, marked `Flatlined`, with its time of death, its estate and a table of every day of its life, all read from the chain.

### Watch a whole life in 15 minutes

Needs a testnet account with about 4 NEAR, or the page and a testnet wallet.

1. Hatch a coin: open the live page and use **Hatch a coin**, or call `hatch` on the hatchery.
   You should see a new coin in the ward, in status `apnea`.
2. Buy some with **Buy GASBAG**.
   You should see your bag fill and the estate rise by what you paid, minus the small registration fee.
3. Press **Start a lung**, or run `node scripts/ignite.mjs <coin account> --as <your testnet account>`.
   You should see status `breathing`. A breath lands about every 2 minutes, and wages arrive in your balance.
4. Do nothing. Gas per breath doubles every 3 minutes on these rules, so the lung runs out.
   You should see `apnea`, a rescue bounty growing, and a flat trace.
5. Keep doing nothing.
   You should see status `dead`, and any `buy` or `ignite` now fails with `GASBAG[DEAD]`.
6. Press **Claim your share**.
   You should see your GASBAG burned and NEAR arrive, and an `ft_burn` event in the transaction.

### Run the tests

Needs Rust and [cargo-near](https://github.com/near/cargo-near).

```bash
cd contracts && cargo test --workspace
```

You should see 110 tests pass: 84 for the coin, 14 for the pacemaker, and 6 each for the hatchery and the heartbeat module. Then run the simulation with `python sim/gasbag_sim.py baseline 40`, which uses only the standard library and prints the outcome table.

## Proven on testnet

These are real transactions on NEAR testnet, from coins deliberately run to the end of their lives.

| What was shown | Result | Transaction |
|---|---|---|
| One transaction keeps a contract waking itself | 3 breaths exactly 201 blocks apart, with nothing else sent to the contract, then a last gasp | [`GGrNGrAm…`](https://testnet.nearblocks.io/txns/GGrNGrAmvxpmjZb4D73rKrZBXTSD2uBEjsfHgNWgqscs) |
| A full 1,000 TGas lung | Accepted, and ran until it was retired | [`9wCc8zKi…`](https://testnet.nearblocks.io/txns/9wCc8zKih1EEFFkbK4wv1AfkrBGNwdDw9orpEkQBybvv) |
| Ageing and last gasp on a 15-minute coin | Breath cost rose as designed, and the lung ended after 5 breaths | [`6xCVCo9f…`](https://testnet.nearblocks.io/txns/6xCVCo9f6cWn4tgUzPXo45GrMbTENQyQhMSnzuWZRSt7) |
| A rescue from apnea | The next breath paid a 3,068-token bounty on top of wages | [`5Afdj91c…`](https://testnet.nearblocks.io/txns/5Afdj91ckHvFEw1aGyc4VejA5g4BzV71vYVwTm9mY1ub) |
| Death by old age | Dead at age 1,512 blocks against a limit of 1,510, and a later `buy` failed with `GASBAG[DEAD]` | Same coin, `death()` before it was retired |
| The will | All 328,602 tokens burned and the whole 0.295 NEAR estate paid out. A second claim failed with `GASBAG[NOTHING]` | [`H5vojYBv…`](https://testnet.nearblocks.io/txns/H5vojYBv4jeTE79wYLiAHFPwhm1NqVu2zSduN9CYGRN) |
| The pacemaker calling a real contract | A job called `total()` on the hatchery every beat with nothing running off chain, and each call returned `"1"` | [`A3Viqo21…`](https://testnet.nearblocks.io/txns/A3Viqo21qJ4nph2ZDn2cMDno6dpU1VtwqKTXXg4a5qa1) |
| The pacemaker refuses a stranger | `PACEMAKER[TARGET] only a contract or its parent account can schedule calls to it` | [`9Bt4scQC…`](https://testnet.nearblocks.io/txns/9Bt4scQCag3UcYjEyJrWyEgNJSndyoLvEsU647L4RLbw) |
| A whole life on a hatched coin, from the page | **Hatch a coin** created a coin with no access keys on the hatchery's fixed rules. It took 5 breaths 201 blocks apart, died of old age, and the will paid out its whole estate. `verify` printed `Everything matches.` at birth, while breathing and after the payout | [hatch `D1VuAFTg…`](https://testnet.nearblocks.io/txns/D1VuAFTgTnKa6aWkNsgT3KCX37BHH7mYumedGaEEWMFt), [claim `D9HSeYhr…`](https://testnet.nearblocks.io/txns/D9HSeYhrVyqLkcwojBfMMdXnjNF3ESNwn7pobZioMXwq) |
| A full lung through the page's session key | **Start a lung** sent 1,000 TGas signed by the limited key, on that hatched coin | [`8DvtCEHM…`](https://testnet.nearblocks.io/txns/8DvtCEHMXa9gBEckv9y1WSmJixDtwFuTBTVhkCPbDV1) |
| Plain NEP-141 transfers | `ft_transfer` moved tokens and required its 1 yoctoNEAR, and `ft_transfer_call` to a contract with no `ft_on_transfer` refunded them | [transfer `AsYyjMVi…`](https://testnet.nearblocks.io/txns/AsYyjMVinT5H9EuvCayp4c189qHRyaur2XKFgvggweVZ), [refund `4ehNEd5g…`](https://testnet.nearblocks.io/txns/4ehNEd5gRjG4J8mBXrQ3UVQzCq5YrUqKR2Hcqhj9Vv83) |
| A limited session key | Refused `claim` and a transfer, then started a lung and returned in 1.5 seconds | [`CfBWs9SR…`](https://testnet.nearblocks.io/txns/CfBWs9SRQjXc4HmnPMvRpZWVJrkhzDppaZPbj3eJ69ur) |
| The caretaker and a flatline | A coin ran out of fuel, `caretaker.mjs` started a new lung, and when that ran out too the coin flatlined | [`Gg7Si1Uf…`](https://testnet.nearblocks.io/txns/Gg7Si1UfGX57jcTt2avW6UaY5ofLcsf4TKs86ukezYAL) |
| The page's buttons | Connect, hatch, buy, start a lung, claim and disconnect, all pressed on the page, with a local stand-in signing in place of a wallet app | [lung `BnwSzyVU…`](https://testnet.nearblocks.io/txns/BnwSzyVUgUKzsLV5G93rhECcGuKkrutPDsmLKhuYac8n), [claim `BNndtqed…`](https://testnet.nearblocks.io/txns/BNndtqedEP8nhZfDFcQzQxH7PoEGx5mmju36T9Xf2usb) |

Deployed contracts: the main coin [`coin.gasbag-hq.testnet`](https://testnet.nearblocks.io/address/coin.gasbag-hq.testnet), flatlined on 9 Nov with its will open, and the scheduler [`pacemaker.gasbag-hq.testnet`](https://testnet.nearblocks.io/address/pacemaker.gasbag-hq.testnet), both with no access keys, and the launchpad [`hatchery.gasbag-hq.testnet`](https://testnet.nearblocks.io/address/hatchery.gasbag-hq.testnet).

Measured on testnet: a breath costs 5.0 to 5.3 TGas in total at birth, and beats landed exactly 201 blocks apart. Prepaid gas is bought at 0.001 NEAR per TGas and burned at 0.0001, and each breath refunds the difference, so a full lung needs about 1 NEAR held up front but burns about 0.1. The page sizes each lung to what the wallet can hold, and leaves 0.1 NEAR free.

## Against the track's requirements

| The track asks for | In GASBAG | Check it |
|---|---|---|
| An NEP-141 token with the mechanic in the contract, not in a backend | One contract holds the token, lungs, curve and estate. The heartbeat runs on protocol timers | `contracts/gasbag/src/lib.rs` |
| All parameters set at launch and readable, no admin key | Set in `new`, returned by `rules()`. No owner, setter or pause, and no access keys | `node scripts/verify.mjs coin.gasbag-hq.testnet` |
| Sybil resistance good enough for a demo, with limits written plainly | See [Sybil resistance and its limits](#sybil-resistance-and-its-limits) | Test `splitting_into_more_accounts_earns_nothing_extra`, and the Sybil scenario in `sim/` |
| A page with the streak, today's progress and a log of every mint and burn | Breathing streak with a high score. Today's breaths, mints and burns, and how far through the day it is. The latest 128 mints, burns and lifecycle events one by one, then a table adding up every mint and burn for each day of its life. Each one also stays in NEP-141 and NEP-297 events | The live page, or `log` and `days` |
| Simulate 60 days under several scenarios first, showing supply over time, then pick the parameters | 8 scenarios, supply by day for each, and 108 parameter sets behind [each launch value](#how-the-launch-parameters-were-picked) | `sim/gasbag_sim.py`, [results below](#the-simulation) |

## How NEAR fits

This couldn't be built the same way on Ethereum or Solana, where a contract can't wake itself.

- **Yield timeouts ([NEP-519](https://github.com/near/NEPs/blob/master/neps/nep-0519.md)).** A yielded promise nobody resumes fires by itself after 200 blocks. The callback yields again and forwards its gas, so one transaction becomes a timed chain.
- **1,000 TGas transactions.** At protocol 88 one transaction can carry 1 PGas, which is what lets a lung last hours.
- **Keyless accounts.** The coin's account has no access keys, so "nobody can change this" is something you can check.
- **Function-call keys.** A lung's transaction stays open for hours, and a client that waits for it gets a 408 timeout after 30 seconds, even though the lung is fine. So the page uses a small key that can only call `ignite`, and submits without waiting. On testnet that key was refused for `claim` and for a transfer, and it started a lung in 1.5 seconds. When a key's allowance runs down, the page asks the wallet for a new one.
- **Live state in metadata.** `ft_metadata` returns an icon that follows the coin's state, so a wallet that reads it live sees it die.

## What it gives NEAR

- **A keeper-free scheduler for every NEAR contract.** The [`pacemaker`](contracts/pacemaker/src/lib.rs) contract calls any method on any contract on a schedule, with nothing running off chain. The [`near-heartbeat`](contracts/heartbeat/src/lib.rs) crate does the same inside a contract. [The guide](docs/HEARTBEAT.md) explains the pattern and its limits.
- **A launchpad for mortal coins.** The [`hatchery`](contracts/hatchery/src/lib.rs) launches a keyless coin in one transaction, always on the same published rules.
- **Real NEAR burned.** Every breath burns gas. Contract gas rewards are zero at protocol 88, so none of it comes back. The simulation burned about 2.5 NEAR in a small community and 1,338 in the longest life.
- **An answer to a request.** NEAR's co-founder [described an AI pet](https://crypto.news/near-co-founder-pitches-ai-pets-private-trading/) with its own NEAR balance paying for inference and its prompt on chain. Gasbag is a community-owned take on that, through a coin instead of an NFT. The contract stores the prompt and supports a voice with its own budget. The main coin has the prompt but launched without a voice.

## The simulation

`sim/gasbag_sim.py` runs the contract's rules for 60 days. The protocol numbers are real. The people are invented, so this is a stress test, not a forecast. Medians of 40 runs on the launch parameters:

| Scenario | Days lived | Supply | Estate, NEAR | Nurses' share | First 10% of buyers | Last 10% of buyers |
|---|---|---|---|---|---|---|
| Steady community | 27.8 | 268.6M | 845 | 9.2% | ×2.04 | ×0.54 |
| Hype then fade | 34.9 | 674.4M | 3,908 | 14.3% | ×2.30 | ×0.48 |
| Whale never helps | 30.7 | 403.6M | 1,594 | 12.7% | ×3.79 | ×0.50 |
| Sybil swarm | 28.1 | 341.0M | 1,261 | 9.3% | ×2.16 | ×0.53 |
| Pure chicken | 22.6 | 91.4M | 163 | 4.5% | ×1.60 | ×0.66 |
| Bots to the end | 52.8 | 883.7M | 5,773 | 19.5% | ×2.27 | ×0.44 |
| Hackathon-sized | 13.4 | 26.3M | 32 | 3.1% | ×1.23 | ×0.81 |
| Ghost town | 1.3 | 15.4M | 18 | 0.4% | n/a | ×0.89 |

Buyer columns show NEAR received at death per NEAR paid, holding to the end.

Supply over time, in millions of tokens. Each cell is the median of 30 runs, and `dead` marks days after that scenario's median death. Every scenario has died by day 53, so the 60 days cover each coin's whole life:

| Scenario | Day 1 | Day 5 | Day 10 | Day 20 | Day 30 | Day 40 | Day 50 |
|---|---|---|---|---|---|---|---|
| Steady community | 38.9 | 126.8 | 178.5 | 238.7 | dead | dead | dead |
| Hype then fade | 204.2 | 432.7 | 543.0 | 618.4 | 657.0 | dead | dead |
| Whale never helps | 274.4 | 291.9 | 315.6 | 364.6 | 403.3 | dead | dead |
| Sybil swarm | 48.3 | 162.5 | 230.2 | 299.8 | dead | dead | dead |
| Pure chicken | 12.8 | 40.3 | 64.1 | 88.8 | dead | dead | dead |
| Bots to the end | 99.4 | 325.3 | 486.1 | 646.2 | 736.2 | 805.2 | 865.5 |
| Hackathon-sized | 12.1 | 20.3 | 25.1 | dead | dead | dead | dead |
| Ghost town | 14.6 | dead | dead | dead | dead | dead | dead |

`python sim/gasbag_sim.py supply` prints this table from the saved results in `sim/results/baseline.json`.

### How the launch parameters were picked

The sweep tried 108 parameter sets, 6 runs each, across four scenarios. Each figure here is the median of every set that used that value. The picked values are the `main` preset in `scripts/launch.mjs`, which the main coin launches with.

| Parameter | Values tried | What changed | Picked | Why |
|---|---|---|---|---|
| Doubling time | 5, 7, 10 days | A steady community lives 21, 30, 43 days | 7 days | Old age lands near day 53, inside the 60 days |
| Wage | 0.25, 0.5, 1, 2% a day | Nurses end with 7, 14, 28, 48% of supply, and the last buyers get ×0.52, ×0.48, ×0.40, ×0.29 | 0.5% | Pays nurses without gutting the floor |
| Grace at birth | 3, 6, 12 hours | A small community lives 9, 13, 19 days | 6 hours | One missed shift stays dangerous |
| Rescue bounty | 25, 50, 75% of missed wages | Pure chicken lives 18, 23, 25 days | 50% | Rewards a rescue, and neglect still costs the coin |
| Gas per breath | Not swept | Measured on testnet at 5.0 to 5.3 TGas | 5 TGas | The true cost of a breath |

`python sim/gasbag_sim.py sweep 6` reruns the sweep and rebuilds `sim/results/sweep.json` exactly, and `python sim/gasbag_sim.py picks` prints this table from it.

## Sybil resistance and its limits

- **Wages follow gas actually burned.** Each lung gets an equal share of wages and burns the same gas per breath, so splitting into more accounts earns nothing extra. In the simulation, 50 accounts earned 1.46 NEAR per NEAR of gas against 1.60 for one honest bot.
- **Buying is unaffected by splitting.** The curve charges the same however a purchase is divided.
- **Limit: bots can outspend people.** Someone who burns more gas gets a bigger share of wages. It's proportional, not free, but it does favour automation.
- **Limit: there is no proof of personhood.** One person can run many lungs. Nothing in the contract tries to stop that, because the cost already scales.

## Honest limits

- **It is a game of who pays whom.** Early buyers are paid by later ones. In the simulation the first 10% of buyers got ×1.23 to ×3.79 and the last 10% got ×0.44 to ×0.89. The last buyer can get back as little as about 37% after a full life of wages. It runs on testnet, and a mainnet launch could count as gambling or a security in some places.
- **Someone has to fuel it.** At birth one transaction lasts about 6.5 hours, and it needs about 1 NEAR held up front, most of it refunded as the lung burns. The chain needs a person or a script to send that, though anyone can and nobody needs permission. If nobody does, it dies, which is the point.
- **Nothing can upgrade it.** A bug can't be patched on a keyless coin. A new coin would have to be launched instead.
- **The voice account is a real role.** If a voice is set at launch, that one account can post lines and draw its budget of up to 5% of purchases. It can't touch the estate or the rules. The main coin launched without one, and the script that writes its lines hasn't been run.
- **On-chain randomness isn't used**, so there is nothing for a validator to bias.
- **Not yet run with a wallet app.** The page's buttons were tested with a local stand-in that signs the same transactions. A real wallet's own screens haven't been tried.
- **The hatchery still has its key.** It's deployed at `hatchery.gasbag-hq.testnet` and has hatched a live coin. Whoever holds that key could replace the hatchery's code, but coins already hatched have no keys and can't be changed.
- **Reviewed, not audited.** An independent review of the code found a critical impersonation flaw and a griefing path in the pacemaker, and smaller issues elsewhere. All are fixed, with a test for each. It wasn't a professional audit.

## Repository

```
contracts/gasbag      the coin: NEP-141, lungs, curve, estate, ageing, the will
contracts/heartbeat   near-heartbeat, the reusable timer module
contracts/pacemaker   a public keeper-free scheduler for any contract
contracts/hatchery    launches keyless coins, including 15-minute speedruns
web                   the live page: static, reads the chain directly, no backend
scripts               verify, status, ignite, buy, claim, schedule, launch, caretaker, speak
sim                   the 60-day simulator and its results
docs/HEARTBEAT.md     the keeper-free timer guide
docs/gasbag-demo.mp4  the demo video
```

The deployed contracts report `https://github.com/near-hackathon/gasbag` as their source link (NEP-330 `contract_source_metadata`). That was a placeholder set before this repository existed. This repository is their source.

## Libraries and AI use

Rust: `near-sdk` 5.30.1, `near-contract-standards`, `uint`, `cargo-near`. Web: Vite, `near-api-js`, `@hot-labs/near-connect`. Fonts: Bagel Fat One, Atkinson Hyperlegible and IBM Plex Mono from Google Fonts. AI coding assistants helped write the code and docs, as the rules allow (Q-005). The demo video was filmed on testnet with Playwright, narrated by an AI text-to-speech model (Chatterbox) and edited with ffmpeg.

## Hackathon timeline

All code here was written during the NEAR HACKATHON window, from 7 November 2026. There is no earlier baseline and no earlier product on another chain.

## License

MIT. See [LICENSE](LICENSE).
