import { useQuery } from '@tanstack/react-query';
import { type Address, getAddressEncoder, getBytesEncoder } from '@solana/kit';
import { getTxClient } from '../solana/tx-client';
import { CBMM_PROGRAM_ADDRESS, type BcpmmPool, getBcpmmPoolDecoder } from '@cbmm/js-client';
import { getProgramDerivedAddress } from '@solana/kit';
import { Buffer } from 'buffer';
import { QUOTE_MINT_ADDRESS } from '../constants';

export function useUserPool(user: Address | null) {
  const mintAddress = QUOTE_MINT_ADDRESS;

  return useQuery({
    queryKey: ['userPool', user?.toString(), mintAddress],
    queryFn: async (): Promise<{ pool: BcpmmPool, poolAddress: Address } | null> => {
      if (!user || !mintAddress) return null;
      const { rpc } = await getTxClient();

      const [poolAddress] = await getProgramDerivedAddress({
        programAddress: CBMM_PROGRAM_ADDRESS,
        seeds: [
          getBytesEncoder().encode(new Uint8Array([98, 99, 112, 109, 109, 95, 112, 111, 111, 108])), // "bcpmm_pool"
          getBytesEncoder().encode(new Uint8Array([0, 0, 0, 0])),
          getAddressEncoder().encode(user),
        ],
      });

      // Get account info
      const account = await rpc.getAccountInfo(poolAddress, { commitment: 'confirmed', encoding: 'base64' }).send();
      if (!account.value) return null;

      const pool = getBcpmmPoolDecoder().decode(Buffer.from(account.value.data[0], 'base64'));
      return { pool, poolAddress };
    },
    enabled: !!user && !!mintAddress,
  });
}

