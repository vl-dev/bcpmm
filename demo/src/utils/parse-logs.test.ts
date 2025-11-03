import test from 'node:test';
import assert from 'node:assert';
import { parseLogs } from './parse-logs';
import type { SellEvent, BurnEvent, BuyEvent } from '@cbmm/js-client';

const SELL_EVENT_LOGS = [
  'Program CBMMzs3HKfTMudbXifeNcw3NcHQhZX7izDBKoGDLRdjj invoke [1]',
  'Program log: Instruction: SellVirtualToken',
  'Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]',
  'Program log: Instruction: TransferChecked',
  'Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 6173 of 171325 compute units',
  'Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success',
  'Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]',
  'Program log: Instruction: TransferChecked',
  'Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 6173 of 162792 compute units',
  'Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success',
  'Program data: Pi83CqUD3CqAsU8BAAAAAM4TAAAAAAAAMwAAAAAAAABmAAAAAAAAAJkAAAAAAAAAAAAAAAAAAAAqssRi3R0AAAyrRLQBAAAAAAAAAAAAAABehrcaAAAAAPc3WDUAAAAA6I+Tkpul2+RYhoWIzJ5goM+Qps0oDAx2WM4lIYgj8cUTVB9EAU572BgoaX2Wui8jaYR0W14ezKuhWzmANtP1Ig==',
  'Program CBMMzs3HKfTMudbXifeNcw3NcHQhZX7izDBKoGDLRdjj consumed 46372 of 200000 compute units',
  'Program CBMMzs3HKfTMudbXifeNcw3NcHQhZX7izDBKoGDLRdjj success',
];

const BURN_EVENT_LOGS = [
  'Program CBMMzs3HKfTMudbXifeNcw3NcHQhZX7izDBKoGDLRdjj invoke [1]',
  'Program CBMMzs3HKfTMudbXifeNcw3NcHQhZX7izDBKoGDLRdjj invoke [1]',
  'Program log: Instruction: BurnVirtualToken',
  'Program data: IVkvdVJ87vrdNy2nBwAAAAAAAAAAAAAAqgB1Yd0dAADavkS0AQAAAAAAAAAAAAAAv93PDgAAAACRN1g1AAAAAOiPk5KbpdvkWIaFiMyeYKDPkKbNKAwMdljOJSGII/HFE1QfRAFOe9gYKGl9lrovI2mEdFteHsyroVs5gDbT9SI=',
  'Program CBMMzs3HKfTMudbXifeNcw3NcHQhZX7izDBKoGDLRdjj consumed 19210 of 200000 compute units',
  'Program CBMMzs3HKfTMudbXifeNcw3NcHQhZX7izDBKoGDLRdjj success',
];

const BUY_EVENT_LOGS = [
  'Program CBMMzs3HKfTMudbXifeNcw3NcHQhZX7izDBKoGDLRdjj invoke [1]',
  'Program log: Instruction: BuyVirtualToken',
  'Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]',
  'Program log: Instruction: TransferChecked',
  'Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 6173 of 171088 compute units',
  'Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success',
  'Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]',
  'Program log: Instruction: TransferChecked',
  'Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 6173 of 162624 compute units',
  'Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success',
  'Program data: Z/RSHyz1d3cAKGvuAAAAADo/BgCBHQAAAFpiAgAAAAAAtMQEAAAAAAAOJwcAAAAAAAAAAAAAAADHpYe94x0AAAd4VLQBAAAAAAAAAAAAAAAzVLcaAAAAAFKfWzUAAAAA6I+Tkpul2+RYhoWIzJ5goM+Qps0oDAx2WM4lIYgj8cUTVB9EAU572BgoaX2Wui8jaYR0W14ezKuhWzmANtP1Ig==',
  'Program CBMMzs3HKfTMudbXifeNcw3NcHQhZX7izDBKoGDLRdjj consumed 46492 of 200000 compute units',
  'Program CBMMzs3HKfTMudbXifeNcw3NcHQhZX7izDBKoGDLRdjj success',
];

test('parses SellEvent from logs', () => {
  const result = parseLogs(SELL_EVENT_LOGS);
  
  assert(result !== null, 'Expected to parse a SellEvent, but got null');
  
  // Type guard to ensure it's a SellEvent
  const sellEvent = result as SellEvent;
  
  // Check that it has SellEvent-specific fields
  assert(sellEvent.bInput !== undefined, 'Expected SellEvent with bInput field');
  assert(sellEvent.aOutput !== undefined, 'Expected SellEvent with aOutput field');
  assert(sellEvent.seller !== undefined, 'Expected SellEvent with seller field');
  assert(sellEvent.pool !== undefined, 'Expected SellEvent with pool field');
});

test('parses BurnEvent from logs', () => {
  const result = parseLogs(BURN_EVENT_LOGS);
  
  assert(result !== null, 'Expected to parse a BurnEvent, but got null');
  
  // Type guard to ensure it's a BurnEvent
  const burnEvent = result as BurnEvent;
  
  // Check that it has BurnEvent-specific fields
  assert(burnEvent.burnAmount !== undefined, 'Expected BurnEvent with burnAmount field');
  assert(burnEvent.burner !== undefined, 'Expected BurnEvent with burner field');
  assert(burnEvent.pool !== undefined, 'Expected BurnEvent with pool field');
});

test('parses BuyEvent from logs', () => {
  const result = parseLogs(BUY_EVENT_LOGS);
  
  assert(result !== null, 'Expected to parse a BuyEvent, but got null');
  
  // Type guard to ensure it's a BuyEvent
  const buyEvent = result as BuyEvent;
  
  // Check that it has BuyEvent-specific fields
  assert(buyEvent.aInput !== undefined, 'Expected BuyEvent with aInput field');
  assert(buyEvent.bOutput !== undefined, 'Expected BuyEvent with bOutput field');
  assert(buyEvent.buyer !== undefined, 'Expected BuyEvent with buyer field');
  assert(buyEvent.pool !== undefined, 'Expected BuyEvent with pool field');
});
