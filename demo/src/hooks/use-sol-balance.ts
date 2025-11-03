import { useQuery } from '@tanstack/react-query';
import { createSolanaRpc, type Address } from '@solana/kit';
const RPC_URL = process.env.NEXT_PUBLIC_RPC_URL as string | undefined;
export function useSolBalance(address: Address | null) {
  if (!RPC_URL) throw new Error('NEXT_PUBLIC_RPC_URL is not set');
  const rpc = createSolanaRpc(RPC_URL);

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

