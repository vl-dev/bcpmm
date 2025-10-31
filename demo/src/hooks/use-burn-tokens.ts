import { getBurnVirtualTokenInstructionAsync, CBMM_PROGRAM_ADDRESS } from "@cbmm/js-client";
import { getInitializeUserBurnAllowanceInstructionAsync } from "@cbmm/js-client";
import { Address, createSignerFromKeyPair, getBytesEncoder, getProgramDerivedAddress, KeyPairSigner, appendTransactionMessageInstruction, setTransactionMessageLifetimeUsingBlockhash, setTransactionMessageFeePayerSigner, signTransactionMessageWithSigners, assertIsSendableTransaction, getBase64EncodedWireTransaction, pipe, createTransactionMessage, getAddressEncoder } from "@solana/kit";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { getTxClient } from "../solana/tx-client";

interface BurnTokensParams {
  user: KeyPairSigner;
  pool: Address;
  poolOwner: boolean;
}

export function useBurnTokens() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ user, pool, poolOwner }: BurnTokensParams) => {
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

      // Get user burn allowance address
      const userAddressBytes = getAddressEncoder().encode(userSigner.address);
      const poolOwnerByte = poolOwner ? 1 : 0;
      const [userBurnAllowanceAddress] = await getProgramDerivedAddress({
        programAddress: CBMM_PROGRAM_ADDRESS,
        seeds: [
          getBytesEncoder().encode(
            new Uint8Array([
              117, 115, 101, 114, 95, 98, 117, 114, 110, 95, 97, 108, 108, 111, 119, 97, 110, 99, 101, // "user_burn_allowance"
            ])
          ),
          userAddressBytes,
          new Uint8Array([poolOwnerByte]),
        ],
      });

      // Ensure user burn allowance account exists; initialize if missing
      const ubaAccount = await rpc.getAccountInfo(userBurnAllowanceAddress, { commitment: 'confirmed', encoding: 'base64' }).send();
      let initializeUserBurnAllowanceInstruction: any | null = null;
      if (!ubaAccount.value) {
        initializeUserBurnAllowanceInstruction = await getInitializeUserBurnAllowanceInstructionAsync({
          payer: userSigner,
          owner: userSigner.address,
          userBurnAllowance: userBurnAllowanceAddress,
          poolOwner,
        });
      }

      const burnInstruction = await getBurnVirtualTokenInstructionAsync({
        signer: userSigner,
        pool: pool,
        userBurnAllowance: userBurnAllowanceAddress,
        centralState: centralStateAddress,
        poolOwner,
      });

      const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
      const transactionMessage = pipe(
        createTransactionMessage({ version: 0 }),
        (tx) => setTransactionMessageFeePayerSigner(userSigner, tx),
        (tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
        (tx) => (initializeUserBurnAllowanceInstruction ? appendTransactionMessageInstruction(initializeUserBurnAllowanceInstruction, tx) : tx),
        (tx) => appendTransactionMessageInstruction(burnInstruction, tx),
      );

      const signedTx = await signTransactionMessageWithSigners(transactionMessage);
      assertIsSendableTransaction(signedTx);

      try {
        const txBase64 = getBase64EncodedWireTransaction(signedTx);
        const simulateResult = await rpc
          .simulateTransaction(txBase64, { encoding: 'base64', sigVerify: true, commitment: 'confirmed' })
          .send();
        console.log('txBase64', txBase64);
        console.log('simulate burnVirtualToken', simulateResult);

        await sendAndConfirmTransaction(signedTx as any, { commitment: 'confirmed' });
        console.log('burnVirtualToken tx sent and confirmed');
      } catch (error) {
        console.error('burnVirtualToken tx error', error);
        throw error;
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['allPools'] });
      queryClient.invalidateQueries({ queryKey: ['userPool'] });
      queryClient.invalidateQueries({ queryKey: ['userBurnAllowance'] });
    },

  });
}

