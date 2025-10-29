import { type BcpmmPool } from "@bcpmm/js-client";

/**
 * Calculate fees for a given A amount
 */
function calculateFees(
  aAmount: bigint,
  creatorFeeBasisPoints: number,
  buybackFeeBasisPoints: number
): { creatorFeesAmount: bigint; buybackFeesAmount: bigint } {
  // Use ceiling division for fees: ceil(x / d) = (x + d - 1) / d
  const creatorFeesAmount =
    (aAmount * BigInt(creatorFeeBasisPoints) + 9999n) / 10000n;
  const buybackFeesAmount =
    (aAmount * BigInt(buybackFeeBasisPoints) + 9999n) / 10000n;
  return { creatorFeesAmount, buybackFeesAmount };
}


/**
 * Calculate the price of 1 B token in terms of A tokens
 * Returns the price as a number (A tokens per B token)
 * 
 * Formula:
 * 1. For desired output_b: swap_amount = output_b * (a_reserve + a_virtual_reserve) / (b_reserve - output_b)
 * 2. a_amount needed = swap_amount / (1 - fee_rate)
 * 
 * Where fee_rate accounts for both creator and buyback fees
 */
export function calculateVirtualTokenPrice(
  pool: BcpmmPool,
  bTokenDecimals: number = 6
): number {
  const oneBToken = BigInt(10 ** bTokenDecimals);
  
  // Handle edge cases
  if (pool.bReserve === 0n || pool.aReserve + pool.aVirtualReserve === 0n || pool.bReserve <= oneBToken) {
    return 0;
  }
  
  // Step 1: Calculate swap_amount needed to get 1 B token
  // From: output_b = (b_reserve * swap_amount) / (a_reserve + a_virtual_reserve + swap_amount)
  // Solving for swap_amount: swap_amount = output_b * (a_reserve + a_virtual_reserve) / (b_reserve - output_b)
  const numerator = oneBToken * (pool.aReserve + pool.aVirtualReserve);
  const denominator = pool.bReserve - oneBToken;
  
  if (denominator <= 0n) {
    return 0;
  }
  
  const swapAmount = numerator / denominator;
  
  // Step 2: Calculate a_amount needed (accounting for fees)
  // swap_amount = a_amount - creator_fees - buyback_fees
  // Fees use ceiling division: ceil(a_amount * fee_bp / 10000)
  // 
  // Approximate: swap_amount = a_amount * (1 - total_fee_rate)
  // More accurate: iterate to find exact a_amount
  const totalFeeRate = (pool.creatorFeeBasisPoints + pool.buybackFeeBasisPoints) / 10000;
  
  // Initial estimate
  let aAmount = (swapAmount * 10000n) / BigInt(Math.floor((1 - totalFeeRate) * 10000));
  
  // Refine: adjust for ceiling division in fees
  for (let i = 0; i < 10; i++) {
    const fees = calculateFees(
      aAmount,
      pool.creatorFeeBasisPoints,
      pool.buybackFeeBasisPoints
    );
    
    const calculatedSwapAmount = aAmount - fees.creatorFeesAmount - fees.buybackFeesAmount;
    
    if (calculatedSwapAmount === swapAmount) {
      break;
    }
    
    // Adjust aAmount proportionally
    if (calculatedSwapAmount > 0n) {
      aAmount = (aAmount * swapAmount) / calculatedSwapAmount;
    } else {
      aAmount = swapAmount + fees.creatorFeesAmount + fees.buybackFeesAmount;
    }
  }
  
  // Convert to human-readable: A tokens (with 6 decimals) per 1 B token
  return Number(aAmount) / 1_000_000;
}

/**
 * Calculate the amount of B tokens received when buying with a given A amount
 * Returns the output in human-readable format (number of B tokens)
 */
export function calculateBuyOutput(
  aAmount: bigint,
  pool: BcpmmPool,
  bTokenDecimals: number = 6
): number {
  if (aAmount === 0n) {
    return 0;
  }

  const fees = calculateFees(
    aAmount,
    pool.creatorFeeBasisPoints,
    pool.buybackFeeBasisPoints
  );

  const swapAmount = aAmount - fees.creatorFeesAmount - fees.buybackFeesAmount;

  if (swapAmount <= 0n) {
    return 0;
  }

  // Calculate output: output_b = (b_reserve * swap_amount) / (a_reserve + a_virtual_reserve + swap_amount)
  const numerator = pool.bReserve * swapAmount;
  const denominator = pool.aReserve + pool.aVirtualReserve + swapAmount;
  const outputB = numerator / denominator;

  // Convert to human-readable: divide by 10^decimals
  return Number(outputB) / Number(BigInt(10 ** bTokenDecimals));
}

