import { useQuery } from '@tanstack/react-query';
import { type Address, getAddressEncoder, getBytesEncoder } from '@solana/kit';
import { getTxClient } from '../solana/tx-client';
import { CPMM_POC_PROGRAM_ADDRESS, type BcpmmPool, fetchBcpmmPool, getBcpmmPoolDecoder } from '@bcpmm/js-client';
import { getProgramDerivedAddress } from '@solana/kit';
import { Buffer } from 'buffer';

export function useUserPool(user: Address | null) {
  const mintAddress = localStorage.getItem('mint_address');

  return useQuery({
    queryKey: ['userPool', user?.toString(), mintAddress],
    queryFn: async (): Promise<{ pool: BcpmmPool, poolAddress: Address } | null> => {
      console.log('useUserPool: starting query');
      const { rpc } = await getTxClient();
      console.log('useUserPool: got rpc client');
      if (!user || !mintAddress) return null;

      const [poolAddress] = await getProgramDerivedAddress({
        programAddress: CPMM_POC_PROGRAM_ADDRESS,
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

