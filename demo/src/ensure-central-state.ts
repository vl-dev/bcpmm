import {
  getProgramDerivedAddress,
  getBytesEncoder,
  createTransactionMessage,
  appendTransactionMessageInstruction,
  setTransactionMessageLifetimeUsingBlockhash,
  createSignerFromKeyPair,
  type Address,
  type KeyPairSigner,
  pipe,
  setTransactionMessageFeePayerSigner,
  signTransactionMessageWithSigners,
  assertIsSendableTransaction,
  getBase64EncodedWireTransaction,
} from '@solana/kit';
import {
  getInitializeCentralStateInstructionAsync,
  fetchMaybeCentralState,
} from '@cbmm/js-client';
import { CBMM_PROGRAM_ADDRESS } from '@cbmm/js-client';
import { getTxClient } from './solana/tx-client';

export async function ensureCentralState(
  adminKeypair: KeyPairSigner
): Promise<Address> {
  const { rpc, sendAndConfirmTransaction } = await getTxClient();
  const adminSigner = await createSignerFromKeyPair(adminKeypair.keyPair);

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

