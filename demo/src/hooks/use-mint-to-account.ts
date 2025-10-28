import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useAdminKeypair } from './use-admin-keypair';
import {
  createSignerFromKeyPair,
  createTransactionMessage,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  appendTransactionMessageInstructions,
  signTransactionMessageWithSigners,
  assertIsSendableTransaction,
  pipe,
  type Address,
  Instruction,
  address,
} from '@solana/kit';
import { getTxClient } from '../solana/tx-client';
import {
  getCreateAssociatedTokenIdempotentInstructionAsync,
  getMintToInstruction,
  findAssociatedTokenPda,
  TOKEN_PROGRAM_ADDRESS,
} from '@solana-program/token';
import { Buffer } from 'buffer';

const DECIMALS = 6; // Same as mint

interface MintToAccountParams {
  user: Address;
  amount: number; // Amount in human-readable format (e.g., 100 tokens)
}

export function useMintToAccount() {
  const { data: adminKeypair } = useAdminKeypair();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ user, amount }: MintToAccountParams) => {
      if (!adminKeypair) throw new Error('Admin keypair not available');
      
      const { rpc, sendAndConfirmTransaction } = await getTxClient();
      const adminSigner = await createSignerFromKeyPair(adminKeypair.keyPair);
      
      // Get mint address from localStorage
      const mintAddress = localStorage.getItem('mint_address');
      if (!mintAddress) throw new Error('Mint address not found');
      const mint = address(mintAddress);

      // Find associated token address
      const [associatedTokenAddress] = await findAssociatedTokenPda({
        mint,
        owner: user,
        tokenProgram: TOKEN_PROGRAM_ADDRESS,
      });

      // Check if account exists
      const account = await rpc.getAccountInfo(associatedTokenAddress, { commitment: 'confirmed', encoding: 'base64' }).send();
      const accountExists = !!account.value;

      // Build instructions
      const instructions: Instruction[] = [];
      
      // Add create ATA instruction if it doesn't exist
      if (!accountExists) {
        const createAtaInstruction = await getCreateAssociatedTokenIdempotentInstructionAsync({
          mint,
          payer: adminSigner,
          owner: user,
        });
        instructions.push(createAtaInstruction);
      }

      // Add mint instruction
      const mintAmount = BigInt(amount * 10 ** DECIMALS);
      const mintInstruction = getMintToInstruction({
        mint,
        token: associatedTokenAddress,
        amount: mintAmount,
        mintAuthority: adminSigner,
      });
      instructions.push(mintInstruction);

      // Build transaction
      const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
      
      const transactionMessage = pipe(
        createTransactionMessage({ version: 0 }),
        (tx) => setTransactionMessageFeePayerSigner(adminSigner, tx),
        (tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
        (tx) => appendTransactionMessageInstructions(instructions, tx),
      );

      const signedTx = await signTransactionMessageWithSigners(transactionMessage);
      assertIsSendableTransaction(signedTx);

      // Send and confirm transaction
      await sendAndConfirmTransaction(signedTx as any, { commitment: 'confirmed' });
      const signature = signedTx.signatures[adminSigner.address];
      const sigatureB64 = Buffer.from(signature?.toString() || '').toString('base64');
      
      console.log(`✅ Tokens minted: ${sigatureB64}`);
      
      return sigatureB64;
    },
    onSuccess: () => {
      // Invalidate token balance queries to refresh the UI
      queryClient.invalidateQueries({ queryKey: ['tokenBalance'] });
    },
  });
}

