import {
  createTransactionMessage,
  appendTransactionMessageInstructions,
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
import { generateKeyPair } from '@solana/keys';
import { getMintSize, getInitializeMintInstruction, TOKEN_PROGRAM_ADDRESS } from '@solana-program/token';
import { getCreateAccountInstruction } from '@solana-program/system';
import { getTxClient } from './solana/tx-client';

const MINT_STORAGE_KEY = 'mint_address';
const DECIMALS = 6; // Adjust decimals as needed

export async function ensureMint(
  adminKeypair: KeyPairSigner
): Promise<Address> {
  const { rpc, sendAndConfirmTransaction } = await getTxClient();
  const adminSigner = await createSignerFromKeyPair(adminKeypair.keyPair);

  // Check if mint address exists in localStorage
  const storedMintAddress = localStorage.getItem(MINT_STORAGE_KEY);
  if (storedMintAddress) {
    console.log('Mint address found in localStorage:', storedMintAddress);
    return storedMintAddress as Address;
  }

  console.log('No mint address in localStorage, creating new mint...');

  // Generate a new keypair for the mint
  const mintKeypair = await generateKeyPair();
  const mintSigner = await createSignerFromKeyPair(mintKeypair);

  // Get mint size and calculate rent
  const mintSpace = BigInt(getMintSize());
  const mintRent = await rpc.getMinimumBalanceForRentExemption(mintSpace).send();

  console.log(`Mint space: ${mintSpace}, Rent: ${mintRent}`);

  // Create instructions
  const createAccountInstruction = getCreateAccountInstruction({
    payer: adminSigner,
    newAccount: mintSigner,
    lamports: mintRent,
    space: mintSpace,
    programAddress: TOKEN_PROGRAM_ADDRESS,
  });

  const initializeMintInstruction = getInitializeMintInstruction({
    mint: mintSigner.address,
    decimals: DECIMALS,
    mintAuthority: adminSigner.address,
  });

  const instructions = [createAccountInstruction, initializeMintInstruction];

  // Get latest blockhash
  const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

  // Build transaction
  const transactionMessage = pipe(
    createTransactionMessage({ version: 0 }),
    (tx) => setTransactionMessageFeePayerSigner(adminSigner, tx),
    (tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
    (tx) => appendTransactionMessageInstructions(instructions, tx),
  );

  const signedTx = await signTransactionMessageWithSigners(transactionMessage);
  assertIsSendableTransaction(signedTx);

  console.log('Sending mint creation transaction');
  try {
    const txBase64 = getBase64EncodedWireTransaction(signedTx);
    const simulateResult = await rpc
      .simulateTransaction(txBase64, { encoding: 'base64', sigVerify: true, commitment: 'confirmed' })
      .send();

    console.log('Transaction simulation:', simulateResult);

    await sendAndConfirmTransaction(signedTx as any, { commitment: 'confirmed' });
    console.log('✅ Mint created')

    // Store the mint address in localStorage
    localStorage.setItem(MINT_STORAGE_KEY, mintSigner.address.toString());
    console.log('Mint address stored:', mintSigner.address.toString());

    return mintSigner.address;
  } catch (error) {
    console.error('Error creating mint:', error);
    throw error;
  }
}
