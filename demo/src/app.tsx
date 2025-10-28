import { useState, useEffect } from 'react';
import {
  airdropFactory,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  KeyPairSigner,
  lamports,
  type Address,
} from '@solana/kit';
import { useSolBalance } from './hooks/use-sol-balance';
import { useTokenBalance } from './hooks/use-token-balance';
import { useMintToAccount } from './hooks/use-mint-to-account';
import { getrLocalWallets } from './wallet-storage';
import { useAdminKeypair } from './hooks/use-admin-keypair';
import { ensureCentralState } from './ensure-central-state';
import { getAdminKeypair } from './admin-keypair';
import { ensureMint, ensureTreasury } from './ensure-mint';
const RPC_URL = import.meta.env.VITE_RPC_URL as string;
const WS_URL = import.meta.env.VITE_WS_URL as string;

function WalletDetails({ selectedWallet }: { selectedWallet: Address | null }) {
  const { data: balance, isLoading } = useSolBalance(selectedWallet);
  const { data: tokenBalance, isLoading: isLoadingToken } = useTokenBalance(selectedWallet);
  const { mutateAsync: mintTokens, isPending: isMinting } = useMintToAccount();
  const [mintAmount, setMintAmount] = useState('100000');

  if (!selectedWallet) {
    return (
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
    );
  }

  return (
    <div style={{
      backgroundColor: '#fff',
      border: '1px solid #ddd',
      padding: '1.5rem',
      borderRadius: '8px',
    }}>
      <h2 style={{ marginTop: 0 }}>Wallet Details</h2>
      
      <div style={{ marginBottom: '1rem' }}>
        <strong>Address:</strong>
        <div style={{
          fontFamily: 'monospace',
          backgroundColor: '#f5f5f5',
          padding: '0.5rem',
          borderRadius: '4px',
          marginTop: '0.25rem',
          wordBreak: 'break-all',
        }}>
          {selectedWallet.toString()}
        </div>
      </div>

      <div style={{ marginBottom: '1rem' }}>
        <strong>SOL Balance:</strong>
        <div style={{
          fontFamily: 'monospace',
          backgroundColor: '#f5f5f5',
          padding: '0.5rem',
          borderRadius: '4px',
          marginTop: '0.25rem',
        }}>
          {isLoading ? 'Loading...' : balance ? `${(Number(balance) / 1_000_000_000).toFixed(9)} SOL` : '0 SOL'}
        </div>
      </div>

      <div style={{ marginBottom: '1rem' }}>
        <strong>Quote Token Balance:</strong>
        <div style={{
          fontFamily: 'monospace',
          backgroundColor: '#f5f5f5',
          padding: '0.5rem',
          borderRadius: '4px',
          marginTop: '0.25rem',
        }}>
          {isLoadingToken ? 'Loading...' : tokenBalance ? `${tokenBalance}` : '0'}
        </div>
      </div>

      <div style={{ marginBottom: '1rem' }}>
        <strong>Mint Tokens:</strong>
        <div style={{
          display: 'flex',
          gap: '0.5rem',
          marginTop: '0.5rem',
          alignItems: 'center',
        }}>
          <input
            type="number"
            value={mintAmount}
            onChange={(e) => setMintAmount(e.target.value)}
            placeholder="Amount"
            style={{
              padding: '0.5rem',
              borderRadius: '4px',
              border: '1px solid #ddd',
              fontFamily: 'monospace',
              width: '120px',
            }}
          />
          <button
            onClick={async () => {
              if (!selectedWallet) return;
              try {
                await mintTokens({
                  user: selectedWallet,
                  amount: parseFloat(mintAmount),
                });
              } catch (error) {
                console.error('Error minting tokens:', error);
                alert('Failed to mint tokens');
              }
            }}
            disabled={isMinting || !selectedWallet}
            style={{
              padding: '0.5rem 1rem',
              backgroundColor: isMinting ? '#ccc' : '#4a90e2',
              color: 'white',
              border: 'none',
              borderRadius: '4px',
              cursor: isMinting ? 'not-allowed' : 'pointer',
              fontFamily: 'monospace',
            }}
          >
            {isMinting ? 'Minting...' : 'Mint'}
          </button>
        </div>
      </div>
    </div>
  );
}

function App() {
  const [wallets, setWallets] = useState<KeyPairSigner[]>([]);
  const [initializingAccounts, setInitializingAccounts] = useState(true);
  const [selectedWalletAddress, setSelectedWalletAddress] = useState<Address | null>(null);
  const { data: adminKeypair } = useAdminKeypair();

  useEffect(() => {
    async function initializeWallets() {
      setInitializingAccounts(true);
      
      // Create RPC and airdrop factory
      const rpc = createSolanaRpc(RPC_URL);
      const rpcSubscriptions = createSolanaRpcSubscriptions(WS_URL);
      const airdrop = airdropFactory({ rpc, rpcSubscriptions });

      // Get or create wallet addresses (persists across reloads)
      const wallets = await getrLocalWallets();
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
      const mint = await ensureMint(adminKp);
      await ensureTreasury(adminKp, mint)

      setInitializingAccounts(false);
    }

    initializeWallets();
  }, []);

  const formatAddress = (address: string) => {
    return `${address.slice(0, 3)}...${address.slice(-3)}`;
  };

  const isSelected = (address: Address) => {
    return selectedWalletAddress?.toString() === address.toString();
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
                onClick={() => setSelectedWalletAddress(wallet.address)}
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
        <WalletDetails selectedWallet={selectedWalletAddress} />

      </div>
    </div>
  );
}

export default App;
