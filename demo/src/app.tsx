import { useState, useEffect } from 'react';
import {
  KeyPairSigner,
} from '@solana/kit';
import { useAdminKeypair } from './hooks/use-admin-keypair';
import { ensureCentralState } from './ensure-central-state';
import { getAdminKeypair } from './admin-keypair';
import { ensureMint } from './ensure-mint';
import { ensureCentralStateAta } from './ensure-central-state-ata';
import AccountDetails from './components/account-details';
import PoolList from './components/pool-list';
import { WalletProvider, useWallet } from './wallet-provider';
import WalletTopBar from './components/wallet-top-bar';
import { useLocalWallets } from './hooks/use-local-wallets';

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
        <h1 style={{ marginBottom: '1.5rem' }}>CBMM Demo</h1>
        
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
  const [initializingAccounts, setInitializingAccounts] = useState(true);
  const { data: adminKeypair } = useAdminKeypair();
  const { data: localWallets } = useLocalWallets();
  useEffect(() => {
    async function initializeWallets() {
      setInitializingAccounts(true);
      
      const adminKp = await getAdminKeypair()
      await ensureCentralState();
      await ensureMint(adminKp);
      await ensureCentralStateAta(adminKp);

      setInitializingAccounts(false);
    }

    initializeWallets();
  }, [localWallets]);

  return (
    <WalletProvider>
      <AppContent 
        wallets={localWallets ?? []}
        initializingAccounts={initializingAccounts}
        adminAddress={adminKeypair?.address.toString()}
      />
    </WalletProvider>
  );
}

export default App;
