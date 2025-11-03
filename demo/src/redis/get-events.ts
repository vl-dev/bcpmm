import { getRedisClient } from './client';

export type EventType = 'buy' | 'sell' | 'burn';

export type EventQuery = {
  eventType?: EventType;
  poolAddress?: string;
  userAddress?: string;
  startTimestamp?: number;
  endTimestamp?: number;
  limit?: number;
};

export type StoredEvent = {
  timestamp: number;
  poolAddress: string;
  userAddress: string;
  eventType: EventType;
  [key: string]: unknown;
};

export async function getEvents(query: EventQuery): Promise<StoredEvent[]> {
  const redis = getRedisClient();
  const { eventType, poolAddress, userAddress, startTimestamp, endTimestamp, limit = 100 } = query;

  const events: StoredEvent[] = [];

  // Determine which event types to query
  const typesToQuery: EventType[] = eventType ? [eventType] : ['buy', 'sell', 'burn'];

  for (const type of typesToQuery) {
    let keyPattern: string;

    if (poolAddress) {
      keyPattern = `cbmm-demo:events:${type}:${poolAddress}`;
    } else if (userAddress) {
      keyPattern = `cbmm-demo:events:${type}:user:${userAddress}`;
    } else {
      // Query all pools for this event type
      const pattern = `cbmm-demo:events:${type}:*`;
      const allKeys = await redis.keys(pattern);
      const uniquePools = new Set<string>();
      
      for (const key of allKeys) {
        const match = key.match(/^events:(buy|sell|burn):([^:]+):/);
        if (match) {
          uniquePools.add(match[2]);
        }
      }

      // Query each pool
      for (const pool of uniquePools) {
        const poolEvents = await getEvents({ ...query, eventType: type, poolAddress: pool });
        events.push(...poolEvents);
      }
      continue;
    }

    // Use sorted set for range queries
    const min = startTimestamp ?? '-inf';
    const max = endTimestamp ?? '+inf';
    
    const keys = await redis.zrangebyscore(keyPattern, min, max, 'LIMIT', 0, limit);

    for (const key of keys) {
      const jsonValue = await redis.get(key);
      if (jsonValue) {
        try {
          const event = JSON.parse(jsonValue) as StoredEvent;
          // Apply filters
          if (poolAddress && event.poolAddress !== poolAddress) continue;
          if (userAddress && event.userAddress !== userAddress) continue;
          if (startTimestamp && event.timestamp < startTimestamp) continue;
          if (endTimestamp && event.timestamp > endTimestamp) continue;
          
          events.push(event);
        } catch (error) {
          console.error(`Failed to parse event fromkey ${key}:`, error);
        }
      }
    }
  }

  // Sort by timestamp descending and limit
  events.sort((a, b) => b.timestamp - a.timestamp);
  return events.slice(0, limit);
}

