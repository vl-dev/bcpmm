import {
  createTransactionMessage,
  appendTransactionMessageInstruction,
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
  getBytesEncoder,
  getProgramDerivedAddress,
  getAddressEncoder,
} from '@solana/kit';
import { generateKeyPair } from '@solana/keys';
import { getMintSize, getInitializeMintInstruction, TOKEN_PROGRAM_ADDRESS } from '@solana-program/token';
import { getCreateAccountInstruction } from '@solana-program/system';
import { CPMM_POC_PROGRAM_ADDRESS, getInitializeTreasuryInstructionAsync, fetchMaybeTreasury } from '@bcpmm/js-client';
import { getTxClient } from './solana/tx-client';

const MINT_STORAGE_KEY = 'mint_address';
const DECIMALS = 6; // Adjust decimals as needed

export async function ensureTreasury(
  adminKeypair: KeyPairSigner,
  mint: Address
): Promise<Address> {
  const { rpc, sendAndConfirmTransaction } = await getTxClient();
  const adminSigner = await createSignerFromKeyPair(adminKeypair.keyPair);
  console.log('Admin signer', adminSigner.address.toString());

  const [mintTreasuryAddress, _bump] = await getProgramDerivedAddress({
    programAddress: CPMM_POC_PROGRAM_ADDRESS,
    seeds: [
      getBytesEncoder().encode(
        new Uint8Array([116, 114, 101, 97, 115, 117, 114, 121]) // "treasury"
      ),
      getAddressEncoder().encode(mint),
    ],
  });

  const mintTreasury = await fetchMaybeTreasury(rpc, mintTreasuryAddress);
  if (mintTreasury.exists) {
    return mintTreasury.address;
  }

  console.log('Treasury does not exist, creating...');

  const initializeTreasuryInstruction = await getInitializeTreasuryInstructionAsync({
    admin: adminSigner,
    aMint: mint,
    treasuryAuthority: adminSigner.address,
  });

  const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

  const transactionMessage = pipe(
    createTransactionMessage({ version: 0 }),
    (tx) => setTransactionMessageFeePayerSigner(adminSigner, tx),
    (tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
    (tx) => appendTransactionMessageInstruction(initializeTreasuryInstruction, tx),
  );

  const signedTx = await signTransactionMessageWithSigners(transactionMessage);
  assertIsSendableTransaction(signedTx);
  
  console.log('Sending treasury creation transaction');
  try {
    await sendAndConfirmTransaction(signedTx as any, { commitment: 'confirmed' });
    console.log('✅ Treasury created')

    return mintTreasuryAddress;
  } catch (error) {
    console.error('Error creating treasury:', error);
    throw error;
  }
}

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
