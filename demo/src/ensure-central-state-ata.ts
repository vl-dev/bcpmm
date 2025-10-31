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
  address,
} from '@solana/kit';
import { CBMM_PROGRAM_ADDRESS } from '@cbmm/js-client';
import { getTxClient } from './solana/tx-client';
import {
  getCreateAssociatedTokenIdempotentInstructionAsync,
  findAssociatedTokenPda,
  TOKEN_PROGRAM_ADDRESS,
} from '@solana-program/token';

export async function ensureCentralStateAta(
  adminKeypair: KeyPairSigner
): Promise<Address> {
  const { rpc, sendAndConfirmTransaction } = await getTxClient();
  const adminSigner = await createSignerFromKeyPair(adminKeypair.keyPair);

  // Get mint address from localStorage
  const mintAddress = localStorage.getItem('mint_address');
  if (!mintAddress) throw new Error('Mint address not found');
  const mint = address(mintAddress);

  // Verify the mint actually exists on-chain
  const mintAccount = await rpc.getAccountInfo(mint, { commitment: 'confirmed' }).send();
  if (!mintAccount.value) {
    throw new Error(`Mint account ${mintAddress} does not exist on-chain`);
  }

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

  // Find associated token address for central state
  const [associatedTokenAddress] = await findAssociatedTokenPda({
    mint,
    owner: centralStateAddress,
    tokenProgram: TOKEN_PROGRAM_ADDRESS,
  });

  // Check if account exists
  const account = await rpc
    .getAccountInfo(associatedTokenAddress, { commitment: 'confirmed', encoding: 'base64' })
    .send();
  const accountExists = !!account.value;

  if (!accountExists) {
    console.log('Central state ATA does not exist, creating...');

    // Get the create ATA instruction
    const instruction = await getCreateAssociatedTokenIdempotentInstructionAsync({
      mint,
      payer: adminSigner,
      owner: centralStateAddress,
    });

    // Build and send transaction
    const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

    const transactionMessage = pipe(
      createTransactionMessage({ version: 0 }),
      (tx) => setTransactionMessageFeePayerSigner(adminSigner, tx),
      (tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
      (tx) => appendTransactionMessageInstruction(instruction, tx),
    );

    const signedTx = await signTransactionMessageWithSigners(transactionMessage);
    assertIsSendableTransaction(signedTx);
    console.log('Sending create ATA transaction');
    try {
      const txBase64 = getBase64EncodedWireTransaction(signedTx);
      const simulateResult = await rpc
        .simulateTransaction(txBase64, { encoding: 'base64', sigVerify: true, commitment: 'confirmed' })
        .send();

      console.log('createCentralStateAta simulation', simulateResult);
      console.log('txBase64', txBase64);

      await sendAndConfirmTransaction(signedTx as any, { commitment: 'confirmed' });
      console.log('Central state ATA created')
    } catch (error) {
      console.error('Error sending create ATA transaction', error);
      throw error;
    }
  } else {
    console.log('Central state ATA already exists');
  }

  return associatedTokenAddress;
}

