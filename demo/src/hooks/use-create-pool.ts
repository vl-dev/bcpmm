import { CPMM_POC_PROGRAM_ADDRESS, getCreatePoolInstructionAsync, fetchCentralState } from "@bcpmm/js-client";
import { Address, createSignerFromKeyPair, getBytesEncoder, getProgramDerivedAddress, KeyPairSigner } from "@solana/kit";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { getTxClient } from "src/solana/tx-client";
import { SYSTEM_PROGRAM_ADDRESS } from "@solana-program/system";
import { TOKEN_PROGRAM_ADDRESS } from "@solana-program/token";
import { createTransactionMessage } from "@solana/kit";

interface CreatePoolParams {
  user: KeyPairSigner;
  mint: Address;
  treasury: Address;
}

export async function useCreatePool() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ user, mint, treasury }: CreatePoolParams) => {
      const { rpc, sendAndConfirmTransaction } = await getTxClient();
      const userSigner = await createSignerFromKeyPair(user.keyPair);
      
      // Get central state address
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

      // Fetch central state to get b_mint_index
      const centralState = await fetchCentralState(rpc, centralStateAddress);
      const bMintIndex = centralState.data.bMintIndex;

      // Derive pool address
      const bMintIndexBytes = new Uint8Array(8);
      const dataView = new DataView(bMintIndexBytes.buffer);
      dataView.setBigUint64(0, bMintIndex, true); // little-endian u64
      
      const [poolAddress] = await getProgramDerivedAddress({
        programAddress: CPMM_POC_PROGRAM_ADDRESS,
        seeds: [
          getBytesEncoder().encode(new Uint8Array([98, 99, 112, 109, 109, 95, 112, 111, 111, 108])), // "bcpmm_pool"
          bMintIndexBytes,
        ],
      });

      const createPoolInstruction = await getCreatePoolInstructionAsync({
        payer: userSigner,
        aMint: mint,
        pool: poolAddress,
        treasury,
        centralState: centralStateAddress,
        tokenProgram: TOKEN_PROGRAM_ADDRESS,
        systemProgram: SYSTEM_PROGRAM_ADDRESS,
        aVirtualReserve: 1000000000000000000,
        creatorFeeBasisPoints: 1000,
        buybackFeeBasisPoints: 1000,
      });

      const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

    },
  });
}