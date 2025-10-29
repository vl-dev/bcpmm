import type { KeyPairSigner } from '@solana/kit';
import { useWallet, useSetWallet } from '../wallet-provider';

type Props = {
  wallets: KeyPairSigner[];
  initializing: boolean;
  adminAddress?: string;
};

const WalletTopBar = ({ wallets, initializing, adminAddress }: Props) => {
  const wallet = useWallet();
  const setWallet = useSetWallet();

  const formatAddress = (address: string) => `${address.slice(0, 3)}...${address.slice(-3)}`;
  const isSelected = (address: string) => wallet?.address.toString() === address;

  return (
    <div style={{ 
      backgroundColor: '#1a1a1a',
      color: 'white',
      padding: '1rem 2rem',
      display: 'flex',
      alignItems: 'center',
      gap: '1.5rem',
      flexWrap: 'wrap',
      position: 'relative',
      zIndex: 1,
    }}>
      <div style={{ fontWeight: 'bold', fontSize: '1.1rem' }}>Wallets:</div>
      {initializing ? (
        <span style={{ color: '#ffd700' }}>Initializing accounts...</span>
      ) : (
        wallets.map((w) => {
          const addressStr = w.address.toString();
          const selected = isSelected(addressStr);
          return (
            <button
              type="button"
              key={addressStr}
              onClick={() => {
                setWallet(w);
              }}
              style={{ 
                padding: '0.25rem 0.75rem',
                backgroundColor: selected ? '#4a90e2' : '#2a2a2a',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                fontFamily: 'monospace',
                cursor: 'pointer',
                transition: 'background-color 0.2s',
                pointerEvents: 'auto',
              }}
            >
              {formatAddress(addressStr)}
              {w.address.toString() === adminAddress && <span style={{ color: '#ffd700' }}> (admin)</span>}
            </button>
          );
        })
      )}
    </div>
  );
};

export default WalletTopBar;


