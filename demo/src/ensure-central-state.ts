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
} from '@bcpmm/js-client';
import { CPMM_POC_PROGRAM_ADDRESS } from '@bcpmm/js-client';
import { getTxClient } from './solana/tx-client';

export async function ensureCentralState(
  adminKeypair: KeyPairSigner
): Promise<Address> {
  const { rpc, sendAndConfirmTransaction } = await getTxClient();
  const adminSigner = await createSignerFromKeyPair(adminKeypair.keyPair);

  // Derive central state PDA
  const [centralStateAddress] = await getProgramDerivedAddress({
    programAddress: CPMM_POC_PROGRAM_ADDRESS,
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
    console.log('Central state does not exist, creating...');
    
    // Get the instruction
    const instruction = await getInitializeCentralStateInstructionAsync({
      admin: adminSigner,
      centralState: centralStateAddress,
      dailyBurnAllowance: 100, // 100 tokens per day
      creatorDailyBurnAllowance: 1000, // 1000 tokens per day for creators
      userBurnBpX100: 1000, // 10% (1000/10000)
      creatorBurnBpX100: 500, // 5% (500/10000)
      burnResetTimeOfDaySeconds: 0, // Midnight
    });

    // Build and send transaction
    const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
    
    const transactionMessage = pipe(
      createTransactionMessage({ version: 0 }),
      (tx) => setTransactionMessageFeePayerSigner(adminSigner, tx),
      (tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx), 
      (tx) => appendTransactionMessageInstruction(instruction, tx),
    )

    const singedTx = await signTransactionMessageWithSigners(transactionMessage);
    assertIsSendableTransaction(singedTx);
    console.log('Sending transaction');
    try {
      const txBase64 = getBase64EncodedWireTransaction(singedTx);
      let simulateResult = await rpc
        .simulateTransaction(txBase64, { encoding: 'base64', sigVerify: true, commitment: 'confirmed' })
        .send();

      console.log('Txbase64', txBase64);
      console.log(simulateResult);

      await sendAndConfirmTransaction(singedTx as any, { commitment: 'confirmed' });
      const signature = singedTx.signatures[adminSigner.address];
      console.log('Central state created', signature?.toString());
    } catch (error) {
      console.error('Error sending transaction', error);
    }
  } else {
    console.log('Central state already exists');
  }

  return centralStateAddress;
}

