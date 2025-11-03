import type { SellEvent } from '@cbmm/js-client';
import { getRedisClient } from './client';

export async function recordSellEvent(
  event: SellEvent,
  timestamp: number,
): Promise<void> {
  const redis = getRedisClient();
  
  // Convert bigint values to strings for JSON serialization
  const eventData = {
    bInput: event.bInput.toString(),
    aOutput: event.aOutput.toString(),
    creatorFees: event.creatorFees.toString(),
    buybackFees: event.buybackFees.toString(),
    platformFees: event.platformFees.toString(),
    topupPaid: event.topupPaid.toString(),
    newBReserve: event.newBReserve.toString(),
    newAReserve: event.newAReserve.toString(),
    newOutstandingTopup: event.newOutstandingTopup.toString(),
    newCreatorFeesBalance: event.newCreatorFeesBalance.toString(),
    newBuybackFeesBalance: event.newBuybackFeesBalance.toString(),
    timestamp,
    poolAddress: event.pool.toString(),
    userAddress: event.seller.toString(),
    eventType: 'sell' as const,
  };

  const poolAddress = event.pool.toString()
  const userAddress = event.seller.toString()
  const key = `cbmm-demo:events:sell:${poolAddress}:${timestamp}`;
  const jsonValue = JSON.stringify(eventData);

  // Store as JSON string
  await redis.set(key, jsonValue);

  // Add to time-series index for querying by timestamp
  // Using TS.ADD for RedisTimeSeries module
  const tsKey = `cbmm-demo:ts:events:sell:${poolAddress}`;
  await redis.call('TS.ADD', tsKey, timestamp, 1, 'LABELS', 'pool', poolAddress, 'user', userAddress, 'type', 'sell');

  // Also store in sorted set for range queries
  await redis.zadd(`cbmm-demo:events:sell:${poolAddress}`, timestamp, key);
}

