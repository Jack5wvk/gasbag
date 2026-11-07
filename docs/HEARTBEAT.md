# Keeper-free timers on NEAR

Smart contracts run only when something calls them. Anything on a schedule, such as a draw, a settlement or a rebalance, usually needs a keeper: a server or a cron job that someone has to pay for and keep online. Most hackathon projects that need a schedule run one.

NEAR has a protocol feature that removes most of that need, and it costs very little to use. This page explains the pattern, what it can and can't do, and how to use it.

## The pattern

NEP-519 lets a contract yield a promise and wait for a resume. If nothing resumes it, the protocol runs the callback anyway after 200 blocks, about two minutes. NEP-519 also says the callback can yield again.

So a contract can do this:

1. A transaction calls `start`, which creates a yielded promise and throws away the resumption id.
2. 200 blocks later the protocol runs the callback, with no transaction and no caller.
3. The callback does its work and yields again, forwarding its unused gas.
4. This repeats until the gas runs out, and the leftover is refunded to the signer.

One transaction becomes a chain of timed wake-ups. Because the id is discarded, nobody can trigger a beat early.

## Use it from your contract

```toml
[dependencies]
near-heartbeat = { path = "../heartbeat" }
```

```rust
use near_sdk::{near, Gas};

#[near]
impl Contract {
    /// Anyone can start the chain. The gas attached is the fuel.
    pub fn start(&mut self) {
        near_heartbeat::arm("on_beat", b"{}");
    }

    /// `#[private]` is essential: only the contract itself may run a beat.
    #[private]
    pub fn on_beat(&mut self) {
        self.do_scheduled_work();
        near_heartbeat::arm_if_fuelled("on_beat", b"{}", Gas::from_tgas(3));
    }
}
```

## Or use the Pacemaker, with no code changes

The [`pacemaker`](../contracts/pacemaker/src/lib.rs) contract does this for any existing contract. Register a target, a method and an interval, attach gas, and it calls that method on schedule. Your contract can allow-list the pacemaker's account instead of a keeper's key. When the gas runs out, anyone can refuel the job.

```bash
near contract call-function as-transaction <pacemaker> schedule json-args '{
  "target": "your-contract.testnet",
  "method": "tick",
  "args": "e30=",
  "every_beats": 5,
  "call_tgas": 30
}' prepaid-gas '1000 Tgas' attached-deposit '0.02 NEAR' sign-as you.testnet network-config testnet sign-with-keychain send
```

## How long a chain lasts

At protocol 88 a transaction can carry 1,000 TGas. Gas costs 0.0001 NEAR per TGas, so a full transaction pays at most 0.1 NEAR, and unused gas is refunded.

| Gas per beat | Beats per transaction | Runs for |
|---|---|---|
| 2 TGas | 500 | about 16 hours |
| 3 TGas | 333 | about 11 hours |
| 10 TGas | 100 | about 3.3 hours |
| 30 TGas | 33 | about 1 hour |

A keeper-free chain still needs someone to send a transaction every so often. The difference is that this is one transaction every few hours from anyone, instead of an always-on server with a privileged key. If nobody sends it, the schedule stops. It doesn't fail silently into a wrong state.

## Things to know

- **The interval is fixed at 200 blocks.** A longer interval means counting beats, and each beat still costs gas. There is no shorter interval, because resuming early needs an outside caller.
- **Never panic in a beat.** A panic rolls back that beat and ends the chain. Handle every case and return.
- **The transaction stays open.** The chain's receipts belong to the transaction that started it, so anything waiting for that transaction to finish will wait for hours. Wallets and RPC clients that wait may report a timeout even though it worked. Send with `wait_until: INCLUDED`, or use a function-call key from a page, as [`web/src/wallet.js`](../web/src/wallet.js) does.
- **Forward gas with weight, and create no other weighted promises in a beat.** They would split the fuel.
- **Don't resume.** Storing the `YieldId` adds a way to trigger the contract early, which you then have to guard.
- **Parallel chains are allowed.** Decide what that means for you. GASBAG treats each as a lung, and the Pacemaker lets a new chain replace the old one.

## Where this comes from

- [NEP-519: Yield Execution](https://github.com/near/NEPs/blob/master/neps/nep-0519.md), which states that a callback can "delay replying again"
- [Yield and Resume docs](https://docs.near.org/smart-contracts/anatomy/yield-resume)
- Live limits come from `EXPERIMENTAL_protocol_config`: `yield_timeout_length_in_blocks` and `max_total_prepaid_gas`
