import { useQuery } from '@tanstack/react-query';
import { Base64EncodedBytes, getBase64Encoder, type Address } from '@solana/kit';
import { getTxClient } from '../solana/tx-client';
import { CPMM_POC_PROGRAM_ADDRESS } from '@bcpmm/js-client';
import { getBcpmmPoolDecoder, BCPMM_POOL_DISCRIMINATOR, type BcpmmPool } from '@bcpmm/js-client';
import { Buffer } from 'buffer';

export function useAllPools() {
  return useQuery({
    queryKey: ['allPools'],
    queryFn: async (): Promise<Array<{ poolAddress: Address; pool: BcpmmPool }>> => {
      try {
      const { rpc } = await getTxClient();
      const discriminatorBase64 = Buffer.from(BCPMM_POOL_DISCRIMINATOR).toString('base64');
      const accounts = await rpc.getProgramAccounts(
        CPMM_POC_PROGRAM_ADDRESS,
        {
          commitment: 'confirmed',
          encoding: 'base64',
          filters: [
            {
              memcmp: {
                bytes: discriminatorBase64 as unknown as Base64EncodedBytes,
                encoding: 'base64',
                offset: 0n,
              },
            },
          ],
        },
      ).send();

      const decoder = getBcpmmPoolDecoder();
      return accounts.map((acc) => ({
        poolAddress: acc.pubkey,
        pool: decoder.decode(Buffer.from(acc.account.data[0], 'base64')),
      }));
    } catch (error) {
      console.error('Error fetching all pools:', error);
      return [];
    }
    },
  });
}


