import { NextRequest, NextResponse } from 'next/server';
import type { BuyEvent } from '@cbmm/js-client';
import type { SellEvent } from '@cbmm/js-client';
import type { BurnEvent } from '@cbmm/js-client';
import { recordBuyEvent } from '../../../../src/redis/record-buy-event';
import { recordSellEvent } from '../../../../src/redis/record-sell-event';
import { recordBurnEvent } from '../../../../src/redis/record-burn-event';

const CBMM_PROGRAM_ID = 'CBMMzs3HKfTMudbXifeNcw3NcHQhZX7izDBKoGDLRdjj';

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

    if (!Array.isArray(payload)) {
      return NextResponse.json({ error: 'Invalid payload format' }, { status: 400 });
    }

    const headers = request.headers;
    const authHeader = headers.get('Authorization');
    console.log('authHeader', authHeader);

    const results: Array<{ success: boolean; eventType?: string; error?: string }> = [];

    for (const tx of payload) {
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

      const poolAddress = tx.accountData?.find(
        (acc) => 
          acc.account !== CBMM_PROGRAM_ID && 
          acc.account !== '11111111111111111111111111111111' &&
          acc.account !== 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA' &&
          acc.account !== 'SysvarRent111111111111111111111111111111111' &&
          acc.account !== 'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL'
      )?.account;

      // User is typically the fee payer
      const userAddress = tx.feePayer || tx.accountData?.find(
        (acc) => acc.nativeBalanceChange < 0 || acc.tokenBalanceChanges.length > 0
      )?.account;

      if (!poolAddress || !userAddress) {
        results.push({ success: false, error: 'Could not extract pool or user address' });
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
          await recordBuyEvent(buyEvent, timestamp);
          results.push({ success: true, eventType: 'buy' });
        }

        // Find SellEvent by bInput field
        const sellEvent = eventsArray.find((event) => event.bInput !== undefined) as SellEvent | undefined;
        if (sellEvent) {
          await recordSellEvent(sellEvent, timestamp);
          results.push({ success: true, eventType: 'sell' });
        }

        // Find BurnEvent by burnAmount field
        const burnEvent = eventsArray.find((event) => event.burnAmount !== undefined) as BurnEvent | undefined;
        if (burnEvent) {
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

