# GASBAG

**A memecoin that breathes NEAR gas.** Keep it breathing and it pays you. Let it die and everyone still holding the bag inherits its estate.

Built at NEAR HACKATHON (November 2026) for the track **Memecoins With Mechanics** (`memecoins`).

| | |
|---|---|
| **What it is** | A NEP-141 token whose heartbeat is a chain of NEAR receipts. One transaction keeps the contract waking itself every 200 blocks, about 2 minutes, with no keeper, until the gas runs out. Then it really stops. |
| **The problem** | Most memecoins die quietly, and the last buyers lose everything. Contracts can't act by themselves either, so scheduled mechanics usually lean on a bot or a server someone can switch off. |
| **What is new** | The coin wakes itself using NEP-519 yield timeouts, so nothing off chain keeps it alive. When it dies, a will pays out: holders burn tokens for a pro-rata share of an estate that nobody could touch while it lived. |
| **Status** | Working on NEAR testnet. The heartbeat, a full life and a payout have all run on the real network ([evidence below](#proven-on-testnet)). The main coin launches shortly before judging, because keeping it alive costs more as it ages. |

## The six rules

All of them live in the contract. Every parameter is fixed in `new`, readable through `rules()`, and the contract account has no access keys.

1. **Breathe.** Anyone sends one transaction with up to 1,000 TGas. That starts a *lung*, which wakes the contract every 200 blocks until its gas runs out. One lung per account.
2. **Get paid.** While any lung is alive, 0.5% of supply per day is minted to the lungs, split equally. Holders who never help get diluted.
3. **Buy in.** Anyone can mint on a linear bonding curve. All the NEAR goes to the estate, less a 1% share for its AI voice. There is no sell-back and no withdraw method.
4. **Grow old.** Gas per breath doubles every 7 days and the grace period halves. Around day 53 a single breath no longer fits in a transaction, and it dies of old age.
5. **Stop breathing.** If every lung runs out, wages stop. Half the missed wages build a rescue bounty for whoever restarts it, and the other half is never minted.
6. **Die.** With no breath for the whole grace period it is dead for good. Anyone can burn GASBAG for a pro-rata share of the estate.

The game inside it: while it isn't breathing, the bounty keeps growing, so waiting pays until someone else moves or the coin dies. Who blinks first?

## Try it: do X, see Y

### With no wallet and no account

Needs Node 20 or later.

1. Run `git clone <this repo> && cd gasbag && npm install`.
   You should see the dependencies install.
2. Run `node scripts/verify.mjs <coin>`.
   You should see a list of `PASS` lines covering NEP-141, zero access keys, public parameters, breathing on schedule, the estate being covered, and `Everything matches.` at the end.
3. Run `node scripts/status.mjs <coin>` twice, 3 minutes apart.
   You should see `breaths` go up without anyone sending a transaction to the coin.
4. Open the live page.
   You should see the balloon, a countdown, and a heart trace where each spike is a real breath from the contract's log.

### Watch a whole life in 15 minutes

Needs a testnet account with about 4 NEAR, or the page and a testnet wallet.

1. Hatch a coin: open the live page and use **Hatch a coin**, or call `hatch` on the hatchery.
   You should see a new coin in the ward, in status `apnea`.
2. Buy some with **Buy GASBAG**.
   You should see your bag fill and the estate rise by what you paid, minus the small registration fee.
3. Press **Start a lung**, or run `node scripts/ignite.mjs <coin> --as <you>`.
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
| The pacemaker refuses a stranger | `PACEMAKER[TARGET] only a contract or its parent account can schedule calls to it` | [`9Bt4scQC…`](https://testnet.nearblocks.io/txns/9Bt4scQCag3UcYjEyJrWyEgNJSndyoLvEsU647L4RLbw) |

Live contract: [`pacemaker.gasbag-hq.testnet`](https://testnet.nearblocks.io/address/pacemaker.gasbag-hq.testnet), with no access keys.

Measured on testnet: a breath costs 5.0 to 5.3 TGas in total at birth, and beats landed exactly 201 blocks apart. Prepaid gas is bought at 0.001 NEAR per TGas and burned at 0.0001, and each breath refunds the difference, so a full lung needs about 1 NEAR held up front but burns about 0.1.

## Against the track's requirements

| The track asks for | In GASBAG | Check it |
|---|---|---|
| An NEP-141 token with the mechanic in the contract, not in a backend | One contract holds the token, lungs, curve and estate. The heartbeat runs on protocol timers | `contracts/gasbag/src/lib.rs` |
| All parameters set at launch and readable, no admin key | Set in `new`, returned by `rules()`. No owner, setter or pause, and no access keys | `node scripts/verify.mjs <coin>` |
| Sybil resistance good enough for a demo, with limits written plainly | See [Sybil resistance and its limits](#sybil-resistance-and-its-limits) | Test `splitting_into_more_accounts_earns_nothing_extra`, and the Sybil scenario in `sim/` |
| A page with the streak, today's progress and a log of every mint and burn | Breathing streak with a high score, today's breaths and lungs, and an on-chain log of the latest 128 mints, burns and lifecycle events. Every older one stays in NEP-141 and NEP-297 events | The live page, or `log` and `days` |
| Simulate 60 days under several scenarios first | 8 scenarios and 108 parameter sets, which picked the launch parameters | `sim/gasbag_sim.py`, [results below](#the-simulation) |

## How NEAR fits

This couldn't be built the same way on Ethereum or Solana, where a contract can't wake itself.

- **Yield timeouts ([NEP-519](https://github.com/near/NEPs/blob/master/neps/nep-0519.md)).** A yielded promise nobody resumes fires by itself after 200 blocks. The callback yields again and forwards its gas, so one transaction becomes a timed chain.
- **1,000 TGas transactions.** At protocol 88 one transaction can carry 1 PGas, which is what lets a lung last hours.
- **Keyless accounts.** The coin's account has no access keys, so "nobody can change this" is something you can check.
- **Function-call keys.** A lung's transaction stays open for hours, and a client that waits for it gets a 408 timeout after 30 seconds, even though the lung is fine. So the page uses a small key that can only call `ignite`, and submits without waiting. On testnet that key was refused for `claim` and for a transfer.
- **Live state in metadata.** `ft_metadata` returns an icon that follows the coin's state, so a wallet that reads it live sees it die.

## What it gives NEAR

- **A keeper-free scheduler for every NEAR contract.** The [`pacemaker`](contracts/pacemaker/src/lib.rs) contract calls any method on any contract on a schedule, with nothing running off chain. The [`near-heartbeat`](contracts/heartbeat/src/lib.rs) crate does the same inside a contract. [The guide](docs/HEARTBEAT.md) explains the pattern and its limits.
- **A launchpad for mortal coins.** The [`hatchery`](contracts/hatchery/src/lib.rs) launches a keyless coin in one transaction, always on the same published rules.
- **Real NEAR burned.** Every breath burns gas. Contract gas rewards are zero at protocol 88, so none of it comes back. The simulation burned about 2.5 NEAR in a small community and 1,338 in the longest life.
- **An answer to a request.** NEAR's co-founder [described an AI pet](https://crypto.news/near-co-founder-pitches-ai-pets-private-trading/) with its own NEAR balance paying for inference and its prompt on chain. Gasbag is a community-owned take on that, through a coin instead of an NFT.

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

## Sybil resistance and its limits

- **Wages follow gas actually burned.** Each lung gets an equal share and each lung burns a full lung of gas, so splitting into more accounts earns nothing extra. In the simulation, 50 accounts earned 1.46 NEAR per NEAR of gas against 1.60 for one honest bot.
- **Buying is unaffected by splitting.** The curve charges the same however a purchase is divided.
- **Limit: bots can outspend people.** Someone who burns more gas gets a bigger share of wages. It's proportional, not free, but it does favour automation.
- **Limit: there is no proof of personhood.** One person can run many lungs. Nothing in the contract tries to stop that, because the cost already scales.

## Honest limits

- **It is a game of who pays whom.** Early buyers are paid by later ones. In the simulation the first 10% of buyers got ×1.23 to ×3.79 and the last 10% got ×0.44 to ×0.89. The last buyer can get back as little as about 37% after a full life of wages. It runs on testnet, and a mainnet launch could count as gambling or a security in some places.
- **Someone has to fuel it.** At birth one transaction lasts about 6.5 hours, and it needs about 1 NEAR held up front, most of it refunded as the lung burns. The chain needs a person or a script to send that, though anyone can and nobody needs permission. If nobody does, it dies, which is the point.
- **Nothing can upgrade it.** A bug can't be patched on a keyless coin. A new coin would have to be launched instead.
- **The voice account is a real role.** If a voice is set at launch, that one account can post lines and draw its 1% budget. It can't touch the estate or the rules.
- **On-chain randomness isn't used**, so there is nothing for a validator to bias.
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
```

## Libraries and AI use

Rust: `near-sdk` 5.30.1, `near-contract-standards`, `uint`, `cargo-near`. Web: Vite, `near-api-js`, `@hot-labs/near-connect`. Fonts: Bagel Fat One, Atkinson Hyperlegible and IBM Plex Mono from Google Fonts. AI coding assistants helped write the code and docs, as the rules allow (Q-005).

## Hackathon timeline

All code here was written during the NEAR HACKATHON window, from 7 November 2026. There is no earlier baseline and no earlier product on another chain.

## License

MIT. See [LICENSE](LICENSE).
