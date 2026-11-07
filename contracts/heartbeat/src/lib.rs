//! # near-heartbeat
//!
//! Keeper-free timers for NEAR contracts.
//!
//! Smart contracts normally run only when something calls them, so anything on a schedule
//! needs a keeper: a server, a cron job, a bot. NEP-519 yield execution offers another way.
//! A contract can create a yielded promise and never resume it. The protocol then runs the
//! callback by itself when the yield times out, 200 blocks later. If the callback yields
//! again and hands over its unused gas, one transaction becomes a chain of timed wake-ups.
//!
//! The chain lives on the gas attached to the transaction that started it. It stops when the
//! gas runs out, and whatever is left is refunded to the signer. At protocol 88 a transaction
//! can carry 1,000 TGas, so one transaction can keep a light contract waking for many hours.
//!
//! ```ignore
//! #[near]
//! impl Contract {
//!     /// Anyone can start the chain. Attach as much gas as you want it to run on.
//!     pub fn start(&mut self) {
//!         near_heartbeat::arm("on_beat", b"{}");
//!     }
//!
//!     /// `#[private]` is essential: only the contract itself may run a beat.
//!     #[private]
//!     pub fn on_beat(&mut self) {
//!         self.do_scheduled_work();
//!         near_heartbeat::arm_if_fuelled("on_beat", b"{}", Gas::from_tgas(3));
//!     }
//! }
//! ```
//!
//! ## Rules that keep a chain alive
//!
//! - Never panic in a beat callback. A panic ends the chain and rolls back that beat.
//! - Keep the beat cheap. Every gas unit it uses shortens the chain.
//! - Don't store the `YieldId`. Nobody needs to resume, and an unresumable yield can't be
//!   triggered early by anyone.
//! - Create no other weighted promises in a beat, or they will share the forwarded gas.

use near_sdk::{Gas, GasWeight, Promise, env};

/// Blocks between beats. Fixed by the protocol as `yield_timeout_length_in_blocks`.
pub const BEAT_BLOCKS: u64 = 200;

/// The most gas one transaction can carry at protocol 88 (`max_total_prepaid_gas`).
pub const MAX_TX_GAS: Gas = Gas::from_tgas(1000);

/// Gas still available to this call.
pub fn fuel_left() -> Gas {
    env::prepaid_gas().saturating_sub(env::used_gas())
}

/// Schedules `callback` to run on this contract after [`BEAT_BLOCKS`] blocks.
///
/// All the gas left when the current call ends goes to the callback, so the callback must be
/// the only weighted promise this call creates. Nobody can trigger it sooner, because the
/// resumption id is thrown away.
pub fn arm(callback: &str, args: impl AsRef<[u8]>) {
    let (promise, _never_resumed) =
        Promise::new_yield(callback, args, Gas::from_gas(0), GasWeight(1));
    promise.detach();
}

/// Arms the next beat if at least `min_fuel` is left. Returns `false` once the chain should end.
pub fn arm_if_fuelled(callback: &str, args: impl AsRef<[u8]>, min_fuel: Gas) -> bool {
    if fuel_left() < min_fuel {
        return false;
    }
    arm(callback, args);
    true
}

/// How many more beats `fuel` pays for at `per_beat` gas each.
pub fn beats_left(fuel: Gas, per_beat: Gas) -> u64 {
    if per_beat.as_gas() == 0 { 0 } else { fuel.as_gas() / per_beat.as_gas() }
}

/// Burns gas until this call has used at least `target`.
///
/// This is how a contract can make a beat cost more on purpose. The gas is paid by whoever
/// fuelled the chain, and burned gas leaves the NEAR supply. Off-chain unit tests have no
/// real gas meter, so there it does nothing.
pub fn burn_to(target: Gas) {
    #[cfg(target_arch = "wasm32")]
    {
        const BLOCK: [u8; 1024] = [0x47; 1024];
        while env::used_gas() < target {
            let _ = env::sha256_array(BLOCK);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = target;
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;

    fn with_gas(tgas: u64) {
        testing_env!(VMContextBuilder::new().prepaid_gas(Gas::from_tgas(tgas)).build());
    }

    #[test]
    fn fuel_left_starts_at_the_prepaid_gas() {
        with_gas(1000);
        assert!(fuel_left() <= Gas::from_tgas(1000));
        assert!(fuel_left() > Gas::from_tgas(999));
    }

    #[test]
    fn arm_if_fuelled_stops_below_the_minimum() {
        with_gas(2);
        assert!(!arm_if_fuelled("on_beat", b"{}", Gas::from_tgas(3)));
    }

    #[test]
    fn arm_if_fuelled_arms_above_the_minimum() {
        with_gas(300);
        assert!(arm_if_fuelled("on_beat", b"{}", Gas::from_tgas(3)));
    }

    #[test]
    fn beats_left_divides_fuel_by_cost() {
        assert_eq!(beats_left(Gas::from_tgas(1000), Gas::from_tgas(3)), 333);
    }

    #[test]
    fn beats_left_is_zero_for_a_free_beat() {
        assert_eq!(beats_left(Gas::from_tgas(1000), Gas::from_gas(0)), 0);
    }

    #[test]
    fn burn_to_is_a_no_op_off_chain() {
        with_gas(10);
        burn_to(Gas::from_tgas(5));
        assert!(fuel_left() > Gas::from_tgas(9));
    }
}
