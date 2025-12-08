use crate::errors::CbmmError;
use crate::helpers::{SCALING_FACTOR, X10K_100_PERCENT_BP};
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default, InitSpace)]
pub struct BurnRateLimiter {
    /// Executed burns ("heat") that decay over time.
    pub accumulated_stress_bp_x10k: u64,
    /// Pending burns in the queue; does not decay.
    pub pending_queue_shares_bp_x10k: u64,
    pub last_update_ts: i64,
}

#[derive(PartialEq, Debug)]
pub enum RateLimitResult {
    /// Queue was empty/flushed fully. Burn this amount.
    ExecuteFull(u64),
    /// Queue was large; only part of it was executed.
    ExecutePartial(u64),
    /// Nothing executed (system hot or burn too small).
    Queued,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace, Default)]
pub struct BurnRateConfig {
    pub limit_bp_x100: u64,
    pub min_burn_bp_x100: u64,
    pub decay_rate_per_sec_bp_x100: u64,
}

impl BurnRateConfig {
    pub fn new(limit_bp_x100: u64, min_burn_bp_x100: u64, decay_rate_per_sec_bp_x100: u64) -> Self {
        Self {
            limit_bp_x100,
            min_burn_bp_x100,
            decay_rate_per_sec_bp_x100,
        }
    }
}

impl BurnRateLimiter {
    pub fn new(now: i64) -> Self {
        Self {
            accumulated_stress_bp_x10k: 0,
            pending_queue_shares_bp_x10k: 0,
            last_update_ts: now,
        }
    }

    /// Geometric add: result = 1 - (1 - a) * (1 - b), in x10k basis points.
    /// Used for combining queue or stress in a share-based way.
    fn compound_add(current_x10k: u64, new_x10k: u64) -> Result<u64> {
        let p = X10K_100_PERCENT_BP;

        let keep_cur = p.checked_sub(current_x10k).ok_or(CbmmError::MathOverflow)?;
        let keep_new = p.checked_sub(new_x10k).ok_or(CbmmError::MathOverflow)?;

        let numerator = (keep_cur as u128)
            .checked_mul(keep_new as u128)
            .ok_or(CbmmError::MathOverflow)?;

        // Round up the "kept" part; the final result is p - keep_combined and needs to be rounded down.
        let keep_combined = numerator
            .checked_add((p - 1) as u128)
            .ok_or(CbmmError::MathOverflow)?
            .checked_div(p as u128)
            .ok_or(CbmmError::MathOverflow)?;

        Ok(p.saturating_sub(keep_combined as u64))
    }

    /// Geometric remove: result = (total - part) / (1 - part), in x10k basis points.
    /// Used to compute the remaining queue after peeling off a burn.
    fn compound_remove(total_x10k: u64, part_to_remove_x10k: u64) -> Result<u64> {
        if part_to_remove_x10k > total_x10k {
            return Err(CbmmError::MathOverflow.into());
        }

        let num = total_x10k
            .checked_sub(part_to_remove_x10k)
            .ok_or(CbmmError::MathOverflow)?;

        let scaled_num = (num as u128)
            .checked_mul(X10K_100_PERCENT_BP as u128)
            .ok_or(CbmmError::MathOverflow)?;

        let denom = X10K_100_PERCENT_BP
            .checked_sub(part_to_remove_x10k)
            .ok_or(CbmmError::MathOverflow)?;

        // Integer division floor is acceptable: worst case we burn slightly less than originally requested.
        let result = scaled_num
            .checked_div(denom as u128)
            .ok_or(CbmmError::MathOverflow)?;

        Ok(result as u64)
    }

    pub fn calculate_required_bp_x100(
        &mut self,
        new_burn_bp_x100: u32, // user input
        config: &BurnRateConfig,
        now: i64,
    ) -> Result<RateLimitResult> {
        // Upscale inputs to x10k basis points.
        let new_burn_x10k = (new_burn_bp_x100 as u64)
            .checked_mul(SCALING_FACTOR)
            .unwrap();
        let soft_limit_x10k = config.limit_bp_x100.checked_mul(SCALING_FACTOR).unwrap();
        let min_burn_x10k = config.min_burn_bp_x100.checked_mul(SCALING_FACTOR).unwrap();
        let decay_rate_x10k = config
            .decay_rate_per_sec_bp_x100
            .checked_mul(SCALING_FACTOR)
            .unwrap();

        // Decay accumulated stress linearly over time.
        let time_delta = (now.saturating_sub(self.last_update_ts)) as u64;
        let decay_amount = time_delta.saturating_mul(decay_rate_x10k);
        self.accumulated_stress_bp_x10k =
            self.accumulated_stress_bp_x10k.saturating_sub(decay_amount);

        // Always enqueue the new request.
        self.pending_queue_shares_bp_x10k =
            Self::compound_add(self.pending_queue_shares_bp_x10k, new_burn_x10k)?;
        self.last_update_ts = now;

        // Remaining linear capacity under the soft limit.
        let available_space_x10k = soft_limit_x10k.saturating_sub(self.accumulated_stress_bp_x10k);
        if available_space_x10k < min_burn_x10k {
            return Ok(RateLimitResult::Queued);
        }

        // Burn the smaller of total queued shares vs remaining capacity.
        let potential_burn_x10k = self.pending_queue_shares_bp_x10k.min(available_space_x10k);
        if potential_burn_x10k < min_burn_x10k {
            return Ok(RateLimitResult::Queued);
        }

        // Add to stress (linear).
        self.accumulated_stress_bp_x10k = self
            .accumulated_stress_bp_x10k
            .checked_add(potential_burn_x10k)
            .ok_or(CbmmError::MathOverflow)?;

        // Remove from queue (geometric).
        self.pending_queue_shares_bp_x10k =
            Self::compound_remove(self.pending_queue_shares_bp_x10k, potential_burn_x10k)?;

        // Downscale back to x100 basis points (floor: may burn slightly less than allowed).
        let burn_output_x100 = potential_burn_x10k / SCALING_FACTOR;

        if self.pending_queue_shares_bp_x10k == 0 {
            Ok(RateLimitResult::ExecuteFull(burn_output_x100))
        } else {
            Ok(RateLimitResult::ExecutePartial(burn_output_x100))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    // Constants chosen to exercise the limiter around a 5% soft limit.
    const SOFT_LIMIT: u64 = 50000; // 5% (500 bps x100)
    const MIN_BURN: u64 = 1000; // 0.1% (10 bps x100)
    const BURN_BP_X100: u32 = 10000; // 1% (100 bps x100)
    const DECAY_RATE_PER_SEC: u64 = 100; // 1 bps per second decay
    const START_TIME: i64 = 0;

    #[test_case(
        // pre: accumulated, pending, last_update_ts
        4_800_000, 0, START_TIME,
        // burn input
        BURN_BP_X100,
        // call timestamp
        START_TIME,
        // expected post state: accumulated, pending, last_update_ts
        5_000_000, 801_603, START_TIME,
        // expected result
        RateLimitResult::ExecutePartial(2000);
        "partial_fill_near_soft_limit"
    )]
    #[test_case(
        // pre state: after scenario 1
        5_000_000, 801_603, START_TIME,
        BURN_BP_X100,
        5, // 5 seconds later (decayed stress is 4.95%)
        4_950_000, 1_793_586, 5,
        RateLimitResult::Queued;
        "dust_rejection"
    )]
    #[test_case(
        // pre state: after scenario 2
        4_950_000, 1_793_586, 5,
        BURN_BP_X100,
        100, // 95 seconds later (decayed stress is 4%)
        5_000_000, 1_793_585, 100,
        RateLimitResult::ExecutePartial(10_000);
        "partial_flush"
    )]
    #[test_case(
        // pre state: after scenario 3
        5_000_000, 1_793_585, 100,
        BURN_BP_X100,
        10000, // anything over 600 behaves the same here
        2_775_649, 0, 10000,
        RateLimitResult::ExecuteFull(27_756);
        "full_flush_after_long_cooldown"
    )]
    fn test_try_burn_and_flush_scenarios(
        // State before
        pre_accumulated_stress_bp_x10k: u64,
        pre_pending_queue_shares_bp_x10k: u64,
        pre_last_update_ts: i64,
        // Burn input
        new_burn_bp_x100: u32,
        // Timestamp of this call
        now: i64,
        // Expected state after
        expected_accumulated_stress_bp_x10k: u64,
        expected_pending_queue_shares_bp_x10k: u64,
        expected_last_update_ts: i64,
        // Expected function output
        expected_result: RateLimitResult,
    ) {
        let mut limiter = BurnRateLimiter {
            accumulated_stress_bp_x10k: pre_accumulated_stress_bp_x10k,
            pending_queue_shares_bp_x10k: pre_pending_queue_shares_bp_x10k,
            last_update_ts: pre_last_update_ts,
        };

        let config = BurnRateConfig::new(SOFT_LIMIT, MIN_BURN, DECAY_RATE_PER_SEC);

        let res = limiter
            .calculate_required_bp_x100(new_burn_bp_x100, &config, now)
            .unwrap();

        assert_eq!(res, expected_result, "unexpected RateLimitResult");
        assert_eq!(
            limiter.accumulated_stress_bp_x10k, expected_accumulated_stress_bp_x10k,
            "unexpected accumulated_stress_bp_x10k"
        );
        assert_eq!(
            limiter.pending_queue_shares_bp_x10k, expected_pending_queue_shares_bp_x10k,
            "unexpected pending_queue_shares_bp_x10k"
        );
        assert_eq!(
            limiter.last_update_ts, expected_last_update_ts,
            "unexpected last_update_ts"
        );
    }
}
