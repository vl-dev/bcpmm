use crate::errors::BcpmmError;
use anchor_lang::prelude::*;

// ==========================================
// CONSTANTS & SCALING
// ==========================================

// INTERNAL PRECISION (10^8)
// 100% = 100,000,000
pub const X10K_100_PERCENT_BP: u64 = 100_000_000;

// EXTERNAL INPUT PRECISION (10^4)
// 100% = 10,000
// 1 bp = 100
pub const X100_100_PERCENT_BP: u64 = 10_000;

// SCALING FACTOR (10^8 / 10^4)
pub const SCALING_FACTOR: u64 = 10_000;

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

        // 2. Ceiling Division: We want to round UP the keep amount (conservative burn)
        // (Num + P - 1) / P
        let adjusted_numerator = numerator
            .checked_add((p - 1) as u128)
            .ok_or(BcpmmError::MathOverflow)?;

        let keep_combined = adjusted_numerator
            .checked_div(p as u128)
            .ok_or(BcpmmError::MathOverflow)?;

        Ok(p.saturating_sub(keep_combined as u64))
    }

    /// Geometric Subtract: Result = (Total - Part) / (1 - Part)
    /// Used to calculate Remaining Queue after a burn, or Available Space in Stress.
    fn compound_remove(total_x10k: u64, part_to_remove_x10k: u64) -> Result<u64> {
        let p = X10K_100_PERCENT_BP;

        // Validation: Cannot remove more than total
        if part_to_remove_x10k >= total_x10k {
            return Ok(0);
        }

        // Formula: R = (Q - B) / (1 - B)

        // 1. Numerator: Q - B
        let num_raw = total_x10k
            .checked_sub(part_to_remove_x10k)
            .ok_or(BcpmmError::MathOverflow)?;

        // 2. Denominator: 1 - B
        let denom = p
            .checked_sub(part_to_remove_x10k)
            .ok_or(BcpmmError::MathOverflow)?;

        // 3. Scale Numerator for Division
        let num_scaled = (num_raw as u128)
            .checked_mul(p as u128)
            .ok_or(BcpmmError::MathOverflow)?;

        // 4. Ceiling Division
        // We want to round UP the result (the remaining queue).
        // Rounding up 'Remaining' is safe (conservative), as we don't accidentally delete debt.
        let adjusted_num = num_scaled
            .checked_add((denom - 1) as u128)
            .ok_or(BcpmmError::MathOverflow)?;

        let result = adjusted_num
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

        // Apply decay
        self.accumulated_stress_bp_x10k =
            self.accumulated_stress_bp_x10k.saturating_sub(decay_amount);

        // 3. ADMISSION (Queue First)
        // We ALWAYS add the new request to the queue.
        self.pending_queue_shares_bp_x10k =
            Self::compound_add(self.pending_queue_shares_bp_x10k, new_burn_x10k)?;

        // Update TS immediately. The state is now valid for `now`.
        self.last_update_ts = now;

        // 4. CAPACITY CHECK
        // How much room is in the bucket?
        // Space = (Limit - Stress) / (1 - Stress)
        let available_space_x10k = if self.accumulated_stress_bp_x10k >= soft_limit_x10k {
            0
        } else {
            Self::compound_remove(soft_limit_x10k, self.accumulated_stress_bp_x10k)?
        };

        // 5. THE FLUSH DECISION
        // We burn the smaller of: The Total Queue OR The Available Space
        let potential_burn_x10k =
            std::cmp::min(self.pending_queue_shares_bp_x10k, available_space_x10k);

        // Dust Check: Is it worth executing?
        if potential_burn_x10k < min_burn_x10k {
            // Logic: Not enough space, or queue too small.
            // Action: Keep everything in queue. Burn nothing.
            return Ok(RateLimitResult::Queued);
        }

        // 6. EXECUTION (State Update)

        // A. Add to Stress (Fill the bucket)
        self.accumulated_stress_bp_x10k =
            Self::compound_add(self.accumulated_stress_bp_x10k, potential_burn_x10k)?;

        // B. Remove from Queue (Peel the layer)
        self.pending_queue_shares_bp_x10k =
            Self::compound_remove(self.pending_queue_shares_bp_x10k, potential_burn_x10k)?;

        // C. Downscale Output
        // Integer division (floor) is safe for execute amount
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

    // We use 0 decay in the function call to "freeze" time
    // and manually set stress levels to match your scenario.
    const NO_DECAY: u64 = 0;

    #[test]
    fn test_scenario_step_by_step() {
        let mut limiter = CompoundingRateLimiter::new(0);

        // ====================================================
        // SCENARIO STEP 1: The Partial Fill
        // "we're at 4.8% stress allowance... 1% burn comes in.
        // We allow 0.2% burn, queue 0.8%, new stress 5%"
        // ====================================================

        // 1. Manually set Stress to 4.8% (Scale x10k)
        // 4.8 * 100 = 480 (x100 scale) -> 4,800,000 (x10k scale)
        limiter.accumulated_stress_bp_x10k = 4_800_000;

        let res = limiter
            .try_burn_and_flush(BURN_INPUT, SOFT_LIMIT, MIN_BURN, NO_DECAY, 0)
            .unwrap();

        // MATH CHECK:
        // Available Space = (0.05 - 0.048) / (1 - 0.048)
        //                 = 0.002 / 0.952 = 0.0021008... (~21 bps)
        // Burn = min(Input 100, Space 21) = 21.

        match res {
            RateLimitResult::ExecutePartial(amount) => {
                // Geometric math gives ~21bps (0.21%), closely matching your 0.2% approx
                assert_eq!(amount, 21, "Should allow ~0.21% burn");
            }
            _ => panic!("Step 1 failed: Expected Partial Execution"),
        }

        // Verify New Stress is FULL (5%)
        // 5% * 100 * 10000 = 5,000,000
        assert_eq!(
            limiter.accumulated_stress_bp_x10k, 5_000_000,
            "Stress should be capped at 5%"
        );

        // Verify Queue is ~0.8%
        // We had 1% input, burned ~0.21%.
        // Queue = (0.01 - 0.0021) / (1 - 0.0021) = 0.00791... (~79 bps)
        // 79 bps x 10,000 = 790,000
        assert_eq!(
            limiter.pending_queue_shares_bp_x10k, 791_597,
            "Queue should be ~0.79%"
        );

        // ====================================================
        // SCENARIO STEP 2: The Dust Rejection
        // "Stress dropped to 4.95%... We don't allow this one... queue it."
        // ====================================================

        // 1. Manually set Stress to 4.95%
        limiter.accumulated_stress_bp_x10k = 4_950_000;

        let res = limiter
            .try_burn_and_flush(BURN_INPUT, SOFT_LIMIT, MIN_BURN, NO_DECAY, 0)
            .unwrap();

        // MATH CHECK:
        // Space = (0.05 - 0.0495) / (1 - 0.0495) = 0.0005 / 0.9505 = ~5 bps.
        // Min Burn = 10 bps (0.1%).
        // 5 < 10 -> REJECT.

        match res {
            RateLimitResult::Queued => assert!(true),
            _ => panic!("Step 2 failed: Expected Queue (Dust rejection)"),
        }

        // Verify Queue Increased
        // Previous Queue (~79) + New Input (100)
        // Geometric add: 1 - (1 - 0.0079)(1 - 0.01) = ~1.78%
        assert_eq!(
            limiter.pending_queue_shares_bp_x10k, 1_783_681,
            "Queue should grow to ~1.78%"
        );

        // Verify Stress didn't change (still 4.95%)
        assert_eq!(limiter.accumulated_stress_bp_x10k, 4_950_000);

        // ====================================================
        // SCENARIO STEP 3: The Priority Flush
        // "stress 4%... adjust queue... take 1%... allow that burn."
        // ====================================================

        // 1. Manually set Stress to 4.0%
        limiter.accumulated_stress_bp_x10k = 4_000_000;

        let res = limiter
            .try_burn_and_flush(BURN_INPUT, SOFT_LIMIT, MIN_BURN, NO_DECAY, 0)
            .unwrap();

        // MATH CHECK:
        // 1. Admission: Queue (~178) + Input (100) = ~276 bps (2.76%)
        //    Your "±2.77%" calculation is confirmed here (2.765%).

        // 2. Space: (0.05 - 0.04) / (1 - 0.04) = 0.01 / 0.96 = 1.0416%
        //    Space is ~104 bps.

        // 3. Decision: min(Queue 276, Space 104) = 104.
        //    This matches your expectation of filling the available space.

        match res {
            RateLimitResult::ExecutePartial(amount) => {
                assert_eq!(amount, 104, "Should fill the 1.04% space");
            }
            _ => panic!("Step 3 failed: Expected Partial Flush"),
        }

        // Verify Stress is Full again (5%)
        assert_eq!(limiter.accumulated_stress_bp_x10k, 5_000_000);

        // Verify Queue reduced
        // Queue was ~276, removed ~104.
        // Result should be roughly 1.74%
        assert_eq!(
            limiter.pending_queue_shares_bp_x10k, 1_741_587,
            "Queue should reduce to ~1.74%"
        );

        // ====================================================
        // SCENARIO STEP 4: The Total Flush
        // "stress 0%... adjust queue... then burn it all"
        // ====================================================

        // 1. Manually set Stress to 0%
        limiter.accumulated_stress_bp_x10k = 0;

        let res = limiter
            .try_burn_and_flush(BURN_INPUT, SOFT_LIMIT, MIN_BURN, NO_DECAY, 0)
            .unwrap();

        // MATH CHECK:
        // 1. Admission: Queue (~174) + Input (100) = ~272 bps (2.72%).
        // 2. Space: (0.05 - 0) / 1 = 5%.
        // 3. Decision: min(Queue 2.72%, Space 5%) = 2.72%.

        match res {
            // It executes immediate because the queue is fully drained
            RateLimitResult::ExecuteFull(amount) => {
                assert_eq!(amount, 272, "Should burn entire queue (2.72%)");
            }
            _ => panic!("Step 4 failed: Expected Immediate Full Flush"),
        }

        // Verify Stress matches the burn (2.72%)
        // 272 bps * 100 * 100 = 2,720,000 (approx due to compounding precision)
        // Actual internal value check:
        assert_eq!(limiter.accumulated_stress_bp_x10k, 2_724_171);

        // Verify Queue is empty
        assert_eq!(limiter.pending_queue_shares_bp_x10k, 0);
    }
}
