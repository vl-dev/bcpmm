import { useQuery } from '@tanstack/react-query';
import { type Address, getAddressEncoder, getBytesEncoder, getProgramDerivedAddress } from '@solana/kit';
import { CBMM_PROGRAM_ADDRESS } from '@cbmm/js-client';
import { getUserBurnAllowanceDecoder } from '@cbmm/js-client';
import { Buffer } from 'buffer';
import { getTxClient } from '../solana/tx-client';

type BurnAllowanceInfo = {
  address: Address;
  exists: boolean;
  account?: ReturnType<typeof getUserBurnAllowanceDecoder> extends { decode: (b: Buffer) => infer T } ? T : any;
};

export function useUserBurnAllowance(user: Address | null) {
  return useQuery({
    queryKey: ['userBurnAllowance', user?.toString()],
    enabled: !!user,
    queryFn: async (): Promise<{ owner: BurnAllowanceInfo; nonOwner: BurnAllowanceInfo } | null> => {
      if (!user) return null;
      const { rpc } = await getTxClient();

      const ownerSeed = new Uint8Array([1]);
      const nonOwnerSeed = new Uint8Array([0]);
      const seedTag = getBytesEncoder().encode(new Uint8Array([117, 115, 101, 114, 95, 98, 117, 114, 110, 95, 97, 108, 108, 111, 119, 97, 110, 99, 101])); // "user_burn_allowance"
      const userBytes = getAddressEncoder().encode(user);

      const [ownerAddr] = await getProgramDerivedAddress({
        programAddress: CBMM_PROGRAM_ADDRESS,
        seeds: [seedTag, userBytes, ownerSeed],
      });
      const [nonOwnerAddr] = await getProgramDerivedAddress({
        programAddress: CBMM_PROGRAM_ADDRESS,
        seeds: [seedTag, userBytes, nonOwnerSeed],
      });

      const [ownerAcc, nonOwnerAcc] = await Promise.all([
        rpc.getAccountInfo(ownerAddr, { commitment: 'confirmed', encoding: 'base64' }).send(),
        rpc.getAccountInfo(nonOwnerAddr, { commitment: 'confirmed', encoding: 'base64' }).send(),
      ]);

      const decoder = getUserBurnAllowanceDecoder();
      const owner: BurnAllowanceInfo = ownerAcc.value
        ? { address: ownerAddr, exists: true, account: decoder.decode(Buffer.from(ownerAcc.value.data[0], 'base64')) }
        : { address: ownerAddr, exists: false };
      const nonOwner: BurnAllowanceInfo = nonOwnerAcc.value
        ? { address: nonOwnerAddr, exists: true, account: decoder.decode(Buffer.from(nonOwnerAcc.value.data[0], 'base64')) }
        : { address: nonOwnerAddr, exists: false };

      return { owner, nonOwner };
    },
  });
}


