import { CBMM_PROGRAM_ADDRESS, getCreatePoolInstructionAsync } from "@cbmm/js-client";
import { Address, createSignerFromKeyPair, getBytesEncoder, getProgramDerivedAddress, KeyPairSigner, appendTransactionMessageInstruction, setTransactionMessageLifetimeUsingBlockhash, setTransactionMessageFeePayerSigner, signTransactionMessageWithSigners, assertIsSendableTransaction, getBase64EncodedWireTransaction, pipe, getBase64Encoder, getAddressEncoder } from "@solana/kit";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { getTxClient } from "../solana/tx-client";
import { SYSTEM_PROGRAM_ADDRESS } from "@solana-program/system";
import { TOKEN_PROGRAM_ADDRESS } from "@solana-program/token";
import { createTransactionMessage } from "@solana/kit";

interface CreatePoolParams {
  user: KeyPairSigner;
  mint: Address;
  aVirtualReserve: number;
}

export function useCreatePool() {
  const queryClient = useQueryClient();

  // ALLOW CHOOSING VIRT RESERVE AMOUNT
  return useMutation({
    mutationFn: async ({ user, mint, aVirtualReserve }: CreatePoolParams) => {
      const { rpc, sendAndConfirmTransaction } = await getTxClient();
      const userSigner = await createSignerFromKeyPair(user.keyPair);

      // Get central state address
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


      const userAddress = getAddressEncoder().encode(userSigner.address);
      const [poolAddress] = await getProgramDerivedAddress({
        programAddress: CBMM_PROGRAM_ADDRESS,
        seeds: [
          getBytesEncoder().encode(new Uint8Array([98, 99, 112, 109, 109, 95, 112, 111, 111, 108])), // "bcpmm_pool"
          getBytesEncoder().encode(new Uint8Array([0, 0, 0, 0])),
          userAddress,
        ],
      });

      const createPoolInstruction = await getCreatePoolInstructionAsync({
        payer: userSigner,
        aMint: mint,
        centralState: centralStateAddress,
        tokenProgram: TOKEN_PROGRAM_ADDRESS,
        systemProgram: SYSTEM_PROGRAM_ADDRESS,
        aVirtualReserve: BigInt(aVirtualReserve) * 10n ** 6n,
      });

      const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
      const transactionMessage = pipe(
        createTransactionMessage({ version: 0 }),
        (tx) => setTransactionMessageFeePayerSigner(userSigner, tx),
        (tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
        (tx) => appendTransactionMessageInstruction(createPoolInstruction, tx),
      );

      const signedTx = await signTransactionMessageWithSigners(transactionMessage);
      assertIsSendableTransaction(signedTx);

      try {
        const txBase64 = getBase64EncodedWireTransaction(signedTx);
        const simulateResult = await rpc
          .simulateTransaction(txBase64, { encoding: 'base64', sigVerify: true, commitment: 'confirmed' })
          .send();
          console.log('txBase64', txBase64);
        console.log('simulate createPool', simulateResult);

        await sendAndConfirmTransaction(signedTx as any, { commitment: 'confirmed' });
        console.log('createPool tx sent and confirmed');
      } catch (error) {
        console.error('createPool tx error', error);
        throw error;
      }

      return poolAddress;
    },
    onSuccess: () => {
      // Ensure user pool refetches
      queryClient.invalidateQueries({ queryKey: ['userPool'] });
      queryClient.invalidateQueries({ queryKey: ['allPools'] });
    },
  });
}