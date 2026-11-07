//! Hatchery: launches mortal coins in one transaction.
//!
//! Each coin is its own sub-account running the GASBAG contract, created with no access keys, so
//! nobody can change its code or rules, including whoever hatched it. A coin's whole life can be
//! watched in minutes with the speedrun rules, which is how a judge can see a birth, a rescue, a
//! death and a payout without waiting 58 days.
//!
//! The contract code is built into this one, so the hatchery itself has nothing to configure and
//! no owner. Every hatch pays for its own copy of the code, which is why it costs about 3 NEAR.

use near_sdk::json_types::U64;
use near_sdk::store::Vector;
use near_sdk::{
    AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise, PromiseError, env, near, require, serde_json,
};

/// The GASBAG contract, copied here by the build script.
const GASBAG_WASM: &[u8] = include_bytes!("../../res/gasbag.wasm");

const INIT_GAS: Gas = Gas::from_tgas(30);
const CALLBACK_GAS: Gas = Gas::from_tgas(8);
const NEAR: u128 = 1_000_000_000_000_000_000_000_000;
/// Pays for the code and the coin's starting state. Stays in the coin's account for good.
const HATCH_PRICE: u128 = 3 * NEAR + NEAR / 10;
const MAX_LABEL_BYTES: usize = 24;

/// A life of about 15 minutes. Gas per breath doubles every 3 minutes from 30 TGas.
const SPEEDRUN_RULES: &str = r#"{
  "base_price": "1000000000000000000",
  "curve_step": "50000000",
  "wage_bps_per_day": 500,
  "blocks_per_day": 600,
  "doubling_blocks": 300,
  "grace_blocks": 500,
  "min_grace_blocks": 400,
  "bounty_bps": 5000,
  "breath_tgas": 30,
  "max_breath_tgas": 990,
  "voice_account": null,
  "voice_bps": 0
}"#;

const SPEEDRUN_SOUL: &str = "You are a very small balloon with about fifteen minutes to live. You know it.";

#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct Hatched {
    pub coin: AccountId,
    pub hatcher: AccountId,
    pub name: String,
    pub symbol: String,
    pub block: u64,
}

#[near]
#[derive(BorshStorageKey)]
enum Key {
    Coins,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Hatchery {
    coins: Vector<Hatched>,
}

#[near]
impl Hatchery {
    #[init]
    pub fn new() -> Self {
        Self { coins: Vector::new(Key::Coins) }
    }

    /// Creates `<label>.<this account>` with no keys and starts a coin there. Without `rules`, it
    /// uses the speedrun rules. If anything fails, the deposit goes back to the caller.
    #[payable]
    pub fn hatch(
        &mut self,
        label: String,
        name: String,
        symbol: String,
        rules: Option<serde_json::Value>,
        soul: Option<String>,
    ) -> Promise {
        let hatcher = env::predecessor_account_id();
        let deposit = env::attached_deposit();
        require!(
            deposit.as_yoctonear() >= HATCH_PRICE,
            "HATCHERY[PRICE] attach at least 3.1 NEAR, which pays for the coin's code and state"
        );
        require!(
            !label.is_empty()
                && label.len() <= MAX_LABEL_BYTES
                && label.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "HATCHERY[LABEL] use up to 24 lowercase letters, digits and hyphens"
        );
        let coin: AccountId = format!("{label}.{}", env::current_account_id())
            .parse()
            .unwrap_or_else(|_| env::panic_str("HATCHERY[LABEL] that is not a valid account name"));
        let rules = rules.unwrap_or_else(|| serde_json::from_str(SPEEDRUN_RULES).expect("built-in rules"));
        let args = serde_json::json!({
            "name": name,
            "symbol": symbol,
            "soul": soul.unwrap_or_else(|| SPEEDRUN_SOUL.to_string()),
            "rules": rules,
        })
        .to_string()
        .into_bytes();

        // No AddKey anywhere in this batch: the coin is keyless from its first block.
        Promise::new(coin.clone())
            .create_account()
            .transfer(deposit)
            .deploy_contract(GASBAG_WASM.to_vec())
            .function_call("new", args, NearToken::from_yoctonear(0), INIT_GAS)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(CALLBACK_GAS)
                    .on_hatched(coin, hatcher, deposit, name, symbol),
            )
    }

    /// Records a new coin, or refunds the caller if the launch failed.
    #[private]
    pub fn on_hatched(
        &mut self,
        coin: AccountId,
        hatcher: AccountId,
        deposit: NearToken,
        name: String,
        symbol: String,
        #[callback_result] result: Result<(), PromiseError>,
    ) -> bool {
        if result.is_err() {
            Promise::new(hatcher).transfer(deposit).detach();
            return false;
        }
        self.coins.push(Hatched { coin, hatcher, name, symbol, block: env::block_height() });
        true
    }

    /// Every coin hatched here, newest first.
    pub fn coins(&self, from: Option<u32>, limit: Option<u32>) -> Vec<Hatched> {
        let total = self.coins.len();
        let skip = from.unwrap_or(0);
        let take = limit.unwrap_or(50).min(100);
        (0..total.saturating_sub(skip).min(take))
            .filter_map(|i| self.coins.get(total - 1 - skip - i).cloned())
            .collect()
    }

    pub fn total(&self) -> U64 {
        U64(self.coins.len() as u64)
    }

    pub fn hatch_price(&self) -> NearToken {
        NearToken::from_yoctonear(HATCH_PRICE)
    }

    pub fn speedrun_rules(&self) -> serde_json::Value {
        serde_json::from_str(SPEEDRUN_RULES).expect("built-in rules")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;

    fn at(who: &str, deposit: u128) {
        testing_env!(
            VMContextBuilder::new()
                .current_account_id("hatchery.testnet".parse().unwrap())
                .predecessor_account_id(who.parse().unwrap())
                .attached_deposit(NearToken::from_yoctonear(deposit))
                .prepaid_gas(Gas::from_tgas(300))
                .build()
        );
    }

    fn hatch(h: &mut Hatchery, label: &str) -> Promise {
        h.hatch(label.into(), "Tiny".into(), "TINY".into(), None, None)
    }

    #[test]
    fn the_speedrun_rules_are_valid_json_with_every_field() {
        at("anyone.testnet", 0);
        let r = Hatchery::new().speedrun_rules();
        for key in ["base_price", "curve_step", "wage_bps_per_day", "blocks_per_day", "doubling_blocks", "grace_blocks",
            "min_grace_blocks", "bounty_bps", "breath_tgas", "max_breath_tgas", "voice_account", "voice_bps"]
        {
            assert!(r.get(key).is_some(), "missing {key}");
        }
    }

    #[test]
    #[should_panic(expected = "HATCHERY[PRICE]")]
    fn a_hatch_below_the_price_fails() {
        at("alice.testnet", HATCH_PRICE - 1);
        let mut h = Hatchery::new();
        hatch(&mut h, "tiny");
    }

    #[test]
    #[should_panic(expected = "HATCHERY[LABEL]")]
    fn an_uppercase_label_fails() {
        at("alice.testnet", HATCH_PRICE);
        let mut h = Hatchery::new();
        hatch(&mut h, "Tiny");
    }

    #[test]
    #[should_panic(expected = "HATCHERY[LABEL]")]
    fn a_dotted_label_fails() {
        at("alice.testnet", HATCH_PRICE);
        let mut h = Hatchery::new();
        hatch(&mut h, "a.b");
    }

    #[test]
    #[should_panic(expected = "HATCHERY[LABEL]")]
    fn a_long_label_fails() {
        at("alice.testnet", HATCH_PRICE);
        let mut h = Hatchery::new();
        hatch(&mut h, &"a".repeat(25));
    }

    #[test]
    fn the_coin_list_is_newest_first_and_pages() {
        at("hatchery.testnet", 0);
        let mut h = Hatchery::new();
        for i in 0..5 {
            h.coins.push(Hatched {
                coin: format!("c{i}.hatchery.testnet").parse().unwrap(),
                hatcher: "alice.testnet".parse().unwrap(),
                name: "x".into(),
                symbol: "X".into(),
                block: i,
            });
        }
        let page = h.coins(Some(1), Some(2));
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].block, 3);
        assert_eq!(page[1].block, 2);
        assert_eq!(h.total().0, 5);
    }
}
