use crate::errors::BcpmmError;
use anchor_lang::prelude::*;

pub const X10K_100_PERCENT_BP: u64 = 100_000_000;
pub const X100_100_PERCENT_BP: u64 = 1_000_000;
pub const SCALING_FACTOR: u64 = X10K_100_PERCENT_BP / X100_100_PERCENT_BP;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct CompoundingRateLimiter {
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

impl CompoundingRateLimiter {
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

        let keep_cur = p
            .checked_sub(current_x10k)
            .ok_or(BcpmmError::MathOverflow)?;
        let keep_new = p.checked_sub(new_x10k).ok_or(BcpmmError::MathOverflow)?;

        let numerator = (keep_cur as u128)
            .checked_mul(keep_new as u128)
            .ok_or(BcpmmError::MathOverflow)?;

        // Round up the "kept" part; the final result is p - keep_combined and needs to be rounded down.
        let keep_combined = numerator
            .checked_add((p - 1) as u128)
            .ok_or(BcpmmError::MathOverflow)?
            .checked_div(p as u128)
            .ok_or(BcpmmError::MathOverflow)?;

        Ok(p.saturating_sub(keep_combined as u64))
    }

    /// Geometric remove: result = (total - part) / (1 - part), in x10k basis points.
    /// Used to compute the remaining queue after peeling off a burn.
    fn compound_remove(total_x10k: u64, part_to_remove_x10k: u64) -> Result<u64> {
        if part_to_remove_x10k > total_x10k {
            return Err(BcpmmError::MathOverflow.into());
        }

        let num = total_x10k
            .checked_sub(part_to_remove_x10k)
            .ok_or(BcpmmError::MathOverflow)?;

        let scaled_num = (num as u128)
            .checked_mul(X10K_100_PERCENT_BP as u128)
            .ok_or(BcpmmError::MathOverflow)?;

        let denom = X10K_100_PERCENT_BP
            .checked_sub(part_to_remove_x10k)
            .ok_or(BcpmmError::MathOverflow)?;

        // Integer division floor is acceptable: worst case we burn slightly less than originally requested.
        let result = scaled_num
            .checked_div(denom as u128)
            .ok_or(BcpmmError::MathOverflow)?;

        Ok(result as u64)
    }

    pub fn try_burn_and_flush(
        &mut self,
        new_burn_bp_x100: u64, // user input
        limit_bp_x100: u64,    // soft limit, e.g. 5%
        min_burn_bp_x100: u64, // min granular burn, e.g. 0.1%
        decay_rate_per_sec_bp_x100: u64,
        now: i64,
    ) -> Result<RateLimitResult> {
        // Upscale inputs to x10k basis points.
        let new_burn_x10k = new_burn_bp_x100.checked_mul(SCALING_FACTOR).unwrap();
        let soft_limit_x10k = limit_bp_x100.checked_mul(SCALING_FACTOR).unwrap();
        let min_burn_x10k = min_burn_bp_x100.checked_mul(SCALING_FACTOR).unwrap();
        let decay_rate_x10k = decay_rate_per_sec_bp_x100
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
            .ok_or(BcpmmError::MathOverflow)?;

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

    // Constants chosen to exercise the limiter around a 5% soft limit.
    const SOFT_LIMIT: u64 = 50000; // 5% (500 bps x100)
    const MIN_BURN: u64 = 1000; // 0.1% (10 bps x100)
    const BURN_INPUT: u64 = 10000; // 1% (100 bps x100)
    const DECAY_RATE_PER_SEC: u64 = 100; // 1 bps per second decay
    const START_TIME: i64 = 0;

    #[test]
    fn test_scenario_step_by_step() {
        let mut limiter = CompoundingRateLimiter::new(START_TIME);

        // Scenario 1: partial fill near the soft limit.
        limiter.accumulated_stress_bp_x10k = 4_800_000;

        let res = limiter
            .try_burn_and_flush(
                BURN_INPUT,
                SOFT_LIMIT,
                MIN_BURN,
                DECAY_RATE_PER_SEC,
                START_TIME,
            )
            .unwrap();

        match res {
            RateLimitResult::ExecutePartial(amount) => {
                assert_eq!(amount, 2000, "Should allow 0.2% burn");
            }
            _ => panic!("Step 1 failed: Expected Partial Execution"),
        }

        assert_eq!(
            limiter.accumulated_stress_bp_x10k, 5_000_000,
            "Stress should be capped at 5%"
        );

        assert_eq!(
            limiter.pending_queue_shares_bp_x10k, 801_603,
            "Queue should be ~0.801603%"
        );

        // Scenario 2: dust rejection when space < min burn.
        let res = limiter
            .try_burn_and_flush(
                BURN_INPUT,
                SOFT_LIMIT,
                MIN_BURN,
                DECAY_RATE_PER_SEC,
                5, // 5 seconds later (decayed stress is 4.95%)
            )
            .unwrap();

        match res {
            RateLimitResult::Queued => {}
            _ => panic!("Step 2 failed: Expected Queue (Dust rejection)"),
        }

        assert_eq!(limiter.accumulated_stress_bp_x10k, 4_950_000);

        assert_eq!(
            limiter.pending_queue_shares_bp_x10k, 1_793_586,
            "Queue should grow to ~1.793586%"
        );

        // Scenario 3: partial flush when 1% capacity opens up.
        let res = limiter
            .try_burn_and_flush(
                BURN_INPUT,
                SOFT_LIMIT,
                MIN_BURN,
                DECAY_RATE_PER_SEC,
                100, // 100 seconds later (decayed stress is 4%)
            )
            .unwrap();

        match res {
            RateLimitResult::ExecutePartial(amount) => {
                assert_eq!(amount, 10000, "Should fill the 1% space");
            }
            _ => panic!("Step 3 failed: Expected Partial Flush"),
        }

        assert_eq!(limiter.accumulated_stress_bp_x10k, 5_000_000);

        assert_eq!(
            limiter.pending_queue_shares_bp_x10k,
            1_793_585, // rounded down
            "Queue should reduce slightly"
        );

        // Scenario 4: full flush after long cooldown.
        let res = limiter
            .try_burn_and_flush(
                BURN_INPUT,
                SOFT_LIMIT,
                MIN_BURN,
                DECAY_RATE_PER_SEC,
                10000, // anything over 510 behaves the same here
            )
            .unwrap();

        match res {
            RateLimitResult::ExecuteFull(amount) => {
                assert_eq!(amount, 27_756, "Should burn entire queue (~2.775649%)");
            }
            _ => panic!("Step 4 failed: Expected Immediate Full Flush"),
        }

        assert_eq!(limiter.accumulated_stress_bp_x10k, 2_775_649);
        assert_eq!(limiter.pending_queue_shares_bp_x10k, 0);
    }
}
