//! Unit tests. Each one drives the contract the way the chain would: a call at a given block,
//! with a given caller, deposit and gas. A yield timeout is simulated by calling `on_breath`
//! at the block it would fire.

use super::*;
use near_sdk::test_utils::{VMContextBuilder, get_logs};
use near_sdk::testing_env;

const NEAR: u128 = 1_000_000_000_000_000_000_000_000;
const BIRTH: u64 = 1_000;
const BEAT: u64 = 200;

fn acct(name: &str) -> AccountId {
    name.parse().unwrap()
}

fn at(block: u64, who: &str, deposit: u128, tgas: u64) {
    testing_env!(
        VMContextBuilder::new()
            .current_account_id(acct("gasbag.testnet"))
            .predecessor_account_id(acct(who))
            .signer_account_id(acct(who))
            .block_height(block)
            .attached_deposit(NearToken::from_yoctonear(deposit))
            .prepaid_gas(Gas::from_tgas(tgas))
            .storage_usage(1_000_000)
            .account_balance(NearToken::from_near(1_000_000))
            .build()
    );
}

/// The launch parameters proposed for the main coin, on a 0.6 s block.
fn launch_rules() -> Rules {
    Rules {
        base_price: U128(NEAR / 1_000_000),
        curve_step: U128(50_000_000),
        wage_bps_per_day: 50,
        blocks_per_day: 144_000,
        doubling_blocks: 1_008_000,
        grace_blocks: 36_000,
        min_grace_blocks: 400,
        bounty_bps: 5_000,
        breath_tgas: 3,
        max_breath_tgas: 990,
        voice_account: None,
        voice_bps: 0,
    }
}

fn coin(rules: Rules) -> Gasbag {
    at(BIRTH, "deployer.testnet", 0, 300);
    // Each coin starts on empty storage, even when one test launches two.
    near_sdk::mock::with_mocked_blockchain(|chain| drop(chain.take_storage()));
    Gasbag::new("Gasbag".into(), "GASBAG".into(), "You are a balloon.".into(), rules)
}

fn launch() -> Gasbag {
    coin(launch_rules())
}

fn buy(c: &mut Gasbag, block: u64, who: &str, near: u128) -> u128 {
    at(block, who, near * NEAR, 100);
    c.buy(None).0
}

fn register(c: &mut Gasbag, block: u64, who: &str) {
    at(block, who, Gasbag::storage_fee().as_yoctonear(), 30);
    c.storage_deposit(None, None);
}

fn ignite(c: &mut Gasbag, block: u64, who: &str) {
    at(block, who, 0, 1000);
    c.ignite();
}

fn breathe(c: &mut Gasbag, block: u64, who: &str, fuel_tgas: u64) {
    let id = c.lungs.get(&acct(who)).expect("lung").id;
    at(block, "gasbag.testnet", 0, fuel_tgas);
    c.on_breath(acct(who), id);
}

fn balance(c: &Gasbag, who: &str) -> u128 {
    c.ft_balance_of(acct(who)).0
}

fn tokens(n: u128) -> u128 {
    n * ONE_TOKEN
}

#[track_caller]
fn assert_close(actual: u128, expected: u128, tolerance: u128) {
    let gap = actual.abs_diff(expected);
    assert!(gap <= tolerance, "{actual} differs from {expected} by {gap}");
}

// ---- Launch rules ------------------------------------------------------------------------------

#[test]
fn a_new_coin_is_in_apnea_with_no_supply() {
    let c = launch();
    at(BIRTH + 1, "anyone.testnet", 0, 30);
    let v = c.vitals();
    assert_eq!(v.status, "apnea");
    assert_eq!(v.supply.0, 0);
    assert_eq!(v.deadline_block.0, BIRTH + 36_000);
}

#[test]
fn rules_can_be_read_back_exactly() {
    let c = launch();
    let r = c.rules();
    assert_eq!(r.wage_bps_per_day, 50);
    assert_eq!(r.doubling_blocks, 1_008_000);
    assert_eq!(c.identity().soul, "You are a balloon.");
}

#[test]
fn old_age_comes_at_about_day_58_with_launch_rules() {
    let c = launch();
    let days = (c.old_age_block - BIRTH) as f64 / 144_000.0;
    assert!((57.9..58.1).contains(&days), "{days}");
}

#[test]
#[should_panic(expected = "GASBAG[RULES] grace must outlast two beats")]
fn a_grace_under_two_beats_is_refused() {
    let mut r = launch_rules();
    r.min_grace_blocks = 399;
    coin(r);
}

#[test]
#[should_panic(expected = "GASBAG[RULES] wage is capped")]
fn an_excessive_wage_is_refused() {
    let mut r = launch_rules();
    r.wage_bps_per_day = 1_001;
    coin(r);
}

#[test]
#[should_panic(expected = "GASBAG[RULES] the last breath must fit")]
fn a_breath_limit_above_the_protocol_limit_is_refused() {
    let mut r = launch_rules();
    r.max_breath_tgas = 999;
    coin(r);
}

#[test]
#[should_panic(expected = "GASBAG[RULES] a voice budget needs a voice account")]
fn a_voice_budget_without_a_voice_is_refused() {
    let mut r = launch_rules();
    r.voice_bps = 100;
    coin(r);
}

// ---- Buying --------------------------------------------------------------------------------------

#[test]
fn buying_registers_mints_and_fills_the_estate() {
    let mut c = launch();
    let got = buy(&mut c, BIRTH + 5, "alice.testnet", 10);
    let fee = Gasbag::storage_fee().as_yoctonear();
    assert_eq!(c.estate, 10 * NEAR - fee);
    assert_eq!(balance(&c, "alice.testnet"), got);
    assert_eq!(c.ft_total_supply().0, got);
    assert!(got > tokens(9_100_000) && got < tokens(9_200_000), "{got}");
    assert_eq!(c.holders, 1);
}

#[test]
fn a_second_buyer_pays_more_per_token() {
    let mut c = launch();
    let first = buy(&mut c, BIRTH + 5, "alice.testnet", 100);
    let second = buy(&mut c, BIRTH + 6, "bob.testnet", 100);
    assert!(second < first);
}

#[test]
fn a_registered_buyer_pays_no_fee() {
    let mut c = launch();
    buy(&mut c, BIRTH + 5, "alice.testnet", 1);
    let before = c.estate;
    buy(&mut c, BIRTH + 6, "alice.testnet", 1);
    assert_eq!(c.estate, before + NEAR);
}

#[test]
#[should_panic(expected = "GASBAG[SLIPPAGE]")]
fn a_buy_below_the_callers_minimum_fails() {
    let mut c = launch();
    at(BIRTH + 5, "alice.testnet", NEAR, 100);
    c.buy(Some(U128(tokens(2_000_000))));
}

#[test]
#[should_panic(expected = "GASBAG[MIN_BUY]")]
fn a_dust_buy_fails() {
    let mut c = launch();
    buy(&mut c, BIRTH + 5, "alice.testnet", 1);
    at(BIRTH + 6, "alice.testnet", MIN_BUY - 1, 100);
    c.buy(None);
}

#[test]
fn the_voice_share_goes_to_the_budget_and_not_the_estate() {
    let mut r = launch_rules();
    r.voice_account = Some(acct("voice.testnet"));
    r.voice_bps = 100;
    let mut c = coin(r);
    buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    let net = 100 * NEAR - Gasbag::storage_fee().as_yoctonear();
    assert_eq!(c.voice_budget, net / 100);
    assert_eq!(c.estate, net - net / 100);
}

// ---- Lungs -----------------------------------------------------------------------------------------

#[test]
#[should_panic(expected = "GASBAG[REGISTER]")]
fn an_unregistered_account_cant_ignite() {
    let mut c = launch();
    ignite(&mut c, BIRTH + 1, "stranger.testnet");
}

#[test]
#[should_panic(expected = "GASBAG[ONE_LUNG]")]
fn one_account_cant_run_two_lungs() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 1);
    ignite(&mut c, BIRTH + 2, "alice.testnet");
    ignite(&mut c, BIRTH + 3, "alice.testnet");
}

#[test]
#[should_panic(expected = "GASBAG[FUEL]")]
fn a_lung_without_enough_gas_is_refused() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 1);
    at(BIRTH + 2, "alice.testnet", 0, 7);
    c.ignite();
}

#[test]
fn the_fuel_needed_rises_with_age() {
    let c = launch();
    assert_eq!(c.breath_gas(BIRTH), Gas::from_tgas(3).as_gas());
    assert_eq!(c.breath_gas(BIRTH + 1_008_000), Gas::from_tgas(6).as_gas());
    assert_eq!(c.breath_gas(BIRTH + 1_512_000), Gas::from_tgas(9).as_gas());
}

#[test]
fn igniting_counts_as_a_breath_for_the_deadline() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 1);
    ignite(&mut c, BIRTH + 35_990, "alice.testnet");
    assert_eq!(c.last_breath_block, BIRTH + 35_990);
    assert!(c.deadline_block > BIRTH + 70_000);
}

#[test]
fn a_breath_pays_the_wage_for_the_blocks_since_the_last_one() {
    let mut c = launch();
    let got = buy(&mut c, BIRTH + 10, "alice.testnet", 100);
    ignite(&mut c, BIRTH + 10, "alice.testnet");
    breathe(&mut c, BIRTH + 210, "alice.testnet", 900);
    // 0.5% a day, 200 blocks of a 144,000-block day. No gap before the lung, so no bounty.
    let expected = got * 50 / 10_000 * 200 / 144_000;
    assert_close(balance(&c, "alice.testnet") - got, expected, ONE_TOKEN);
    assert_eq!(c.breaths, 1);
    assert_eq!(c.vitals_status(), "breathing");
}

#[test]
fn two_lungs_split_wages_equally_even_with_unequal_holdings() {
    let mut c = launch();
    let whale = buy(&mut c, BIRTH + 10, "whale.testnet", 1_000);
    let shrimp = buy(&mut c, BIRTH + 10, "shrimp.testnet", 1);
    ignite(&mut c, BIRTH + 10, "whale.testnet");
    ignite(&mut c, BIRTH + 10, "shrimp.testnet");
    breathe(&mut c, BIRTH + 210, "whale.testnet", 900);
    breathe(&mut c, BIRTH + 210, "shrimp.testnet", 900);
    let whale_wage = balance(&c, "whale.testnet") - whale;
    let shrimp_wage = balance(&c, "shrimp.testnet") - shrimp;
    assert!(whale_wage > 0);
    assert_close(whale_wage, shrimp_wage, ONE_TOKEN);
}

#[test]
fn splitting_into_more_accounts_earns_nothing_extra() {
    // One operator with one lung, against the same operator with four, while an honest lung
    // runs alongside. The operator's total share of wages must follow its lung count, and each
    // extra lung costs a full lung of gas.
    fn run(sybils: usize) -> (u128, u128) {
        let mut c = launch();
        buy(&mut c, BIRTH + 2, "honest.testnet", 100);
        let names: Vec<String> = (0..sybils).map(|i| format!("sybil{i}.testnet")).collect();
        for n in &names {
            register(&mut c, BIRTH + 2, n);
        }
        ignite(&mut c, BIRTH + 2, "honest.testnet");
        for n in &names {
            ignite(&mut c, BIRTH + 2, n);
        }
        let start = balance(&c, "honest.testnet");
        for beat in 1..=10 {
            let block = BIRTH + 2 + beat * BEAT;
            breathe(&mut c, block, "honest.testnet", 900);
            for n in &names {
                breathe(&mut c, block, n, 900);
            }
        }
        let wage_per_lung = balance(&c, "honest.testnet") - start;
        let operator_total: u128 = names.iter().map(|n| balance(&c, n)).sum();
        (operator_total, wage_per_lung)
    }
    let (one, one_each) = run(1);
    let (four, four_each) = run(4);
    assert!(four > one, "more lungs do earn more in total");
    let per_lung_one = one;
    let per_lung_four = four / 4;
    assert!(per_lung_four < per_lung_one, "but each lung earns less");
    assert!(four_each < one_each);
    assert_close(four, four_each * 4, ONE_TOKEN);
}

#[test]
fn a_stale_breath_is_ignored() {
    let mut c = launch();
    let got = buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    ignite(&mut c, BIRTH + 10, "alice.testnet");
    at(BIRTH + 210, "gasbag.testnet", 0, 900);
    c.on_breath(acct("alice.testnet"), 999);
    assert_eq!(c.breaths, 0);
    assert_eq!(balance(&c, "alice.testnet"), got);
}

#[test]
fn a_breath_for_an_unknown_lung_does_nothing() {
    let mut c = launch();
    at(BIRTH + 210, "gasbag.testnet", 0, 900);
    c.on_breath(acct("ghost.testnet"), 1);
    assert_eq!(c.breaths, 0);
}

#[test]
fn a_lung_out_of_fuel_takes_its_last_gasp() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    ignite(&mut c, BIRTH + 10, "alice.testnet");
    breathe(&mut c, BIRTH + 210, "alice.testnet", 4);
    assert!(c.lung_of(acct("alice.testnet")).is_none());
    assert!(get_logs().iter().any(|l| l.contains("last_gasp")));
    assert_eq!(c.vitals_status(), "apnea");
}

#[test]
fn a_fuelled_lung_keeps_going() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    ignite(&mut c, BIRTH + 10, "alice.testnet");
    for beat in 1..=5 {
        breathe(&mut c, BIRTH + 10 + beat * BEAT, "alice.testnet", 900);
    }
    assert_eq!(c.lung_of(acct("alice.testnet")).unwrap().breaths, 5);
    assert_eq!(c.breaths, 5);
}

#[test]
fn a_failed_lung_can_be_reaped_by_anyone() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    ignite(&mut c, BIRTH + 10, "alice.testnet");
    at(BIRTH + 10 + 3 * BEAT + 1, "anyone.testnet", 0, 30);
    c.reap(acct("alice.testnet"));
    assert!(c.lung_of(acct("alice.testnet")).is_none());
}

#[test]
#[should_panic(expected = "GASBAG[REAP] that lung is healthy")]
fn a_healthy_lung_cant_be_reaped() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    ignite(&mut c, BIRTH + 10, "alice.testnet");
    at(BIRTH + 10 + 3 * BEAT, "griefer.testnet", 0, 30);
    c.reap(acct("alice.testnet"));
}

// ---- Apnea and rescue --------------------------------------------------------------------------

#[test]
fn half_the_missed_wages_build_a_bounty_and_half_are_lost() {
    let mut c = launch();
    let got = buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    at(BIRTH + 14_401, "alice.testnet", 0, 30);
    c.accrue(BIRTH + 14_401);
    // A tenth of a day at 0.5% a day.
    let missed = got * 50 / 10_000 / 10;
    assert_close(c.pot, missed / 2, ONE_TOKEN);
    assert_close(c.lost, missed / 2, ONE_TOKEN);
}

#[test]
fn vitals_show_the_bounty_growing_before_anyone_calls() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    at(BIRTH + 7_201, "anyone.testnet", 0, 30);
    let early = c.vitals().rescue_bounty.0;
    at(BIRTH + 14_401, "anyone.testnet", 0, 30);
    let later = c.vitals().rescue_bounty.0;
    assert!(early > 0);
    assert_close(later, early * 2, ONE_TOKEN);
}

#[test]
fn the_first_breath_after_apnea_collects_the_bounty_and_restarts_the_streak() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    register(&mut c, BIRTH + 2, "bob.testnet");
    ignite(&mut c, BIRTH + 14_401, "bob.testnet");
    assert_eq!(balance(&c, "bob.testnet"), 0, "the bounty waits for a real breath");
    let bounty = c.pot;
    assert!(bounty > 0);
    breathe(&mut c, BIRTH + 14_601, "bob.testnet", 900);
    assert!(balance(&c, "bob.testnet") >= bounty);
    assert_eq!(c.pot, 0);
    assert!(get_logs().iter().any(|l| l.contains("rescue")));
    assert_eq!(c.streak_start_block, BIRTH + 14_401);
}

#[test]
fn a_new_lung_does_not_reset_a_running_streak() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    buy(&mut c, BIRTH + 1, "bob.testnet", 100);
    ignite(&mut c, BIRTH + 10, "alice.testnet");
    ignite(&mut c, BIRTH + 150, "bob.testnet");
    assert_eq!(c.streak_start_block, BIRTH + 10);
}

#[test]
fn the_best_streak_survives_apnea() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    ignite(&mut c, BIRTH + 10, "alice.testnet");
    breathe(&mut c, BIRTH + 210, "alice.testnet", 900);
    breathe(&mut c, BIRTH + 410, "alice.testnet", 4);
    assert_eq!(c.best_streak_blocks, 400);
    ignite(&mut c, BIRTH + 1_000, "alice.testnet");
    assert_eq!(c.best_streak_blocks, 400);
    assert_eq!(c.streak_start_block, BIRTH + 1_000);
}

// ---- Death -----------------------------------------------------------------------------------------

#[test]
fn it_flatlines_one_block_after_the_deadline() {
    let c = launch();
    at(BIRTH + 36_000, "anyone.testnet", 0, 30);
    assert_eq!(c.vitals_status(), "apnea");
    at(BIRTH + 36_001, "anyone.testnet", 0, 30);
    assert_eq!(c.vitals_status(), "dead");
    assert_eq!(c.vitals().cause, Some(Cause::Flatline));
}

#[test]
fn grace_shrinks_with_age_but_stops_at_the_minimum() {
    let c = launch();
    assert_eq!(c.grace_at(BIRTH), 36_000);
    assert_eq!(c.grace_at(BIRTH + 1_008_000), 18_000);
    assert_eq!(c.grace_at(BIRTH + 7_560_000), 400);
}

#[test]
fn it_dies_of_old_age_even_with_lungs_running() {
    let mut r = launch_rules();
    r.doubling_blocks = 10_000;
    r.grace_blocks = 1_000_000;
    let mut c = coin(r);
    buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    ignite(&mut c, BIRTH + 2, "alice.testnet");
    let last = c.old_age_block;
    breathe(&mut c, last - 1, "alice.testnet", 1000);
    assert!(c.death.is_none());
    breathe(&mut c, last, "alice.testnet", 1000);
    assert_eq!(c.death.as_ref().unwrap().cause, Cause::OldAge);
    assert!(c.lung_of(acct("alice.testnet")).is_none());
}

#[test]
#[should_panic(expected = "GASBAG[DEAD]")]
fn a_dead_coin_cant_be_bought() {
    let mut c = launch();
    buy(&mut c, BIRTH + 36_001, "alice.testnet", 1);
}

#[test]
#[should_panic(expected = "GASBAG[DEAD]")]
fn a_dead_coin_cant_be_revived() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 1);
    ignite(&mut c, BIRTH + 36_001, "alice.testnet");
}

#[test]
fn a_running_lung_ends_when_it_finds_the_coin_dead() {
    let mut r = launch_rules();
    r.grace_blocks = 400;
    let mut c = coin(r);
    buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    ignite(&mut c, BIRTH + 2, "alice.testnet");
    // The chain was delayed past the deadline.
    breathe(&mut c, BIRTH + 2 + 401, "alice.testnet", 900);
    assert!(c.lung_of(acct("alice.testnet")).is_none());
    assert_eq!(c.death.as_ref().unwrap().cause, Cause::Flatline);
    assert_eq!(c.breaths, 0, "a dead coin does not breathe");
}

#[test]
fn tokens_still_move_after_death() {
    let mut c = launch();
    let got = buy(&mut c, BIRTH + 1, "alice.testnet", 1);
    buy(&mut c, BIRTH + 1, "bob.testnet", 1);
    at(BIRTH + 36_001, "alice.testnet", 1, 30);
    c.ft_transfer(acct("bob.testnet"), U128(got), None);
    assert_eq!(balance(&c, "alice.testnet"), 0);
}

// ---- The will --------------------------------------------------------------------------------------

#[test]
#[should_panic(expected = "GASBAG[ALIVE] the estate opens at death")]
fn the_estate_is_locked_while_it_lives() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 1);
    at(BIRTH + 2, "alice.testnet", 0, 30);
    c.claim();
}

#[test]
#[should_panic(expected = "GASBAG[NOTHING]")]
fn claiming_with_no_tokens_fails() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 1);
    at(BIRTH + 36_001, "stranger.testnet", 0, 30);
    c.claim();
}

#[test]
fn claims_are_pro_rata_and_burn_the_tokens() {
    let mut c = launch();
    let a = buy(&mut c, BIRTH + 1, "alice.testnet", 300);
    let b = buy(&mut c, BIRTH + 2, "bob.testnet", 100);
    let estate = c.estate;
    let supply = a + b;

    at(BIRTH + 36_001, "alice.testnet", 0, 30);
    let got_a = c.claim().0;
    assert_close(got_a, mul_div(estate, a, supply), 1);
    assert_eq!(balance(&c, "alice.testnet"), 0);
    assert_eq!(c.ft_total_supply().0, b);

    at(BIRTH + 36_002, "bob.testnet", 0, 30);
    let got_b = c.claim().0;
    assert_close(got_b, mul_div(estate, b, supply), 2);
    assert_eq!(c.ft_total_supply().0, 0);
    assert!(got_a + got_b <= estate, "the estate can never overpay");
    assert!(estate - (got_a + got_b) < 3);
}

#[test]
fn claim_order_does_not_change_anyones_share() {
    fn run(first: &str, second: &str) -> (u128, u128) {
        let mut c = launch();
        buy(&mut c, BIRTH + 1, "alice.testnet", 300);
        buy(&mut c, BIRTH + 2, "bob.testnet", 100);
        at(BIRTH + 36_001, first, 0, 30);
        let x = c.claim().0;
        at(BIRTH + 36_002, second, 0, 30);
        let y = c.claim().0;
        if first == "alice.testnet" { (x, y) } else { (y, x) }
    }
    let (a1, b1) = run("alice.testnet", "bob.testnet");
    let (a2, b2) = run("bob.testnet", "alice.testnet");
    assert_close(a1, a2, 2);
    assert_close(b1, b2, 2);
}

#[test]
#[should_panic(expected = "GASBAG[NOTHING]")]
fn nobody_can_claim_twice() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    at(BIRTH + 36_001, "alice.testnet", 0, 30);
    c.claim();
    at(BIRTH + 36_002, "alice.testnet", 0, 30);
    c.claim();
}

#[test]
fn a_nurse_with_no_purchase_inherits_from_wages() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "buyer.testnet", 100);
    register(&mut c, BIRTH + 1, "nurse.testnet");
    ignite(&mut c, BIRTH + 2, "nurse.testnet");
    for beat in 1..=10 {
        breathe(&mut c, BIRTH + 2 + beat * BEAT, "nurse.testnet", 900);
    }
    let dead_at = c.deadline_block + 1;
    at(dead_at, "nurse.testnet", 0, 30);
    assert!(c.claim().0 > 0);
}

#[test]
fn death_is_recorded_once_with_the_final_numbers() {
    let mut c = launch();
    let got = buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    ignite(&mut c, BIRTH + 2, "alice.testnet");
    breathe(&mut c, BIRTH + 202, "alice.testnet", 4);
    let wages = c.wages;
    at(c.deadline_block + 1, "anyone.testnet", 0, 30);
    c.pronounce();
    let d = c.death().unwrap();
    assert_eq!(d.supply.0, got + wages);
    assert_eq!(d.last_lung, Some(acct("alice.testnet")));
    assert_eq!(d.breaths, 1);
    let log_len = c.log_total;
    c.pronounce();
    assert_eq!(c.log_total, log_len, "a second pronouncement changes nothing");
}

#[test]
#[should_panic(expected = "GASBAG[ALIVE] it is still alive")]
fn it_cant_be_pronounced_early() {
    let mut c = launch();
    at(BIRTH + 36_000, "anyone.testnet", 0, 30);
    c.pronounce();
}

// ---- Voice -------------------------------------------------------------------------------------------

fn voiced() -> Gasbag {
    let mut r = launch_rules();
    r.voice_account = Some(acct("voice.testnet"));
    r.voice_bps = 100;
    coin(r)
}

#[test]
fn only_the_voice_account_speaks_and_speech_is_kept_newest_first() {
    let mut c = voiced();
    at(BIRTH + 1, "voice.testnet", 0, 30);
    c.say("first".into(), None);
    at(BIRTH + 2, "voice.testnet", 0, 30);
    c.say("second".into(), Some("sig".into()));
    let s = c.speech(None);
    assert_eq!(s.len(), 2);
    assert_eq!(s[0].text, "second");
    assert_eq!(s[0].proof.as_deref(), Some("sig"));
}

#[test]
#[should_panic(expected = "GASBAG[VOICE] only the voice account speaks")]
fn a_stranger_cant_speak_for_the_coin() {
    let mut c = voiced();
    at(BIRTH + 1, "alice.testnet", 0, 30);
    c.say("hello".into(), None);
}

#[test]
#[should_panic(expected = "GASBAG[VOICE] speech must be")]
fn long_speech_is_refused() {
    let mut c = voiced();
    at(BIRTH + 1, "voice.testnet", 0, 30);
    c.say("x".repeat(281), None);
}

#[test]
fn the_voice_can_draw_only_its_share() {
    let mut c = voiced();
    buy(&mut c, BIRTH + 1, "alice.testnet", 1);
    buy(&mut c, BIRTH + 2, "alice.testnet", 100);
    let estate = c.estate;
    at(BIRTH + 3, "voice.testnet", 0, 30);
    let drawn = c.draw_voice_budget().0;
    assert!(drawn > 0 && drawn < 2 * NEAR);
    assert_eq!(c.voice_budget, 0);
    assert_eq!(c.estate, estate, "the estate is untouched");
}

#[test]
#[should_panic(expected = "GASBAG[VOICE] only the voice account draws")]
fn a_stranger_cant_draw_the_voice_budget() {
    let mut c = voiced();
    buy(&mut c, BIRTH + 1, "alice.testnet", 1);
    at(BIRTH + 3, "alice.testnet", 0, 30);
    c.draw_voice_budget();
}

#[test]
#[should_panic(expected = "GASBAG[VOICE] only the voice account speaks")]
fn a_silent_coin_refuses_any_speaker() {
    let mut c = launch();
    at(BIRTH + 1, "alice.testnet", 0, 30);
    c.say("hi".into(), None);
}

// ---- Views and standards --------------------------------------------------------------------------

#[test]
fn the_log_wraps_and_returns_newest_first() {
    let mut c = launch();
    for i in 0..(LOG_CAP + 10) {
        buy(&mut c, BIRTH + 1 + i, "alice.testnet", 1);
    }
    at(BIRTH + 500, "anyone.testnet", 0, 30);
    let log = c.log(Some(200));
    assert_eq!(log.len() as u64, LOG_CAP);
    assert_eq!(log[0].n, LOG_CAP + 9);
    assert_eq!(log[(LOG_CAP - 1) as usize].n, 10);
}

#[test]
fn the_daily_table_tracks_supply_and_money() {
    let mut c = launch();
    let got = buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    ignite(&mut c, BIRTH + 2, "alice.testnet");
    breathe(&mut c, BIRTH + 202, "alice.testnet", 900);
    at(BIRTH + 300, "anyone.testnet", 0, 30);
    let days = c.days(None, None);
    assert_eq!(days.len(), 1);
    let (index, row) = &days[0];
    assert_eq!(*index, 0);
    assert_eq!(row.bought.0, got);
    assert_eq!(row.breaths, 1);
    assert_eq!(row.supply_end.0, c.ft_total_supply().0);
}

#[test]
fn the_icon_follows_the_coins_state() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 1);
    at(BIRTH + 2, "anyone.testnet", 0, 30);
    assert_eq!(c.ft_metadata().icon.as_deref(), Some(metadata::ICON_GASPING));
    ignite(&mut c, BIRTH + 3, "alice.testnet");
    assert_eq!(c.ft_metadata().icon.as_deref(), Some(metadata::ICON_BREATHING));
    at(c.deadline_block + 1, "anyone.testnet", 0, 30);
    assert_eq!(c.ft_metadata().icon.as_deref(), Some(metadata::ICON_DEAD));
}

#[test]
fn metadata_is_valid_nep148() {
    let c = launch();
    at(BIRTH + 1, "anyone.testnet", 0, 30);
    let m = c.ft_metadata();
    assert_eq!(m.spec, FT_METADATA_SPEC);
    assert_eq!(m.decimals, 18);
    assert_eq!(m.symbol, "GASBAG");
}

#[test]
fn ft_transfer_moves_tokens_between_registered_accounts() {
    let mut c = launch();
    let got = buy(&mut c, BIRTH + 1, "alice.testnet", 1);
    buy(&mut c, BIRTH + 2, "bob.testnet", 1);
    let bob = balance(&c, "bob.testnet");
    at(BIRTH + 3, "alice.testnet", 1, 30);
    c.ft_transfer(acct("bob.testnet"), U128(tokens(1)), None);
    assert_eq!(balance(&c, "alice.testnet"), got - tokens(1));
    assert_eq!(balance(&c, "bob.testnet"), bob + tokens(1));
}

#[test]
#[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
fn ft_transfer_requires_one_yocto() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 1);
    buy(&mut c, BIRTH + 2, "bob.testnet", 1);
    at(BIRTH + 3, "alice.testnet", 0, 30);
    c.ft_transfer(acct("bob.testnet"), U128(1), None);
}

#[test]
fn storage_deposit_registers_once_and_refunds_extra() {
    let mut c = launch();
    let fee = Gasbag::storage_fee().as_yoctonear();
    at(BIRTH + 1, "alice.testnet", fee, 30);
    c.storage_deposit(None, None);
    assert!(c.storage_balance_of(acct("alice.testnet")).is_some());
    assert_eq!(c.holders, 1);
    at(BIRTH + 2, "alice.testnet", fee, 30);
    c.storage_deposit(None, None);
    assert_eq!(c.holders, 1);
}

#[test]
#[should_panic(expected = "GASBAG[STORAGE] the registration fee was not attached")]
fn storage_deposit_needs_the_fee() {
    let mut c = launch();
    at(BIRTH + 1, "alice.testnet", 1, 30);
    c.storage_deposit(None, None);
}

#[test]
fn storage_cant_be_withdrawn_or_unregistered() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 1);
    at(BIRTH + 2, "alice.testnet", 1, 30);
    assert!(!c.storage_unregister(Some(true)));
    assert!(c.storage_balance_of(acct("alice.testnet")).is_some());
}

#[test]
fn storage_fee_covers_a_balance_and_a_lung_for_the_longest_account_id() {
    let mut c = launch();
    let longest = format!("{}.testnet", "a".repeat(56));
    assert_eq!(longest.len(), 64);
    at(BIRTH + 1, &longest, 0, 30);
    let id = acct(&longest);
    let before = env::storage_usage();
    c.register(&id);
    c.lungs.insert(
        id.clone(),
        Lung {
            id: u64::MAX,
            started_block: u64::MAX,
            last_breath_block: u64::MAX,
            breaths: u32::MAX,
            fuel_tgas: u32::MAX,
            paid_index: U128(u128::MAX),
            paid_total: U128(u128::MAX),
        },
    );
    c.lungs.flush();
    let used = env::storage_usage() - before;
    assert!(used <= STORAGE_BYTES_PER_ACCOUNT, "{used} bytes");
}

#[test]
fn the_estate_only_changes_through_buys_and_claims() {
    let mut c = launch();
    buy(&mut c, BIRTH + 1, "alice.testnet", 100);
    let estate = c.estate;
    ignite(&mut c, BIRTH + 2, "alice.testnet");
    for beat in 1..=20 {
        breathe(&mut c, BIRTH + 2 + beat * BEAT, "alice.testnet", 900);
    }
    assert_eq!(c.estate, estate);
}

impl Gasbag {
    fn vitals_status(&self) -> String {
        self.vitals().status
    }
}
