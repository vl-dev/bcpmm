import type { BurnEvent } from '@cbmm/js-client';
import { getRedisClient } from './client';

export async function recordBurnEvent(
  event: BurnEvent,
  timestamp: number,
): Promise<void> {
  const redis = getRedisClient();
  
  // Convert bigint values to strings for JSON serialization
  const eventData = {
    burnAmount: event.burnAmount.toString(),
    topupAccrued: event.topupAccrued.toString(),
    newBReserve: event.newBReserve.toString(),
    newAReserve: event.newAReserve.toString(),
    newOutstandingTopup: event.newOutstandingTopup.toString(),
    newVirtualReserve: event.newVirtualReserve.toString(),
    newBuybackFeesBalance: event.newBuybackFeesBalance.toString(),
    timestamp,
    poolAddress: event.pool.toString(),
    userAddress: event.burner.toString(),
    eventType: 'burn' as const,
  };

  const poolAddress = event.pool.toString()
  const userAddress = event.burner.toString()
  const key = `cbmm-demo:events:burn:${poolAddress}:${timestamp}`;
  const jsonValue = JSON.stringify(eventData);

  // Store as JSON string
  await redis.set(key, jsonValue);

  // Add to time-series index for querying by timestamp
  // Using TS.ADD for RedisTimeSeries module
  const tsKey = `cbmm-demo:ts:events:burn:${poolAddress}`;
  await redis.call('TS.ADD', tsKey, timestamp, 1, 'LABELS', 'pool', poolAddress, 'user', userAddress, 'type', 'burn');

  // Also store in sorted set for range queries
  await redis.zadd(`cbmm-demo:events:burn:${poolAddress}`, timestamp, key);
}

