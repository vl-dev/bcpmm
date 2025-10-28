import { useState, useEffect } from 'react';
import {
  airdropFactory,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  KeyPairSigner,
  lamports,
  type Address,
} from '@solana/kit';
import { getLocalWallets } from './wallet-storage';
import { useAdminKeypair } from './hooks/use-admin-keypair';
import { ensureCentralState } from './ensure-central-state';
import { getAdminKeypair } from './admin-keypair';
import { ensureMint } from './ensure-mint';
import { ensureCentralStateAta } from './ensure-central-state-ata';
import AccountDetails from './components/account-details';
import PoolList from './components/pool-list';

const RPC_URL = import.meta.env.VITE_RPC_URL as string;
const WS_URL = import.meta.env.VITE_WS_URL as string;

function App() {
  const [wallets, setWallets] = useState<KeyPairSigner[]>([]);
  const [initializingAccounts, setInitializingAccounts] = useState(true);
  const [selectedWallet, setSelectedWallet] = useState<KeyPairSigner | null>(null);
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

  const formatAddress = (address: string) => {
    return `${address.slice(0, 3)}...${address.slice(-3)}`;
  };

  const isSelected = (address: Address) => {
    return selectedWallet?.address.toString() === address.toString();
  };

  return (
    <div style={{ 
      minHeight: '100vh',
      fontFamily: 'system-ui, -apple-system, sans-serif',
    }}>
      {/* Top bar with wallets */}
      <div style={{ 
        backgroundColor: '#1a1a1a',
        color: 'white',
        padding: '1rem 2rem',
        display: 'flex',
        alignItems: 'center',
        gap: '1.5rem',
        flexWrap: 'wrap',
      }}>
        <div style={{ fontWeight: 'bold', fontSize: '1.1rem' }}>Wallets:</div>
        {initializingAccounts ? (
          <span style={{ color: '#ffd700' }}>Initializing accounts...</span>
        ) : (
          wallets.map((wallet, idx) => {
            const selected = isSelected(wallet.address);
            return (
              <button
                key={idx}
                onClick={() => setSelectedWallet(wallet)}
                style={{ 
                  padding: '0.25rem 0.75rem',
                  backgroundColor: selected ? '#4a90e2' : '#2a2a2a',
                  color: 'white',
                  border: 'none',
                  borderRadius: '4px',
                  fontFamily: 'monospace',
                  cursor: 'pointer',
                  transition: 'background-color 0.2s',
                }}
                onMouseEnter={(e) => {
                  if (!selected) {
                    e.currentTarget.style.backgroundColor = '#3a3a3a';
                  }
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.backgroundColor = selected ? '#4a90e2' : '#2a2a2a';
                }}
              >
                {formatAddress(wallet.address.toString())}
                {wallet.toString() === adminKeypair?.address.toString() && <span style={{ color: '#ffd700' }}> (admin)</span>}
              </button>
            );
          })
        )}
      </div>

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

export default App;
