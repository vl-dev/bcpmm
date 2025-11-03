import {
    sendAndConfirmTransactionFactory, 
    createSolanaRpc,
    createSolanaRpcSubscriptions,
} from '@solana/kit';

export type TxClient = {
    sendAndConfirmTransaction: ReturnType<typeof sendAndConfirmTransactionFactory>; 
    rpc: ReturnType<typeof createSolanaRpc>;
    rpcSubscriptions: ReturnType<typeof createSolanaRpcSubscriptions>;
}
const RPC_URL = process.env.NEXT_PUBLIC_RPC_URL as string | undefined;
const WS_URL = process.env.NEXT_PUBLIC_WS_URL as string | undefined;
 
let client: TxClient | undefined;
export async function getTxClient(): Promise<TxClient> {
    if (!RPC_URL || !WS_URL) {
      throw new Error('NEXT_PUBLIC_RPC_URL or NEXT_PUBLIC_WS_URL is not set');
    }
    if (!client) {
        const sendAndConfirmTransaction = sendAndConfirmTransactionFactory({
            rpc: createSolanaRpc(RPC_URL),
            rpcSubscriptions: createSolanaRpcSubscriptions(WS_URL),
        });
        client = {
            sendAndConfirmTransaction,
            rpc: createSolanaRpc(RPC_URL),
            rpcSubscriptions: createSolanaRpcSubscriptions(WS_URL),
        };
    }
    return client;

}
