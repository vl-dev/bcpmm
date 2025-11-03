import { NextRequest, NextResponse } from 'next/server';
import type { BuyEvent } from '@cbmm/js-client';
import type { SellEvent } from '@cbmm/js-client';
import type { BurnEvent } from '@cbmm/js-client';
import { recordBuyEvent } from '../../../../src/redis/record-buy-event';
import { recordSellEvent } from '../../../../src/redis/record-sell-event';
import { recordBurnEvent } from '../../../../src/redis/record-burn-event';

const CBMM_PROGRAM_ID = 'CBMMzs3HKfTMudbXifeNcw3NcHQhZX7izDBKoGDLRdjj';
const WEBHOOK_SECRET = process.env.WEBHOOK_SECRET;

type HeliusWebhookPayload = Array<{
  accountData: Array<{
    account: string;
    nativeBalanceChange: number;
    tokenBalanceChanges: Array<unknown>;
  }>;
  timestamp: number;
  signature: string;
  slot: number;
  events?: Array<Record<string, unknown>> | Record<string, unknown>;
  feePayer?: string;
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

      console.log('[WEBHOOK] tx', tx);
      // Check if this transaction involves our program
      const involvesCBMM = tx.accountData?.some(
        (acc) => acc.account === CBMM_PROGRAM_ID
      );

      if (!involvesCBMM) {
        continue;
      }

      const timestamp = tx.timestamp;
      if (!timestamp) {
        results.push({ success: false, error: 'Missing timestamp' });
        continue;
      }

      // Parse events directly from tx.events
      if (!tx.events) {
        continue;
      }

      try {
        // Convert events to array if it's an object
        const eventsArray: Array<Record<string, unknown>> = Array.isArray(tx.events) 
          ? (tx.events as Array<Record<string, unknown>>)
          : Object.values(tx.events) as Array<Record<string, unknown>>;

        // Find BuyEvent by aInput field
        const buyEvent = eventsArray.find((event) => event.aInput !== undefined) as BuyEvent | undefined;
        if (buyEvent) {
          console.log('[WEBHOOK] buyEvent', buyEvent);
          await recordBuyEvent(buyEvent, timestamp);
          results.push({ success: true, eventType: 'buy' });
        }

        // Find SellEvent by bInput field
        const sellEvent = eventsArray.find((event) => event.bInput !== undefined) as SellEvent | undefined;
        if (sellEvent) {
          console.log('[WEBHOOK] sellEvent', sellEvent);
          await recordSellEvent(sellEvent, timestamp);
          results.push({ success: true, eventType: 'sell' });
        }

        // Find BurnEvent by burnAmount field
        const burnEvent = eventsArray.find((event) => event.burnAmount !== undefined) as BurnEvent | undefined;
        if (burnEvent) {
          console.log('[WEBHOOK] burnEvent', burnEvent);
          await recordBurnEvent(burnEvent, timestamp);
          results.push({ success: true, eventType: 'burn' });
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

