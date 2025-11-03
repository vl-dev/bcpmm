import { createHash } from 'crypto';
import {
  getBurnEventDecoder,
  getBuyEventDecoder,
  getSellEventDecoder,
  type BurnEvent,
  type BuyEvent,
  type SellEvent,
} from '@cbmm/js-client';

// Compute event discriminators (first 8 bytes of SHA256("event:EventName"))
function getEventDiscriminator(eventName: string): Uint8Array {
  const hash = createHash('sha256').update(`event:${eventName}`).digest();
  return new Uint8Array(hash.slice(0, 8));
}

const BURN_EVENT_DISCRIMINATOR = getEventDiscriminator('BurnEvent');
const BUY_EVENT_DISCRIMINATOR = getEventDiscriminator('BuyEvent');
const SELL_EVENT_DISCRIMINATOR = getEventDiscriminator('SellEvent');

export type ParsedEvent = BuyEvent | SellEvent | BurnEvent;

export function parseLogs(logs: string[]): ParsedEvent | null {
  for (const log of logs) {
    // Events are logged as "Program data: <base64_data>"
    if (log.startsWith('Program data: ')) {
      const base64Data = log.slice('Program data: '.length);
      
      try {
        // Decode base64 to bytes
        const eventData = Buffer.from(base64Data, 'base64');
        
        // Check the discriminator (first 8 bytes)
        const discriminator = eventData.slice(0, 8);
        
        // Try each event type
        if (discriminator.equals(Buffer.from(BURN_EVENT_DISCRIMINATOR))) {
          const decoder = getBurnEventDecoder();
          const burnEvent = decoder.decode(eventData.slice(8));
          return burnEvent;
        }
        
        if (discriminator.equals(Buffer.from(BUY_EVENT_DISCRIMINATOR))) {
          const decoder = getBuyEventDecoder();
          const buyEvent = decoder.decode(eventData.slice(8));
          return buyEvent;
        }
        
        if (discriminator.equals(Buffer.from(SELL_EVENT_DISCRIMINATOR))) {
          const decoder = getSellEventDecoder();
          const sellEvent = decoder.decode(eventData.slice(8));
          return sellEvent;
        }
      } catch (error) {
        // Skip invalid log entries
        continue;
      }
    }
  }
  
  return null;
}

