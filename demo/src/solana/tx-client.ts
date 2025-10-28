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
const RPC_URL = import.meta.env.VITE_RPC_URL as string | undefined;
const WS_URL = import.meta.env.VITE_WS_URL as string | undefined;
 
let client: TxClient | undefined;
export async function getTxClient(): Promise<TxClient> {
    if (!RPC_URL || !WS_URL) {
      throw new Error('VITE_RPC_URL or VITE_WS_URL is not set');
    }
    if (!client) {
        // ...
 
        // Create a function to send and confirm transactions.
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
