import { useQuery } from '@tanstack/react-query';
import { type Address, address } from '@solana/kit';
import { getTxClient } from '../solana/tx-client';
import { findAssociatedTokenPda, TOKEN_PROGRAM_ADDRESS } from '@solana-program/token';

export function useTokenBalance(user: Address | null) {
  const mintAddress = localStorage.getItem('mint_address');

  return useQuery({
    queryKey: ['tokenBalance', user?.toString(), mintAddress],
    queryFn: async () => {
      const { rpc } = await getTxClient();
      if (!user || !mintAddress) return null;
      const [associatedTokenAddress] = await findAssociatedTokenPda({
        mint: address(mintAddress),
        owner: user,
        tokenProgram: TOKEN_PROGRAM_ADDRESS,
      });

      const balance = await rpc.getTokenAccountBalance(associatedTokenAddress).send();
      return balance.value.uiAmountString;
    },
    enabled: !!user && !!mintAddress,
  });
}

