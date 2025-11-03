import { NextRequest, NextResponse } from 'next/server';
import type { BuyEvent } from '@cbmm/js-client';
import type { SellEvent } from '@cbmm/js-client';
import type { BurnEvent } from '@cbmm/js-client';
import { parseLogs } from '../../../../src/utils/parse-logs';
import { recordBuyEvent } from '../../../../src/redis/record-buy-event';
import { recordSellEvent } from '../../../../src/redis/record-sell-event';
import { recordBurnEvent } from '../../../../src/redis/record-burn-event';

const CBMM_PROGRAM_ID = 'CBMMzs3HKfTMudbXifeNcw3NcHQhZX7izDBKoGDLRdjj';
const WEBHOOK_SECRET = process.env.WEBHOOK_SECRET;

type HeliusWebhookPayload = Array<{
  blockTime: number | null;
  indexWithinBlock: number;
  meta: {
    err: unknown;
    fee: number;
    logMessages: string[] | null;
    [key: string]: unknown;
  };
  slot: number;
  transaction: {
    message: {
      accountKeys: string[];
      [key: string]: unknown;
    };
    [key: string]: unknown;
  };
  [key: string]: unknown;
}>;

export async function POST(request: NextRequest) {
  try {
    const payload: HeliusWebhookPayload = await request.json();
    console.log('[WEBHOOK] processing request');

    if (!Array.isArray(payload)) {
      console.error('[WEBHOOK] Invalid payload format');
      return NextResponse.json({ error: 'Invalid payload format' }, { status: 400 });
    }

    const headers = request.headers;
    const authHeader = headers.get('Authorization');
    if(!WEBHOOK_SECRET) {
      console.error('[WEBHOOK] WEBHOOK_SECRET is not set');
      return NextResponse.json({ error: 'WEBHOOK_SECRET is not set' }, { status: 400 });
    }
    if (authHeader?.trim() !== WEBHOOK_SECRET) {
      console.error('[WEBHOOK] Invalid authorization header');
      return NextResponse.json({ error: 'Invalid authorization header' }, { status: 401 });
    }

    const results: Array<{ success: boolean; eventType?: string; error?: string }> = [];

    for (const tx of payload) {

      // Check if this transaction involves our program by checking account keys
      const involvesCBMM = tx.transaction?.message?.accountKeys?.some(
        (key: string) => key === CBMM_PROGRAM_ID
      );

      if (!involvesCBMM) {
        continue;
      }

      const blockTime = tx.blockTime;
      if (!blockTime) {
        results.push({ success: false, error: 'Missing blockTime' });
        continue;
      }

      // Parse events from log messages
      const logMessages = tx.meta?.logMessages;
      if (!logMessages || !Array.isArray(logMessages) || logMessages.length === 0) {
        continue;
      }

      try {
        // Parse the event from logs
        const parsedEvent = parseLogs(logMessages);
        
        if (!parsedEvent) {
          // No event found in logs, skip
          continue;
        }

        // Check event type and record accordingly
        if ('bInput' in parsedEvent && 'aOutput' in parsedEvent && 'seller' in parsedEvent) {
          // SellEvent
          const sellEvent = parsedEvent as SellEvent;
          console.log('[WEBHOOK] sellEvent', sellEvent);
          await recordSellEvent(sellEvent, blockTime);
          results.push({ success: true, eventType: 'sell' });
        } else if ('aInput' in parsedEvent && 'bOutput' in parsedEvent && 'buyer' in parsedEvent) {
          // BuyEvent
          const buyEvent = parsedEvent as BuyEvent;
          console.log('[WEBHOOK] buyEvent', buyEvent);
          await recordBuyEvent(buyEvent, blockTime);
          results.push({ success: true, eventType: 'buy' });
        } else if ('burnAmount' in parsedEvent && 'burner' in parsedEvent) {
          // BurnEvent
          const burnEvent = parsedEvent as BurnEvent;
          console.log('[WEBHOOK] burnEvent', burnEvent);
          await recordBurnEvent(burnEvent, blockTime);
          results.push({ success: true, eventType: 'burn' });
        } else {
          console.warn('[WEBHOOK] Unknown event type:', parsedEvent);
          results.push({ success: false, error: 'Unknown event type' });
        }
      } catch (error) {
        console.error('Error processing event:', error);
        results.push({ success: false, error: `Failed to process event: ${error}` });
      }
    }

    return NextResponse.json({ 
      processed: results.length,
      results 
    }, { status: 200 });
  } catch (error) {
    console.error('Error processing webhook:', error);
    return NextResponse.json(
      { error: 'Failed to process webhook', details: error instanceof Error ? error.message : 'Unknown error' },
      { status: 500 }
    );
  }
}
