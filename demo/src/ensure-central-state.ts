import {
  getProgramDerivedAddress,
  getBytesEncoder,
  type Address,
} from '@solana/kit';
import {
  fetchMaybeCentralState,
} from '@cbmm/js-client';
import { CBMM_PROGRAM_ADDRESS } from '@cbmm/js-client';
import { getTxClient } from './solana/tx-client';

export async function ensureCentralState(): Promise<Address> {
  const { rpc } = await getTxClient();

  // Derive central state PDA
  const [centralStateAddress] = await getProgramDerivedAddress({
    programAddress: CBMM_PROGRAM_ADDRESS,
    seeds: [
      getBytesEncoder().encode(
        new Uint8Array([
          99, 101, 110, 116, 114, 97, 108, 95, 115, 116, 97, 116, 101,
        ])
      ),
    ],
  });

  // Check if central state exists
  const maybeCentralState = await fetchMaybeCentralState(rpc, centralStateAddress);

  if (!maybeCentralState.exists) {
    alert('Central state does not cexist. Create it first before running the demo!');
    throw new Error('Central state does not exist');
  }

  return centralStateAddress;
}

