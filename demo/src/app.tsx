import { useState, useEffect } from 'react';
import {
  airdropFactory,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  KeyPairSigner,
  lamports,
} from '@solana/kit';
import { getLocalWallets } from './wallet-storage';
import { useAdminKeypair } from './hooks/use-admin-keypair';
import { ensureCentralState } from './ensure-central-state';
import { getAdminKeypair } from './admin-keypair';
import { ensureMint } from './ensure-mint';
import { ensureCentralStateAta } from './ensure-central-state-ata';
import AccountDetails from './components/account-details';
import PoolList from './components/pool-list';
import { WalletProvider, useWallet } from './wallet-provider';
import WalletTopBar from './components/wallet-top-bar';

const RPC_URL = import.meta.env.VITE_RPC_URL as string;
const WS_URL = import.meta.env.VITE_WS_URL as string;

function AppContent({ wallets, initializingAccounts, adminAddress }: {
  wallets: KeyPairSigner[];
  initializingAccounts: boolean;
  adminAddress?: string;
}) {
  const selectedWallet = useWallet();
  return (
    <div style={{ 
      minHeight: '100vh',
      fontFamily: 'system-ui, -apple-system, sans-serif',
    }}>
      <WalletTopBar
        wallets={wallets}
        initializing={initializingAccounts}
        adminAddress={adminAddress}
      />

      {/* Main content */}
      <div style={{ 
        padding: '2rem',
        maxWidth: '1200px',
        margin: '0 auto'
      }}>
        <h1 style={{ marginBottom: '1.5rem' }}>BCPMM Demo</h1>
        
        {/* Wallet details section */}
        {selectedWallet && <AccountDetails selectedWallet={selectedWallet} />}
        {!selectedWallet && 
          <div style={{
            backgroundColor: '#fff',
            border: '2px dashed #ccc',
            padding: '1.5rem',
            borderRadius: '8px',
            textAlign: 'center',
            color: '#999',
          }}>
            No wallet selected
          </div>
        }

        <PoolList />
      </div>
    </div>
  );
}

function App() {
  const [wallets, setWallets] = useState<KeyPairSigner[]>([]);
  const [initializingAccounts, setInitializingAccounts] = useState(true);
  const { data: adminKeypair } = useAdminKeypair();
  useEffect(() => {
    async function initializeWallets() {
      setInitializingAccounts(true);
      
      // Create RPC and airdrop factory
      const rpc = createSolanaRpc(RPC_URL);
      const rpcSubscriptions = createSolanaRpcSubscriptions(WS_URL);
      const airdrop = airdropFactory({ rpc, rpcSubscriptions });

      // Get or create wallet addresses (persists across reloads)
      const wallets = await getLocalWallets();
      setWallets(wallets);

      // Check balances and airdrop only if below 0.5 SOL
      const minBalanceLamports = lamports(500_000_000n); // 0.5 SOL
      
      for (const address of wallets.map(wallet => wallet.address)) {
        const balanceResponse = await rpc.getBalance(address).send();
        const balance = balanceResponse.value;

        if (balance < minBalanceLamports) {
          console.log(`Airdropping to ${address.toString()} (balance: ${Number(balance) / 1_000_000_000} SOL)`);
          await airdrop({
            recipientAddress: address,
            lamports: lamports(1_000_000_000n),
            commitment: 'confirmed',
          });
        }
      }

      const adminKp = await getAdminKeypair()
      await ensureCentralState(adminKp);
      await ensureMint(adminKp);
      await ensureCentralStateAta(adminKp);

      setInitializingAccounts(false);
    }

    initializeWallets();
  }, []);

  return (
    <WalletProvider>
      <AppContent 
        wallets={wallets}
        initializingAccounts={initializingAccounts}
        adminAddress={adminKeypair?.address.toString()}
      />
    </WalletProvider>
  );
}

export default App;
