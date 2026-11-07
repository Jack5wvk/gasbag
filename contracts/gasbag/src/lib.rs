//! GASBAG: a NEP-141 memecoin that breathes NEAR gas.
//!
//! The coin stays alive through *lungs*. A lung is a chain of receipts started by one
//! transaction. It wakes this contract every 200 blocks through a yield timeout, with no keeper,
//! and it ends when its gas runs out. While any lung runs, wages are minted to the lungs. All the
//! NEAR paid for tokens goes into an estate that has no withdraw path. With no breath for the
//! grace period, or once a single breath no longer fits in a transaction, the coin is dead for
//! good and holders burn their tokens for a pro-rata share of the estate.
//!
//! There is no owner, no setter and no pause. Every parameter is fixed in [`Gasbag::new`] and
//! readable through [`Gasbag::rules`]. Panics name the rule that stopped the call, as
//! `GASBAG[RULE] reason`.

mod math;
mod metadata;
#[cfg(test)]
mod tests;

use math::{Curve, ONE_TOKEN, age_when_doubled_reaches, doubled, halved, mul_div};
use near_contract_standards::fungible_token::FungibleToken;
use near_contract_standards::fungible_token::core::FungibleTokenCore;
use near_contract_standards::fungible_token::events::{FtBurn, FtMint};
use near_contract_standards::fungible_token::metadata::{
    FT_METADATA_SPEC, FungibleTokenMetadata, FungibleTokenMetadataProvider,
};
use near_contract_standards::fungible_token::resolver::FungibleTokenResolver;
use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use near_heartbeat as heartbeat;
use near_sdk::json_types::{U64, U128};
use near_sdk::store::{IterableMap, Lazy, LookupMap};
use near_sdk::{
    AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise, PromiseOrValue, assert_one_yocto,
    env, near, require, serde_json,
};

/// Gas kept back so a breath can always finish and write its state.
const LAST_GASP_RESERVE: Gas = Gas::from_tgas(3);
/// Gas `ignite` needs for itself, on top of one breath.
const IGNITE_OVERHEAD: Gas = Gas::from_tgas(5);
/// What a breath still spends after its metabolism burn: base fees, writing state and arming the
/// next beat. Measured on testnet, so `breath_tgas` means what a breath really costs.
const BREATH_TAIL: Gas = Gas::from_gas(2_200_000_000_000);
/// A lung this many beats overdue has failed and can be removed by anyone.
const STALE_AFTER_BEATS: u64 = 3;
/// Entries kept in the on-chain log. Older mints and burns remain in the NEP-297 events.
const LOG_CAP: u64 = 128;
const VOICE_CAP: u64 = 32;
const MAX_SPEECH_BYTES: usize = 280;
const MAX_PROOF_BYTES: usize = 300;
const MAX_SOUL_BYTES: usize = 2_000;
/// Covers one balance row and one lung row for the longest possible account id.
const STORAGE_BYTES_PER_ACCOUNT: u64 = 500;
const BPS: u128 = 10_000;
/// About a year of blocks. No schedule may be longer, so a coin can't be set up to never die.
const MAX_SCHEDULE_BLOCKS: u64 = 52_560_000;
const MIN_BUY: u128 = 1_000_000_000_000_000_000_000; // 0.001 NEAR

/// The laws of this coin. Set once at launch. Nothing can change them.
#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct Rules {
    /// yoctoNEAR per whole token when nothing has been bought.
    pub base_price: U128,
    /// The price rises by `base_price` for every `curve_step` whole tokens bought.
    pub curve_step: U128,
    /// Wages per day while breathing, in basis points of supply.
    pub wage_bps_per_day: u32,
    pub blocks_per_day: u64,
    /// Gas per breath doubles, and grace halves, every this many blocks.
    pub doubling_blocks: u64,
    /// Blocks allowed without a breath, at birth.
    pub grace_blocks: u64,
    /// Grace never falls below this.
    pub min_grace_blocks: u64,
    /// Share of missed wages that becomes the rescue bounty, in basis points.
    pub bounty_bps: u32,
    /// Gas per breath at birth.
    pub breath_tgas: u64,
    /// When a breath needs this much gas the coin dies of old age.
    pub max_breath_tgas: u64,
    /// May speak for the coin and draw its budget. `None` means it stays silent.
    pub voice_account: Option<AccountId>,
    /// Share of each purchase set aside for the voice, in basis points.
    pub voice_bps: u32,
}

#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct Identity {
    pub name: String,
    pub symbol: String,
    /// The system prompt of the coin's voice, stored here so anyone can check it.
    pub soul: String,
}

#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct Lung {
    pub id: u64,
    pub started_block: u64,
    pub last_breath_block: u64,
    pub breaths: u32,
    /// Gas left in this lung's chain after its latest breath.
    pub fuel_tgas: u32,
    /// Wage index already paid to this lung.
    pub paid_index: U128,
    pub paid_total: U128,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cause {
    /// No breath for the whole grace period.
    Flatline,
    /// One breath no longer fits in a transaction.
    OldAge,
}

#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct Death {
    pub block: u64,
    pub cause: Cause,
    pub last_lung: Option<AccountId>,
    pub supply: U128,
    pub estate: U128,
    pub breaths: u64,
    pub best_streak_blocks: u64,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Buy,
    Wage,
    Bounty,
    Claim,
    Ignite,
    LastGasp,
    Death,
}

#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct LogEntry {
    pub n: u64,
    pub block: u64,
    pub kind: Kind,
    pub who: Option<AccountId>,
    pub amount: U128,
}

/// One row per coin-day, so supply can be charted from the contract alone.
#[near(serializers = [json, borsh])]
#[derive(Clone, Default)]
pub struct Day {
    pub breaths: u32,
    pub bought: U128,
    pub near_in: U128,
    pub wages: U128,
    pub burned: U128,
    pub gas: U128,
    pub supply_end: U128,
}

#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct Speech {
    pub block: u64,
    pub text: String,
    /// Reference to a verifiable-inference proof, such as a TEE signature.
    pub proof: Option<String>,
}

#[near(serializers = [json])]
pub struct Vitals {
    /// `breathing`, `apnea` or `dead`.
    pub status: String,
    pub cause: Option<Cause>,
    pub block: U64,
    pub born_block: U64,
    pub age_blocks: U64,
    pub old_age_block: U64,
    pub last_breath_block: U64,
    /// Dead after this block unless something breathes.
    pub deadline_block: U64,
    pub grace_blocks: U64,
    pub breath_tgas: U64,
    pub lungs: u32,
    pub last_lung: Option<AccountId>,
    pub breaths: U64,
    pub streak_blocks: U64,
    pub best_streak_blocks: U64,
    pub supply: U128,
    pub bought: U128,
    pub wages: U128,
    pub burned: U128,
    /// Missed wages that will never be minted.
    pub lost: U128,
    pub rescue_bounty: U128,
    pub estate: U128,
    pub estate_left: U128,
    pub price: U128,
    /// Estate per whole token.
    pub floor: U128,
    pub voice_budget: U128,
    /// Gas burned by breaths, including an estimate of each breath's overhead.
    pub gas_burned: U128,
    pub holders: U64,
}

#[near(event_json(standard = "gasbag"))]
pub enum Event<'a> {
    #[event_version("1.0.0")]
    Ignite { owner: &'a AccountId, lung: u64, fuel_tgas: u64 },
    #[event_version("1.0.0")]
    Breath { owner: &'a AccountId, lung: u64, wage: U128, fuel_tgas: u64 },
    #[event_version("1.0.0")]
    LastGasp { owner: &'a AccountId, lung: u64, breaths: u32 },
    #[event_version("1.0.0")]
    Rescue { owner: &'a AccountId, bounty: U128 },
    #[event_version("1.0.0")]
    Death { cause: Cause, estate: U128, supply: U128 },
}

#[near]
#[derive(BorshStorageKey)]
enum Key {
    Balances,
    Identity,
    Lungs,
    Log,
    Days,
    Voice,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Gasbag {
    token: FungibleToken,
    rules: Rules,
    identity: Lazy<Identity>,
    born_block: u64,
    old_age_block: u64,
    last_breath_block: u64,
    deadline_block: u64,
    death: Option<Death>,
    lungs: IterableMap<AccountId, Lung>,
    next_lung: u64,
    last_lung: Option<AccountId>,
    /// Wages earned so far by a lung that has been alive since block zero.
    wage_index: u128,
    accrued_block: u64,
    pot: u128,
    lost: u128,
    bought: u128,
    wages: u128,
    burned: u128,
    estate: u128,
    estate_left: u128,
    voice_budget: u128,
    breaths: u64,
    gas_burned: u128,
    streak_start_block: u64,
    best_streak_blocks: u64,
    holders: u64,
    log: LookupMap<u64, LogEntry>,
    log_total: u64,
    days: LookupMap<u64, Day>,
    voice: LookupMap<u64, Speech>,
    voice_total: u64,
}

#[near]
impl Gasbag {
    #[init]
    pub fn new(name: String, symbol: String, soul: String, rules: Rules) -> Self {
        require!(!name.is_empty() && name.len() <= 32, "GASBAG[RULES] name must be 1 to 32 bytes");
        require!(!symbol.is_empty() && symbol.len() <= 12, "GASBAG[RULES] symbol must be 1 to 12 bytes");
        require!(soul.len() <= MAX_SOUL_BYTES, "GASBAG[RULES] soul is too long");
        require!(rules.base_price.0 > 0, "GASBAG[RULES] base_price must be positive");
        require!(rules.curve_step.0 > 0, "GASBAG[RULES] curve_step must be positive");
        require!(rules.wage_bps_per_day <= 1_000, "GASBAG[RULES] wage is capped at 10% a day");
        require!(rules.blocks_per_day >= heartbeat::BEAT_BLOCKS, "GASBAG[RULES] a day must be at least one beat");
        require!(
            rules.blocks_per_day <= MAX_SCHEDULE_BLOCKS
                && rules.doubling_blocks <= MAX_SCHEDULE_BLOCKS
                && rules.grace_blocks <= MAX_SCHEDULE_BLOCKS,
            "GASBAG[RULES] no schedule can be longer than a year"
        );
        require!(rules.doubling_blocks >= heartbeat::BEAT_BLOCKS, "GASBAG[RULES] doubling must be at least one beat");
        require!(
            rules.min_grace_blocks >= 2 * heartbeat::BEAT_BLOCKS,
            "GASBAG[RULES] grace must outlast two beats, or a healthy lung could flatline"
        );
        require!(rules.grace_blocks >= rules.min_grace_blocks, "GASBAG[RULES] grace is below its minimum");
        require!(rules.bounty_bps as u128 <= BPS, "GASBAG[RULES] bounty is above 100%");
        require!(rules.breath_tgas >= 5, "GASBAG[RULES] a real breath costs at least 5 TGas");
        require!(
            rules.max_breath_tgas > rules.breath_tgas
                && Gas::from_tgas(rules.max_breath_tgas).saturating_add(IGNITE_OVERHEAD) <= heartbeat::MAX_TX_GAS,
            "GASBAG[RULES] the last breath must fit in one transaction"
        );
        require!(rules.voice_bps <= 500, "GASBAG[RULES] the voice can take at most 5%");
        require!(
            rules.voice_bps == 0 || rules.voice_account.is_some(),
            "GASBAG[RULES] a voice budget needs a voice account"
        );

        let now = env::block_height();
        let old_age = age_when_doubled_reaches(
            Gas::from_tgas(rules.breath_tgas).as_gas() as u128,
            Gas::from_tgas(rules.max_breath_tgas).as_gas() as u128,
            rules.doubling_blocks,
        );
        let grace = rules.grace_blocks;
        Self {
            token: FungibleToken::new(Key::Balances),
            rules,
            identity: Lazy::new(Key::Identity, Identity { name, symbol, soul }),
            born_block: now,
            old_age_block: now + old_age,
            last_breath_block: now,
            deadline_block: now + grace,
            death: None,
            lungs: IterableMap::new(Key::Lungs),
            next_lung: 1,
            last_lung: None,
            wage_index: 0,
            accrued_block: now,
            pot: 0,
            lost: 0,
            bought: 0,
            wages: 0,
            burned: 0,
            estate: 0,
            estate_left: 0,
            voice_budget: 0,
            breaths: 0,
            gas_burned: 0,
            streak_start_block: now,
            best_streak_blocks: 0,
            holders: 0,
            log: LookupMap::new(Key::Log),
            log_total: 0,
            days: LookupMap::new(Key::Days),
            voice: LookupMap::new(Key::Voice),
            voice_total: 0,
        }
    }

    // ---- Actions anyone can take ------------------------------------------------------------

    /// Buys GASBAG on the bonding curve with the attached NEAR, and registers the caller if
    /// needed. All of it goes to the estate, less any voice share set at launch.
    #[payable]
    pub fn buy(&mut self, min_tokens: Option<U128>) -> U128 {
        let now = env::block_height();
        self.require_alive(now);
        let who = env::predecessor_account_id();
        let mut near = env::attached_deposit().as_yoctonear();
        if !self.is_registered(&who) {
            let fee = Self::storage_fee().as_yoctonear();
            require!(near > fee, "GASBAG[STORAGE] attach enough to cover registration");
            near -= fee;
            self.register(&who);
        }
        require!(near >= MIN_BUY, "GASBAG[MIN_BUY] the smallest purchase is 0.001 NEAR");

        // Wages up to now are owed on the supply before this purchase.
        self.accrue(now);
        let tokens = self.curve().tokens_for(near, self.bought);
        require!(tokens > 0, "GASBAG[MIN_BUY] that buys less than one unit");
        if let Some(min) = min_tokens {
            require!(tokens >= min.0, "GASBAG[SLIPPAGE] the curve moved past your minimum");
        }
        let voice_cut = mul_div(near, self.rules.voice_bps as u128, BPS);
        self.voice_budget += voice_cut;
        self.estate += near - voice_cut;
        self.bought += tokens;
        self.token.internal_deposit(&who, tokens);
        FtMint { owner_id: &who, amount: U128(tokens), memo: Some("buy") }.emit();
        self.note(Kind::Buy, Some(who), tokens);
        let day = self.day_index(now);
        self.update_day(day, |d, supply| {
            d.bought = U128(d.bought.0 + tokens);
            d.near_in = U128(d.near_in.0 + near);
            d.supply_end = U128(supply);
        });
        U128(tokens)
    }

    /// Attaches a lung. Whatever gas this call has left keeps the lung breathing, so send as much
    /// as you want it to run on. One lung per account. Costs no NEAR beyond the gas.
    pub fn ignite(&mut self) {
        let now = env::block_height();
        self.require_alive(now);
        let who = env::predecessor_account_id();
        require!(self.is_registered(&who), "GASBAG[REGISTER] call storage_deposit or buy first");
        require!(!self.lungs.contains_key(&who), "GASBAG[ONE_LUNG] this account already has a lung");
        let need = Gas::from_gas(self.breath_gas(now)).saturating_add(IGNITE_OVERHEAD);
        require!(
            env::prepaid_gas() >= need,
            format!("GASBAG[FUEL] a lung needs at least {} TGas at this age", need.as_tgas())
        );

        self.accrue(now);
        if self.lungs.is_empty() {
            self.streak_start_block = now;
        }
        let id = self.next_lung;
        self.next_lung += 1;
        self.lungs.insert(
            who.clone(),
            Lung {
                id,
                started_block: now,
                last_breath_block: now,
                breaths: 0,
                fuel_tgas: heartbeat::fuel_left().as_tgas() as u32,
                paid_index: U128(self.wage_index),
                paid_total: U128(0),
            },
        );
        self.touch_deadline(now);
        self.note(Kind::Ignite, Some(who.clone()), 0);
        Event::Ignite { owner: &who, lung: id, fuel_tgas: heartbeat::fuel_left().as_tgas() }.emit();
        heartbeat::arm("on_breath", Self::breath_args(&who, id));
    }

    /// One breath. Runs by itself when the yield times out. Only this contract can reach it.
    /// Must never panic: a panic would end the lung and roll this beat back.
    #[private]
    pub fn on_breath(&mut self, owner: AccountId, id: u64) {
        let now = env::block_height();
        let Some(lung) = self.lungs.get(&owner).cloned() else { return };
        if lung.id != id {
            return;
        }
        if self.cause_of_death(now).is_some() {
            self.pronounce_at(now);
            self.remove_lung(&owner, now);
            return;
        }

        self.accrue(now);
        let wage = self.wage_index - lung.paid_index.0;
        let bounty = std::mem::take(&mut self.pot);
        let pay = wage + bounty;
        if pay > 0 {
            self.token.internal_deposit(&owner, pay);
            self.wages += pay;
        }
        if wage > 0 {
            FtMint { owner_id: &owner, amount: U128(wage), memo: Some("wage") }.emit();
            self.note(Kind::Wage, Some(owner.clone()), wage);
        }
        if bounty > 0 {
            FtMint { owner_id: &owner, amount: U128(bounty), memo: Some("bounty") }.emit();
            self.note(Kind::Bounty, Some(owner.clone()), bounty);
            Event::Rescue { owner: &owner, bounty: U128(bounty) }.emit();
        }
        if let Some(entry) = self.lungs.get_mut(&owner) {
            entry.breaths += 1;
            entry.last_breath_block = now;
            entry.paid_index = U128(self.wage_index);
            entry.paid_total = U128(entry.paid_total.0 + pay);
        }
        self.breaths += 1;
        self.last_lung = Some(owner.clone());
        self.touch_deadline(now);

        // Metabolism: this breath burns its age-appropriate gas, then decides whether the fuel
        // left pays for another one.
        let cost = Gas::from_gas(self.breath_gas(now));
        let target = cost.saturating_sub(BREATH_TAIL);
        let stop_at = env::prepaid_gas().saturating_sub(LAST_GASP_RESERVE);
        heartbeat::burn_to(if target < stop_at { target } else { stop_at });
        let used = env::used_gas().saturating_add(BREATH_TAIL).as_gas() as u128;
        self.gas_burned += used;
        let day = self.day_index(now);
        self.update_day(day, |d, supply| {
            d.breaths += 1;
            d.wages = U128(d.wages.0 + pay);
            d.gas = U128(d.gas.0 + used);
            d.supply_end = U128(supply);
        });

        let fuel = heartbeat::fuel_left();
        if let Some(entry) = self.lungs.get_mut(&owner) {
            entry.fuel_tgas = fuel.as_tgas() as u32;
        }
        Event::Breath { owner: &owner, lung: id, wage: U128(wage), fuel_tgas: fuel.as_tgas() }.emit();
        if fuel >= cost.saturating_add(LAST_GASP_RESERVE) {
            heartbeat::arm("on_breath", Self::breath_args(&owner, id));
        } else {
            let breaths = lung.breaths + 1;
            self.remove_lung(&owner, now);
            self.note(Kind::LastGasp, Some(owner.clone()), 0);
            Event::LastGasp { owner: &owner, lung: id, breaths }.emit();
        }
    }

    /// Removes a lung whose chain failed, so it stops collecting wages it can't receive. Anyone
    /// can call this. A healthy lung is never overdue.
    pub fn reap(&mut self, owner: AccountId) {
        let now = env::block_height();
        let lung = self.lungs.get(&owner).cloned();
        let Some(lung) = lung else { env::panic_str("GASBAG[REAP] no such lung") };
        require!(
            now > lung.last_breath_block + STALE_AFTER_BEATS * heartbeat::BEAT_BLOCKS,
            "GASBAG[REAP] that lung is healthy"
        );
        // A lung reaped after death must not stretch the streak past the moment of death.
        self.pronounce_at(now);
        if self.death.is_none() {
            self.accrue(now);
        }
        self.remove_lung(&owner, now);
    }

    /// Records a death that has already happened by the rules. Anyone can call this, and `claim`
    /// does it too. Views report the death either way.
    pub fn pronounce(&mut self) {
        let now = env::block_height();
        require!(self.cause_of_death(now).is_some(), "GASBAG[ALIVE] it is still alive");
        self.pronounce_at(now);
    }

    /// The will. After death, burns all your GASBAG for your share of the estate.
    pub fn claim(&mut self) -> U128 {
        let now = env::block_height();
        require!(self.cause_of_death(now).is_some(), "GASBAG[ALIVE] the estate opens at death");
        self.pronounce_at(now);
        let who = env::predecessor_account_id();
        let balance = self.token.ft_balance_of(who.clone()).0;
        require!(balance > 0, "GASBAG[NOTHING] you hold no GASBAG");
        let pay = mul_div(self.estate_left, balance, self.token.total_supply);
        self.token.internal_withdraw(&who, balance);
        self.estate_left -= pay;
        self.burned += balance;
        FtBurn { owner_id: &who, amount: U128(balance), memo: Some("claim") }.emit();
        self.note(Kind::Claim, Some(who.clone()), balance);
        // Every claim lands on the day of death, so claims can't add rows to the table forever.
        let died = self.death.as_ref().map_or(now, |d| d.block);
        let day = self.day_index(died);
        self.update_day(day, |d, supply| {
            d.burned = U128(d.burned.0 + balance);
            d.supply_end = U128(supply);
        });
        Promise::new(who).transfer(NearToken::from_yoctonear(pay)).detach();
        U128(pay)
    }

    // ---- The voice ---------------------------------------------------------------------------

    /// Stores one thing the coin said. Only the voice account set at launch can do this, and it
    /// can't touch anything else. The rules work the same whether or not it ever speaks.
    pub fn say(&mut self, text: String, proof: Option<String>) {
        require!(
            Some(env::predecessor_account_id()) == self.rules.voice_account,
            "GASBAG[VOICE] only the voice account speaks"
        );
        require!(!text.is_empty() && text.len() <= MAX_SPEECH_BYTES, "GASBAG[VOICE] speech must be 1 to 280 bytes");
        require!(
            proof.as_ref().is_none_or(|p| p.len() <= MAX_PROOF_BYTES),
            "GASBAG[VOICE] proof is too long"
        );
        let speech = Speech { block: env::block_height(), text, proof };
        self.voice.insert(self.voice_total % VOICE_CAP, speech);
        self.voice_total += 1;
    }

    /// Pays the voice its accumulated share so it can buy inference. Only that share can leave
    /// this way. It can never reach the estate.
    pub fn draw_voice_budget(&mut self) -> U128 {
        let voice = self.rules.voice_account.clone();
        require!(Some(env::predecessor_account_id()) == voice, "GASBAG[VOICE] only the voice account draws this");
        let amount = std::mem::take(&mut self.voice_budget);
        require!(amount > 0, "GASBAG[VOICE] the budget is empty");
        if let Some(account) = voice {
            Promise::new(account).transfer(NearToken::from_yoctonear(amount)).detach();
        }
        U128(amount)
    }

    // ---- Views ----------------------------------------------------------------------------------

    /// Every parameter, as fixed at launch.
    pub fn rules(&self) -> Rules {
        self.rules.clone()
    }

    /// The name, symbol and the prompt behind the voice.
    pub fn identity(&self) -> Identity {
        self.identity.get().clone()
    }

    /// Everything the monitor shows, computed for the current block. A death shows here at once,
    /// even before anyone records it.
    pub fn vitals(&self) -> Vitals {
        let now = env::block_height();
        let cause = self.cause_of_death(now);
        let lungs = self.lungs.len();
        let status = match (cause.is_some(), lungs) {
            (true, _) => "dead",
            (false, 0) => "apnea",
            _ => "breathing",
        };
        let streak = if cause.is_none() && lungs > 0 { now - self.streak_start_block } else { 0 };
        let best = self.best_streak_blocks.max(streak);
        let supply = self.token.total_supply;
        let estate_left = if self.death.is_some() { self.estate_left } else { self.estate };
        let bounty = if cause.is_none() && lungs == 0 {
            let missed = self.wages_between(self.accrued_block, now);
            self.pot + mul_div(missed, self.rules.bounty_bps as u128, BPS)
        } else if cause.is_none() {
            self.pot
        } else {
            0
        };
        Vitals {
            status: status.to_string(),
            cause,
            block: U64(now),
            born_block: U64(self.born_block),
            age_blocks: U64(now - self.born_block),
            old_age_block: U64(self.old_age_block),
            last_breath_block: U64(self.last_breath_block),
            deadline_block: U64(self.deadline_block),
            grace_blocks: U64(self.grace_at(now)),
            breath_tgas: U64(Gas::from_gas(self.breath_gas(now)).as_tgas()),
            lungs,
            last_lung: self.last_lung.clone(),
            breaths: U64(self.breaths),
            streak_blocks: U64(streak),
            best_streak_blocks: U64(best),
            supply: U128(supply),
            bought: U128(self.bought),
            wages: U128(self.wages),
            burned: U128(self.burned),
            lost: U128(self.lost),
            rescue_bounty: U128(bounty),
            estate: U128(self.estate),
            estate_left: U128(estate_left),
            price: U128(self.curve().price(self.bought)),
            floor: U128(mul_div(estate_left, ONE_TOKEN, supply)),
            voice_budget: U128(self.voice_budget),
            gas_burned: U128(self.gas_burned),
            holders: U64(self.holders),
        }
    }

    pub fn death(&self) -> Option<Death> {
        self.death.clone()
    }

    pub fn lung_of(&self, account_id: AccountId) -> Option<Lung> {
        self.lungs.get(&account_id).cloned()
    }

    /// Who is keeping it alive right now.
    pub fn lungs(&self, from: Option<u32>, limit: Option<u32>) -> Vec<(AccountId, Lung)> {
        self.lungs
            .iter()
            .skip(from.unwrap_or(0) as usize)
            .take(limit.unwrap_or(50).min(100) as usize)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Newest first. Covers the latest entries. Every mint and burn is also a NEP-297 event.
    pub fn log(&self, limit: Option<u32>) -> Vec<LogEntry> {
        let want = (limit.unwrap_or(50) as u64).min(LOG_CAP);
        let newest = self.log_total;
        (0..want.min(newest))
            .filter_map(|i| self.log.get(&((newest - 1 - i) % LOG_CAP)).cloned())
            .collect()
    }

    pub fn days(&self, from: Option<u32>, limit: Option<u32>) -> Vec<(u64, Day)> {
        let start = from.unwrap_or(0) as u64;
        let last = self.day_index(env::block_height());
        (start..=last)
            .take(limit.unwrap_or(100).min(200) as usize)
            .filter_map(|d| self.days.get(&d).cloned().map(|row| (d, row)))
            .collect()
    }

    pub fn speech(&self, limit: Option<u32>) -> Vec<Speech> {
        let want = (limit.unwrap_or(10) as u64).min(VOICE_CAP);
        let newest = self.voice_total;
        (0..want.min(newest))
            .filter_map(|i| self.voice.get(&((newest - 1 - i) % VOICE_CAP)).cloned())
            .collect()
    }

    /// NEAR this account would receive for claiming now, or would be owed at today's numbers.
    pub fn payout_of(&self, account_id: AccountId) -> U128 {
        let left = if self.death.is_some() { self.estate_left } else { self.estate };
        U128(mul_div(left, self.token.ft_balance_of(account_id).0, self.token.total_supply))
    }
}

// ---- Internals --------------------------------------------------------------------------------

impl Gasbag {
    fn curve(&self) -> Curve {
        Curve { base_price: self.rules.base_price.0, step: self.rules.curve_step.0 }
    }

    fn age(&self, now: u64) -> u64 {
        now.saturating_sub(self.born_block)
    }

    fn breath_gas(&self, now: u64) -> u64 {
        doubled(
            Gas::from_tgas(self.rules.breath_tgas).as_gas() as u128,
            self.age(now),
            self.rules.doubling_blocks,
        )
        .min(u64::MAX as u128) as u64
    }

    fn grace_at(&self, now: u64) -> u64 {
        halved(self.rules.grace_blocks, self.age(now), self.rules.doubling_blocks)
            .max(self.rules.min_grace_blocks)
    }

    fn day_index(&self, now: u64) -> u64 {
        self.age(now) / self.rules.blocks_per_day
    }

    fn cause_of_death(&self, now: u64) -> Option<Cause> {
        if let Some(death) = &self.death {
            return Some(death.cause);
        }
        // Whichever limit came first is the one that killed it.
        let flatlined = now > self.deadline_block;
        let aged = now >= self.old_age_block;
        match (flatlined, aged) {
            (true, true) if self.deadline_block < self.old_age_block => Some(Cause::Flatline),
            (_, true) => Some(Cause::OldAge),
            (true, false) => Some(Cause::Flatline),
            (false, false) => None,
        }
    }

    fn require_alive(&self, now: u64) {
        if self.cause_of_death(now).is_some() {
            env::panic_str("GASBAG[DEAD] it has died, and the only thing left is the will");
        }
    }

    fn pronounce_at(&mut self, now: u64) {
        if self.death.is_some() {
            return;
        }
        let Some(cause) = self.cause_of_death(now) else { return };
        let block = match cause {
            Cause::Flatline => self.deadline_block + 1,
            Cause::OldAge => self.old_age_block,
        };
        if !self.lungs.is_empty() {
            self.best_streak_blocks = self.best_streak_blocks.max(block.saturating_sub(self.streak_start_block));
        }
        self.estate_left = self.estate;
        let supply = U128(self.token.total_supply);
        let estate = U128(self.estate);
        self.death = Some(Death {
            block,
            cause,
            last_lung: self.last_lung.clone(),
            supply,
            estate,
            breaths: self.breaths,
            best_streak_blocks: self.best_streak_blocks,
        });
        self.note(Kind::Death, self.last_lung.clone(), self.estate);
        Event::Death { cause, estate, supply }.emit();
    }

    fn wages_between(&self, from: u64, to: u64) -> u128 {
        let blocks = to.saturating_sub(from) as u128;
        mul_div(
            self.token.total_supply,
            self.rules.wage_bps_per_day as u128 * blocks,
            BPS * self.rules.blocks_per_day as u128,
        )
    }

    /// Brings wages up to `now`. Call it before anything that changes supply or the lung count.
    fn accrue(&mut self, now: u64) {
        if now <= self.accrued_block {
            return;
        }
        let wages = self.wages_between(self.accrued_block, now);
        self.accrued_block = now;
        let lungs = self.lungs.len() as u128;
        if lungs > 0 {
            self.wage_index += wages / lungs;
        } else {
            let bounty = mul_div(wages, self.rules.bounty_bps as u128, BPS);
            self.pot += bounty;
            self.lost += wages - bounty;
        }
    }

    fn touch_deadline(&mut self, now: u64) {
        self.last_breath_block = now;
        self.deadline_block = now + self.grace_at(now);
    }

    fn remove_lung(&mut self, owner: &AccountId, now: u64) {
        self.lungs.remove(owner);
        if self.lungs.is_empty() {
            let end = now.min(self.death.as_ref().map_or(now, |d| d.block));
            self.best_streak_blocks = self.best_streak_blocks.max(end.saturating_sub(self.streak_start_block));
        }
    }

    fn breath_args(owner: &AccountId, id: u64) -> Vec<u8> {
        serde_json::json!({ "owner": owner, "id": id }).to_string().into_bytes()
    }

    fn note(&mut self, kind: Kind, who: Option<AccountId>, amount: u128) {
        let n = self.log_total;
        let entry = LogEntry { n, block: env::block_height(), kind, who, amount: U128(amount) };
        self.log.insert(n % LOG_CAP, entry);
        self.log_total += 1;
    }

    fn update_day(&mut self, day: u64, change: impl FnOnce(&mut Day, u128)) {
        let supply = self.token.total_supply;
        let row = self.days.entry(day).or_default();
        change(row, supply);
    }

    fn is_registered(&self, account: &AccountId) -> bool {
        self.token.accounts.contains_key(account)
    }

    fn register(&mut self, account: &AccountId) {
        self.token.internal_register_account(account);
        self.holders += 1;
    }

    fn storage_fee() -> NearToken {
        env::storage_byte_cost().saturating_mul(STORAGE_BYTES_PER_ACCOUNT as u128)
    }
}

// ---- NEP-141 ----------------------------------------------------------------------------------

#[near]
impl FungibleTokenCore for Gasbag {
    #[payable]
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>) {
        self.token.ft_transfer(receiver_id, amount, memo)
    }

    #[payable]
    fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128> {
        self.token.ft_transfer_call(receiver_id, amount, memo, msg)
    }

    fn ft_total_supply(&self) -> U128 {
        self.token.ft_total_supply()
    }

    fn ft_balance_of(&self, account_id: AccountId) -> U128 {
        self.token.ft_balance_of(account_id)
    }
}

#[near]
impl FungibleTokenResolver for Gasbag {
    #[private]
    fn ft_resolve_transfer(&mut self, sender_id: AccountId, receiver_id: AccountId, amount: U128) -> U128 {
        let (used, _burned) = self.token.internal_ft_resolve_transfer(&sender_id, receiver_id, amount);
        used.into()
    }
}

// ---- NEP-145 ----------------------------------------------------------------------------------
//
// One flat fee, which also pays for the account's lung row. That way `ignite` needs no deposit
// and a browser session key can start a lung. Accounts can't unregister, because a running lung
// must always be able to mint to its owner.

#[near]
impl StorageManagement for Gasbag {
    #[payable]
    fn storage_deposit(&mut self, account_id: Option<AccountId>, registration_only: Option<bool>) -> StorageBalance {
        let _ = registration_only;
        let payer = env::predecessor_account_id();
        let account = account_id.unwrap_or_else(|| payer.clone());
        let attached = env::attached_deposit();
        let fee = Self::storage_fee();
        let refund = if self.is_registered(&account) {
            attached
        } else {
            require!(attached >= fee, "GASBAG[STORAGE] the registration fee was not attached");
            self.register(&account);
            attached.saturating_sub(fee)
        };
        if !refund.is_zero() {
            Promise::new(payer).transfer(refund).detach();
        }
        StorageBalance { total: fee, available: NearToken::from_yoctonear(0) }
    }

    #[payable]
    fn storage_withdraw(&mut self, amount: Option<NearToken>) -> StorageBalance {
        assert_one_yocto();
        require!(amount.is_none_or(|a| a.is_zero()), "GASBAG[STORAGE] the registration fee can't be withdrawn");
        StorageBalance { total: Self::storage_fee(), available: NearToken::from_yoctonear(0) }
    }

    #[payable]
    fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        assert_one_yocto();
        let _ = force;
        false
    }

    fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        let fee = Self::storage_fee();
        StorageBalanceBounds { min: fee, max: Some(fee) }
    }

    fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        self.is_registered(&account_id)
            .then(|| StorageBalance { total: Self::storage_fee(), available: NearToken::from_yoctonear(0) })
    }
}

// ---- NEP-148 ----------------------------------------------------------------------------------

#[near]
impl FungibleTokenMetadataProvider for Gasbag {
    /// The icon follows the coin's state, so a wallet that reads it live sees it die.
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        let now = env::block_height();
        let icon = if self.cause_of_death(now).is_some() {
            metadata::ICON_DEAD
        } else if self.lungs.is_empty() {
            metadata::ICON_GASPING
        } else {
            metadata::ICON_BREATHING
        };
        let id = self.identity.get();
        FungibleTokenMetadata {
            spec: FT_METADATA_SPEC.to_string(),
            name: id.name.clone(),
            symbol: id.symbol.clone(),
            icon: Some(icon.to_string()),
            reference: None,
            reference_hash: None,
            decimals: 18,
        }
    }
}
