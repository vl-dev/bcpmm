use crate::errors::BcpmmError;
use anchor_lang::prelude::*;

// ==========================================
// CONSTANTS & SCALING
// ==========================================

pub const X10K_100_PERCENT_BP: u64 = 100_000_000;
pub const X100_100_PERCENT_BP: u64 = 1_000_000;
pub const SCALING_FACTOR: u64 = X10K_100_PERCENT_BP / X100_100_PERCENT_BP;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct CompoundingRateLimiter {
    // "The Bucket": Tracks executed burns (Heat). Decays over time.
    pub accumulated_stress_bp_x10k: u64,

    // "The Tank": Tracks waiting requests. Does NOT decay.
    pub pending_queue_shares_bp_x10k: u64,

    pub last_update_ts: i64,
}

#[derive(PartialEq, Debug)]
pub enum RateLimitResult {
    /// Queue was empty/flushed fully. Burn this amount.
    ExecuteFull(u64),
    /// Queue was huge. We peeled off this specific amount to burn.
    ExecutePartial(u64),
    /// System hot or burn too small. Burn 0.
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

    // ==========================================
    // MATH HELPERS
    // ==========================================

    /// Geometric Add: Result = 1 - (1 - A) * (1 - B)
    /// Used to add a new request to the queue, or add execution to stress.
    fn compound_add(current_x10k: u64, new_x10k: u64) -> Result<u64> {
        let p = X10K_100_PERCENT_BP;

        let keep_cur = p
            .checked_sub(current_x10k)
            .ok_or(BcpmmError::MathOverflow)?;
        let keep_new = p.checked_sub(new_x10k).ok_or(BcpmmError::MathOverflow)?;

        // 1. Numerator = KeepA * KeepB
        let numerator = (keep_cur as u128)
            .checked_mul(keep_new as u128)
            .ok_or(BcpmmError::MathOverflow)?;

        // round the numerator up as it is subtracted in the final result and we need the result to be rounded down
        let keep_combined = numerator
            .checked_add((p - 1) as u128)
            .ok_or(BcpmmError::MathOverflow)?
            .checked_div(p as u128)
            .ok_or(BcpmmError::MathOverflow)?;

        Ok(p.saturating_sub(keep_combined as u64))
    }

    /// Geometric Subtract: Result = (Total - Part) / (1 - Part)
    /// Used to calculate Remaining Queue after a burn, or Available Space in Stress.
    fn compound_remove(total_x10k: u64, part_to_remove_x10k: u64) -> Result<u64> {
        // Validation: Cannot remove more than total
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

        // We  do not worry about the result being rounded down, as we worst case is that we burn less than the requested amount.
        let result = scaled_num
            .checked_div(denom as u128)
            .ok_or(BcpmmError::MathOverflow)?;

        Ok(result as u64)
    }

    // ==========================================
    // CORE LOGIC
    // ==========================================

    pub fn try_burn_and_flush(
        &mut self,
        new_burn_bp_x100: u64, // User Input
        limit_bp_x100: u64,    // e.g. 5%
        min_burn_bp_x100: u64, // e.g. 0.1%
        decay_rate_per_sec_bp_x100: u64,
        now: i64,
    ) -> Result<RateLimitResult> {
        // 1. UPSCALE INPUTS
        let new_burn_x10k = new_burn_bp_x100.checked_mul(SCALING_FACTOR).unwrap();
        let soft_limit_x10k = limit_bp_x100.checked_mul(SCALING_FACTOR).unwrap();
        let min_burn_x10k = min_burn_bp_x100.checked_mul(SCALING_FACTOR).unwrap();
        let decay_rate_x10k = decay_rate_per_sec_bp_x100
            .checked_mul(SCALING_FACTOR)
            .unwrap();

        // 2. DECAY STRESS (Time Travel)
        // Calculate where stress SHOULD be right now.
        let time_delta = (now.saturating_sub(self.last_update_ts)) as u64;
        let decay_amount = time_delta.saturating_mul(decay_rate_x10k);

        // Apply decay - linear decay, geometric decay is more complex and more restrictive
        self.accumulated_stress_bp_x10k =
            self.accumulated_stress_bp_x10k.saturating_sub(decay_amount);

        // 3. ADMISSION (Queue First)
        // We ALWAYS add the new request to the queue.
        self.pending_queue_shares_bp_x10k =
            Self::compound_add(self.pending_queue_shares_bp_x10k, new_burn_x10k)?;
        self.last_update_ts = now;

        // 4. CAPACITY CHECK
        // How much allowance is left to fill the stress? Linear calculation is more restrictive and less error prone than geometric calculation.
        let available_space_x10k = soft_limit_x10k.saturating_sub(self.accumulated_stress_bp_x10k);
        if available_space_x10k < min_burn_x10k {
            return Ok(RateLimitResult::Queued);
        }

        // 5. THE FLUSH DECISION
        // We burn the smaller of: The Total Queue OR The Available Space
        let potential_burn_x10k = self.pending_queue_shares_bp_x10k.min(available_space_x10k);
        if potential_burn_x10k < min_burn_x10k {
            return Ok(RateLimitResult::Queued);
        }

        // 6. EXECUTION (State Update)

        // A. Add to Stress (Fill the bucket) - linear addition as geometric addition is close in the percentages this is designed for.
        self.accumulated_stress_bp_x10k = self
            .accumulated_stress_bp_x10k
            .checked_add(potential_burn_x10k)
            .ok_or(BcpmmError::MathOverflow)?;

        // B. Remove from Queue (Peel the layer) - geometric to honor the scheduled burns.
        self.pending_queue_shares_bp_x10k =
            Self::compound_remove(self.pending_queue_shares_bp_x10k, potential_burn_x10k)?;

        // C. Downscale Output
        // Integer division (floor) is safe for burn amount - the worst case is that we burn less than the amount allowed by the rate limiter
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

    // Setup Constants based on your design
    // 5% Soft Limit (500 bps x100)
    const SOFT_LIMIT: u64 = 50000;
    // 0.1% Min Burn (10 bps x100)
    const MIN_BURN: u64 = 1000;
    // 1% Input (100 bps x100)
    const BURN_INPUT: u64 = 10000;
    // 1 bps per second decay rate
    const DECAY_RATE_PER_SEC: u64 = 100;
    // start time
    const START_TIME: i64 = 0;

    #[test]
    fn test_scenario_step_by_step() {
        let mut limiter = CompoundingRateLimiter::new(START_TIME);

        // ====================================================
        // SCENARIO STEP 1: The Partial Fill
        // "we're at 4.8% stress allowance... 1% burn comes in.
        // We allow 0.2% burn, queue 0.8%, new stress 5%"
        // ====================================================

        // 1. Manually set Stress to 4.8% (Scale x10k)
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
                // Geometric math gives ~21bps (0.21%), closely matching your 0.2% approx
                assert_eq!(amount, 2000, "Should allow 0.2% burn");
            }
            _ => panic!("Step 1 failed: Expected Partial Execution"),
        }

        // Verify New Stress is FULL (5%)
        // 5% * 100 * 10000 = 5,000,000
        assert_eq!(
            limiter.accumulated_stress_bp_x10k, 5_000_000,
            "Stress should be capped at 5%"
        );

        // Verify Queue is 0.801603%
        assert_eq!(
            limiter.pending_queue_shares_bp_x10k, 801_603,
            "Queue should be 0.801603%"
        );

        // ====================================================
        // SCENARIO STEP 2: The Dust Rejection
        // "Stress dropped to 4.95%... We don't allow this one... queue it."
        // ====================================================

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
            RateLimitResult::Queued => assert!(true),
            _ => panic!("Step 2 failed: Expected Queue (Dust rejection)"),
        }

        // Verify Stress is 4.95%
        assert_eq!(limiter.accumulated_stress_bp_x10k, 4_950_000);

        // Verify Queue Increased
        assert_eq!(
            limiter.pending_queue_shares_bp_x10k, 1_793_586,
            "Queue should grow to 1.793586%"
        );

        // ====================================================
        // SCENARIO STEP 3: The Priority Flush
        // "stress 4%... adjust queue... take 1%... allow that burn."
        // ====================================================

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
            1_793_585, // (rounded down)
            "Queue should reduce to 1.793586%"
        );

        // ====================================================
        // SCENARIO STEP 4: The Total Flush
        // "stress 0%... adjust queue... then burn it all"
        // ====================================================

        let res = limiter
            .try_burn_and_flush(
                BURN_INPUT,
                SOFT_LIMIT,
                MIN_BURN,
                DECAY_RATE_PER_SEC,
                10000, // anything over 500 will be treated same as 500
            )
            .unwrap();

        match res {
            // It executes immediate because the queue is fully drained
            RateLimitResult::ExecuteFull(amount) => {
                assert_eq!(amount, 27_756, "Should burn entire queue (2.775649%)");
            }
            _ => panic!("Step 4 failed: Expected Immediate Full Flush"),
        }

        assert_eq!(limiter.accumulated_stress_bp_x10k, 2_775_649);
        assert_eq!(limiter.pending_queue_shares_bp_x10k, 0);
    }
}
