import { useQuery } from '@tanstack/react-query';
import { type Address, getAddressEncoder, getBytesEncoder } from '@solana/kit';
import { getTxClient } from '../solana/tx-client';
import { CPMM_POC_PROGRAM_ADDRESS, fetchMaybeVirtualTokenAccount } from '@bcpmm/js-client';
import { getProgramDerivedAddress } from '@solana/kit';

export function useVirtualTokenBalance(user: Address | null, poolAddress: Address | null) {
  return useQuery({
    queryKey: ['virtualTokenBalance', user?.toString(), poolAddress?.toString()],
    queryFn: async () => {
      const { rpc } = await getTxClient();
      if (!user || !poolAddress) return null;

      // Derive virtual token account address
      const [virtualTokenAccountAddress] = await getProgramDerivedAddress({
        programAddress: CPMM_POC_PROGRAM_ADDRESS,
        seeds: [
          getBytesEncoder().encode(
            new Uint8Array([
              118, 105, 114, 116, 117, 97, 108, 95, 116, 111, 107, 101, 110, 95,
              97, 99, 99, 111, 117, 110, 116, // "virtual_token_account"
            ])
          ),
          getAddressEncoder().encode(poolAddress),
          getAddressEncoder().encode(user),
        ],
      });

      try {
        const maybeAccount = await fetchMaybeVirtualTokenAccount(rpc, virtualTokenAccountAddress, { commitment: 'confirmed' });
        if (!maybeAccount.exists) return { balance: 0n, exists: false };
        return { balance: maybeAccount.data.balance, exists: true };
      } catch (error) {
        return { balance: 0n, exists: false };
      }
    },
    enabled: !!user && !!poolAddress,
  });
}

