import { useQuery } from '@tanstack/react-query';
import { type Address, address, Base58EncodedBytes, getBytesDecoder, getBytesEncoder } from '@solana/kit';
import { getTxClient } from '../solana/tx-client';
import { CPMM_POC_PROGRAM_ADDRESS, type BcpmmPool, getBcpmmPoolDecoder, getBcpmmPoolSize } from '@bcpmm/js-client';
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

      let resp;
      try {
        console.log('useUserPool: calling getProgramAccounts...');
        console.log('useUserPool: getBcpmmPoolSize() =', getBcpmmPoolSize());
        // Get all pools and iterate to find one where creator matches user
        resp = await rpc
          .getProgramAccounts(CPMM_POC_PROGRAM_ADDRESS, {
            withContext: true,
            commitment: 'confirmed',
            encoding: "base64",
            filters: [
              { dataSize: BigInt(getBcpmmPoolSize()) },
              { memcmp: { offset: BigInt(8 + 1), bytes: user as unknown as Base58EncodedBytes, encoding: 'base58' } },
            ],
          })
          .send();

        console.log('useUserPool: got response, length:', resp.value.length);
        if (!resp.value.length) return null;
      } catch (e) {
        console.error('useUserPool: error in getProgramAccounts', e);
        throw e;
      }

      if (resp.value.length === 0) return null;

      const pool = getBcpmmPoolDecoder().decode(Buffer.from(resp.value[0].account.data[0] as string, 'base64'));
      return { pool, poolAddress: address(resp.value[0].pubkey) };
    },
    enabled: !!user && !!mintAddress,
  });
}

