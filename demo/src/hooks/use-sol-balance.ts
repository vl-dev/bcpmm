import { useQuery } from '@tanstack/react-query';
import { createSolanaRpc, type Address } from '@solana/kit';

export function useSolBalance(address: Address | null) {
  const rpc = createSolanaRpc('http://127.0.0.1:8899');

  return useQuery({
    queryKey: ['solBalance', address?.toString()],
    queryFn: async () => {
      if (!address) return null;
      
      const balanceResponse = await rpc.getBalance(address).send();
      return balanceResponse.value;
    },
    enabled: !!address,
    refetchInterval: 3000, // Refetch every 3 seconds
  });
}

