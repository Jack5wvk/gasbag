//! Pacemaker: a public, keeper-free scheduler for NEAR.
//!
//! Contracts only run when called, so anything on a schedule usually needs a server or a cron job
//! that somebody has to keep paying for and keep online. The pacemaker replaces that. Register a
//! job with a target contract, a method and an interval, attach gas, and it calls that method on
//! time for as long as the gas lasts. Nothing runs off chain.
//!
//! The target sees the pacemaker as its caller, so it can allow-list this one account instead of
//! a keeper's key. When the gas runs out, anyone can top the job up with `fuel`.

use near_heartbeat as heartbeat;
use near_sdk::json_types::Base64VecU8;
use near_sdk::store::IterableMap;
use near_sdk::{AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise, env, near, require, serde_json};

/// Gas one beat needs for itself, before any call it makes.
const MIN_BEAT: Gas = Gas::from_tgas(3);
const MAX_ARGS_BYTES: usize = 1_024;
const MAX_METHOD_BYTES: usize = 64;

#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct Job {
    pub owner: AccountId,
    pub target: AccountId,
    pub method: String,
    pub args: Base64VecU8,
    /// Call the target every this many beats. One beat is 200 blocks, about two minutes.
    pub every_beats: u32,
    /// Gas given to each call to the target.
    pub call_tgas: u64,
    pub beats: u64,
    pub calls: u64,
    /// The running chain. A beat from any other chain stops without doing anything.
    pub chain: u64,
    pub last_beat_block: u64,
    /// Storage deposit, returned on `cancel`.
    pub deposit: NearToken,
}

#[near]
#[derive(BorshStorageKey)]
enum Key {
    Jobs,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Pacemaker {
    jobs: IterableMap<u64, Job>,
    next_job: u64,
}

#[near]
impl Pacemaker {
    #[init]
    pub fn new() -> Self {
        Self { jobs: IterableMap::new(Key::Jobs), next_job: 1 }
    }

    /// Registers a job and starts its chain with the gas left in this call. Attach enough NEAR to
    /// cover the job's storage; the rest is refunded.
    #[payable]
    pub fn schedule(
        &mut self,
        target: AccountId,
        method: String,
        args: Base64VecU8,
        every_beats: u32,
        call_tgas: u64,
    ) -> u64 {
        require!(every_beats >= 1, "PACEMAKER[EVERY] every_beats must be at least 1");
        require!(
            !method.is_empty() && method.len() <= MAX_METHOD_BYTES,
            "PACEMAKER[METHOD] method must be 1 to 64 bytes"
        );
        require!(args.0.len() <= MAX_ARGS_BYTES, "PACEMAKER[ARGS] args are capped at 1 KB");
        let owner = env::predecessor_account_id();
        let attached = env::attached_deposit();
        let id = self.next_job;
        self.next_job += 1;

        let before = env::storage_usage();
        self.jobs.insert(
            id,
            Job {
                owner: owner.clone(),
                target,
                method,
                args,
                every_beats,
                call_tgas,
                beats: 0,
                calls: 0,
                chain: 1,
                last_beat_block: env::block_height(),
                deposit: NearToken::from_yoctonear(0),
            },
        );
        self.jobs.flush();
        let bytes = env::storage_usage().saturating_sub(before) as u128;
        let cost = env::storage_byte_cost().saturating_mul(bytes);
        require!(attached >= cost, format!("PACEMAKER[STORAGE] attach at least {cost}"));
        if let Some(job) = self.jobs.get_mut(&id) {
            job.deposit = cost;
        }
        let refund = attached.saturating_sub(cost);
        if !refund.is_zero() {
            Promise::new(owner).transfer(refund).detach();
        }
        heartbeat::arm("on_beat", Self::beat_args(id, 1));
        id
    }

    /// Starts a fresh chain for a job, on the gas of this call. Anyone can pay. A chain still
    /// running stops at its next beat and refunds its own signer.
    pub fn fuel(&mut self, id: u64) {
        let Some(job) = self.jobs.get_mut(&id) else {
            env::panic_str("PACEMAKER[JOB] no such job");
        };
        job.chain += 1;
        let chain = job.chain;
        heartbeat::arm("on_beat", Self::beat_args(id, chain));
    }

    /// One beat. Runs by itself when the yield times out. It must not panic.
    #[private]
    pub fn on_beat(&mut self, id: u64, chain: u64) {
        let Some(job) = self.jobs.get_mut(&id) else { return };
        if job.chain != chain {
            return;
        }
        job.beats += 1;
        job.last_beat_block = env::block_height();
        let every = job.every_beats.max(1) as u64;
        let call_gas = Gas::from_tgas(job.call_tgas);
        if job.beats % every == 0 && heartbeat::fuel_left() >= call_gas.saturating_add(MIN_BEAT) {
            job.calls += 1;
            // A failing target only fails this call. The chain carries on regardless.
            Promise::new(job.target.clone())
                .function_call(job.method.clone(), job.args.0.clone(), NearToken::from_yoctonear(0), call_gas)
                .detach();
        }
        heartbeat::arm_if_fuelled("on_beat", Self::beat_args(id, chain), MIN_BEAT);
    }

    /// Ends a job and returns its storage deposit. Only the owner can do this.
    pub fn cancel(&mut self, id: u64) {
        let Some(job) = self.jobs.get(&id).cloned() else {
            env::panic_str("PACEMAKER[JOB] no such job");
        };
        require!(env::predecessor_account_id() == job.owner, "PACEMAKER[OWNER] only the job's owner can cancel");
        self.jobs.remove(&id);
        if !job.deposit.is_zero() {
            Promise::new(job.owner).transfer(job.deposit).detach();
        }
    }

    pub fn job(&self, id: u64) -> Option<Job> {
        self.jobs.get(&id).cloned()
    }

    pub fn jobs(&self, from: Option<u32>, limit: Option<u32>) -> Vec<(u64, Job)> {
        self.jobs
            .iter()
            .skip(from.unwrap_or(0) as usize)
            .take(limit.unwrap_or(50).min(100) as usize)
            .map(|(id, job)| (*id, job.clone()))
            .collect()
    }
}

impl Pacemaker {
    fn beat_args(id: u64, chain: u64) -> Vec<u8> {
        serde_json::json!({ "id": id, "chain": chain }).to_string().into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;

    const FEE: u128 = 1_000_000_000_000_000_000_000_000 / 10;

    fn at(who: &str, deposit: u128, tgas: u64) {
        testing_env!(
            VMContextBuilder::new()
                .current_account_id("pacemaker.testnet".parse().unwrap())
                .predecessor_account_id(who.parse().unwrap())
                .attached_deposit(NearToken::from_yoctonear(deposit))
                .prepaid_gas(Gas::from_tgas(tgas))
                .account_balance(NearToken::from_near(100))
                .build()
        );
    }

    fn with_job(every: u32) -> (Pacemaker, u64) {
        at("owner.testnet", FEE, 300);
        let mut p = Pacemaker::new();
        let id = p.schedule(
            "target.testnet".parse().unwrap(),
            "tick".into(),
            Base64VecU8(b"{}".to_vec()),
            every,
            30,
        );
        (p, id)
    }

    #[test]
    fn scheduling_records_the_job_and_charges_only_its_storage() {
        let (p, id) = with_job(5);
        let job = p.job(id).unwrap();
        assert_eq!(job.every_beats, 5);
        assert_eq!(job.chain, 1);
        assert!(!job.deposit.is_zero());
        assert!(job.deposit.as_yoctonear() < FEE);
    }

    #[test]
    #[should_panic(expected = "PACEMAKER[STORAGE]")]
    fn scheduling_without_storage_fails() {
        at("owner.testnet", 0, 300);
        let mut p = Pacemaker::new();
        p.schedule("target.testnet".parse().unwrap(), "tick".into(), Base64VecU8(vec![]), 1, 30);
    }

    #[test]
    #[should_panic(expected = "PACEMAKER[EVERY]")]
    fn a_zero_interval_is_refused() {
        with_job(0);
    }

    #[test]
    fn it_calls_the_target_on_the_interval_only() {
        let (mut p, id) = with_job(3);
        for _ in 0..7 {
            at("pacemaker.testnet", 0, 500);
            p.on_beat(id, 1);
        }
        let job = p.job(id).unwrap();
        assert_eq!(job.beats, 7);
        assert_eq!(job.calls, 2);
    }

    #[test]
    fn a_beat_too_low_on_gas_skips_the_call() {
        let (mut p, id) = with_job(1);
        at("pacemaker.testnet", 0, 32);
        p.on_beat(id, 1);
        let job = p.job(id).unwrap();
        assert_eq!(job.beats, 1);
        assert_eq!(job.calls, 0);
    }

    #[test]
    fn an_old_chain_stops_once_refuelled() {
        let (mut p, id) = with_job(1);
        at("anyone.testnet", 0, 300);
        p.fuel(id);
        at("pacemaker.testnet", 0, 500);
        p.on_beat(id, 1);
        assert_eq!(p.job(id).unwrap().beats, 0);
        at("pacemaker.testnet", 0, 500);
        p.on_beat(id, 2);
        assert_eq!(p.job(id).unwrap().beats, 1);
    }

    #[test]
    #[should_panic(expected = "PACEMAKER[OWNER]")]
    fn only_the_owner_can_cancel() {
        let (mut p, id) = with_job(1);
        at("stranger.testnet", 0, 30);
        p.cancel(id);
    }

    #[test]
    fn the_owner_can_cancel_and_a_cancelled_job_stops() {
        let (mut p, id) = with_job(1);
        at("owner.testnet", 0, 30);
        p.cancel(id);
        assert!(p.job(id).is_none());
        at("pacemaker.testnet", 0, 500);
        p.on_beat(id, 1);
        assert!(p.jobs(None, None).is_empty());
    }

    #[test]
    #[should_panic(expected = "PACEMAKER[JOB]")]
    fn fuelling_an_unknown_job_fails() {
        let (mut p, _) = with_job(1);
        at("anyone.testnet", 0, 300);
        p.fuel(99);
    }
}
