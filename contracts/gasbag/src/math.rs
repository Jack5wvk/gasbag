//! Pure arithmetic: the bonding curve, ageing and safe scaling. No chain access here, so all of
//! it can be checked directly and mirrored in the simulator and the web page.

use uint::construct_uint;

construct_uint! {
    pub struct U256(4);
}

/// GASBAG has 18 decimals.
pub const ONE_TOKEN: u128 = 1_000_000_000_000_000_000;

/// `a * b / c`, rounded down, with a 256-bit intermediate.
pub fn mul_div(a: u128, b: u128, c: u128) -> u128 {
    if c == 0 {
        return 0;
    }
    let wide = U256::from(a) * U256::from(b) / U256::from(c);
    if wide > U256::from(u128::MAX) { u128::MAX } else { wide.low_u128() }
}

/// Value of `base` doubled every `period`, rising in a straight line between doublings.
pub fn doubled(base: u128, age: u64, period: u64) -> u128 {
    let doublings = (age / period).min(64) as u32;
    let at_start = base << doublings;
    at_start + mul_div(at_start, (age % period) as u128, period as u128)
}

/// Value of `base` halved every `period`, falling in a straight line between halvings.
pub fn halved(base: u64, age: u64, period: u64) -> u64 {
    let halvings = (age / period).min(63) as u32;
    let at_start = (base >> halvings) as u128;
    (at_start - at_start * (age % period) as u128 / (2 * period as u128)) as u64
}

/// The first age at which [`doubled`] reaches `limit`.
pub fn age_when_doubled_reaches(base: u128, limit: u128, period: u64) -> u64 {
    let mut doublings = 0u64;
    let mut at_start = base;
    while at_start * 2 <= limit {
        at_start *= 2;
        doublings += 1;
    }
    // Round up so the value at the returned age is never below `limit`.
    let rem_age = ((limit - at_start) * period as u128).div_ceil(at_start);
    doublings * period + rem_age as u64
}

/// The bonding curve. The price starts at `base_price` yoctoNEAR per token and rises by
/// `base_price` for every `step` whole tokens bought.
#[derive(Clone, Copy)]
pub struct Curve {
    pub base_price: u128,
    pub step: u128,
}

impl Curve {
    fn step_units(&self) -> U256 {
        U256::from(self.step) * U256::from(ONE_TOKEN)
    }

    /// Token units that `near` yoctoNEAR buys when `bought` units have already been sold.
    ///
    /// Spending from `q` to `q + n` costs `base_price * ((S + q + n)^2 - (S + q)^2) / (2 * S)`
    /// per whole token, where `S` is the step in units, so `n` has a closed form.
    pub fn tokens_for(&self, near: u128, bought: u128) -> u128 {
        let s = self.step_units();
        let q = U256::from(bought) + s;
        let inner = q * q + U256::from(2) * s * U256::from(near) * U256::from(ONE_TOKEN) / U256::from(self.base_price);
        (inner.integer_sqrt() - q).low_u128()
    }

    /// yoctoNEAR for the next whole token when `bought` units have been sold.
    pub fn price(&self, bought: u128) -> u128 {
        let s = self.step_units();
        (U256::from(self.base_price) * (U256::from(bought) + s) / s).low_u128()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEAR: u128 = 1_000_000_000_000_000_000_000_000;

    fn launch_curve() -> Curve {
        Curve { base_price: NEAR / 1_000_000, step: 50_000_000 }
    }

    #[test]
    fn mul_div_survives_values_that_overflow_u128() {
        let big = 10u128.pow(30);
        assert_eq!(mul_div(big, big, big), big);
    }

    #[test]
    fn mul_div_rounds_down() {
        assert_eq!(mul_div(10, 3, 4), 7);
    }

    #[test]
    fn mul_div_by_zero_is_zero() {
        assert_eq!(mul_div(10, 3, 0), 0);
    }

    #[test]
    fn doubled_is_exact_at_each_doubling() {
        assert_eq!(doubled(3, 0, 100), 3);
        assert_eq!(doubled(3, 100, 100), 6);
        assert_eq!(doubled(3, 200, 100), 12);
    }

    #[test]
    fn doubled_rises_in_a_line_between_doublings() {
        assert_eq!(doubled(1000, 50, 100), 1500);
        assert_eq!(doubled(1000, 150, 100), 3000);
    }

    #[test]
    fn halved_is_exact_at_each_halving() {
        assert_eq!(halved(3600, 0, 100), 3600);
        assert_eq!(halved(3600, 100, 100), 1800);
        assert_eq!(halved(3600, 200, 100), 900);
    }

    #[test]
    fn halved_falls_in_a_line_between_halvings() {
        assert_eq!(halved(1000, 50, 100), 750);
    }

    #[test]
    fn halved_cant_overflow_with_the_longest_allowed_schedules() {
        let year = 52_560_000;
        assert_eq!(halved(year, year / 2, year), year - year / 4);
    }

    #[test]
    fn halved_never_rises_with_age() {
        let mut last = u64::MAX;
        for age in (0..1000).step_by(7) {
            let now = halved(36_000, age, 100);
            assert!(now <= last);
            last = now;
        }
    }

    #[test]
    fn age_limit_is_the_first_age_at_the_limit() {
        for (base, limit, period) in [(3u128, 990u128, 1_008_000u64), (30, 990, 600), (7, 700, 13)] {
            let age = age_when_doubled_reaches(base * 1_000_000_000_000, limit * 1_000_000_000_000, period);
            assert!(doubled(base * 1_000_000_000_000, age, period) >= limit * 1_000_000_000_000);
            assert!(doubled(base * 1_000_000_000_000, age - 1, period) < limit * 1_000_000_000_000);
        }
    }

    #[test]
    fn launch_limit_lands_around_day_53() {
        let age = age_when_doubled_reaches(5_000_000_000_000, 990_000_000_000_000, 1_008_000);
        let days = age as f64 / 144_000.0;
        assert!((52.7..53.0).contains(&days), "{days}");
    }

    #[test]
    fn the_first_near_buys_about_990_thousand_tokens() {
        // A flat price would give 1,000,000. The curve rises 2% across that first purchase.
        let got = launch_curve().tokens_for(NEAR, 0) / ONE_TOKEN;
        assert!((990_000..991_000).contains(&got), "{got}");
    }

    #[test]
    fn splitting_a_buy_costs_nothing_extra() {
        let c = launch_curve();
        let once = c.tokens_for(100 * NEAR, 0);
        let first = c.tokens_for(40 * NEAR, 0);
        let second = c.tokens_for(60 * NEAR, first);
        assert!(first + second <= once);
        assert!(once - (first + second) < ONE_TOKEN);
    }

    #[test]
    fn price_is_the_base_at_zero_and_doubles_after_one_step() {
        let c = launch_curve();
        assert_eq!(c.price(0), c.base_price);
        assert_eq!(c.price(50_000_000 * ONE_TOKEN), 2 * c.base_price);
    }

    #[test]
    fn floor_is_at_least_half_the_last_price() {
        // The property the page promises: at any point, estate / bought >= price / 2.
        let c = launch_curve();
        for near in [1u128, 10, 100, 1_000, 10_000, 100_000] {
            let spent = near * NEAR;
            let bought = c.tokens_for(spent, 0);
            let floor = mul_div(spent, ONE_TOKEN, bought);
            assert!(floor * 2 >= c.price(bought), "at {near} NEAR");
        }
    }

    #[test]
    fn no_arithmetic_overflow_at_a_huge_estate() {
        let c = launch_curve();
        let bought = c.tokens_for(1_000_000_000 * NEAR, 0);
        assert!(bought > 0);
        assert!(c.price(bought) > c.base_price);
    }
}
