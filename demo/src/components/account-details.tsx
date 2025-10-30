import { address, KeyPairSigner } from "@solana/kit";
import { useSolBalance } from "../hooks/use-sol-balance";
import { useTokenBalance } from "../hooks/use-token-balance";
import { useMintToAccount } from "../hooks/use-mint-to-account";
import { useUserPool } from "../hooks/use-user-pool";
import { useState } from "react";
import { useUserBurnAllowance } from "../hooks/use-user-burn-allowance";
import { useCreatePool } from "../hooks/use-create-pool";
import { useDeleteWallet } from "../wallet-provider";

export default function AccountDetails({ selectedWallet }: { selectedWallet: KeyPairSigner }) {
  const walletAddress = selectedWallet?.address;
  const { data: balance, isLoading } = useSolBalance(walletAddress);
  const { data: tokenBalance, isLoading: isLoadingToken } = useTokenBalance(walletAddress);
  const { data: userPool, isLoading: isLoadingPool } = useUserPool(walletAddress);
  const { mutateAsync: mintTokens, isPending: isMinting } = useMintToAccount();
  const [mintAmount, setMintAmount] = useState('100000');
  const { mutateAsync: createPool, isPending: isCreatingPool } = useCreatePool();
  const { data: burnAllowances } = useUserBurnAllowance(walletAddress);
  const deleteWallet = useDeleteWallet();
  const [showDeleteDialog, setShowDeleteDialog] = useState(false);

  const formatTimestamp = (v?: bigint) => {
    if (!v) return '—';
    const n = Number(v);
    if (!Number.isFinite(n) || n <= 0) return '—';
    const ms = n < 1_000_000_000_000 ? n * 1000 : n;
    return new Date(ms).toLocaleString();
  };

  return (
    <div style={{
      backgroundColor: '#fff',
      border: '1px solid #ddd',
      padding: '1.5rem',
      borderRadius: '8px',
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', marginBottom: '1rem' }}>
        <h2 style={{ marginTop: 0, marginBottom: 0 }}>Selected Wallet</h2>
        <button
          onClick={() => setShowDeleteDialog(true)}
          style={{
            backgroundColor: '#dc3545',
            color: 'white',
            border: 'none',
            padding: '0.25rem 0.5rem',
            borderRadius: '4px',
            cursor: 'pointer',
            fontSize: '0.875rem',
          }}
        >
          Delete
        </button>
      </div>
      
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
          {walletAddress?.toString()}
        </div>
      </div>

      <div style={{ marginBottom: '1rem' }}>
        <strong>Pool Address:</strong>
        <div style={{
          fontFamily: 'monospace',
          backgroundColor: '#f5f5f5',
          padding: '0.5rem',
          borderRadius: '4px',
          marginTop: '0.25rem',
          wordBreak: 'break-all',
        }}>
          {isLoadingPool ? 'Loading...' : userPool ? userPool.poolAddress.toString() : 'No pool found'}
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
        <strong>Burn Allowances:</strong>
        <div style={{
          display: 'grid',
          gridTemplateColumns: '1fr 1fr',
          gap: '0.5rem',
          marginTop: '0.5rem',
        }}>
          <div style={{ backgroundColor: '#f5f5f5', padding: '0.5rem', borderRadius: '4px' }}>
            <div style={{ fontWeight: 600 }}>For own pool</div>
            <div style={{ fontFamily: 'monospace', wordBreak: 'break-all' }}>
              {burnAllowances ? burnAllowances.owner.address.toString() : '...'}
            </div>
            <div style={{ color: '#666', fontSize: '0.85rem' }}>
              {burnAllowances ? (burnAllowances.owner.exists ? 'Initialized' : 'Not initialized') : ''}
            </div>
          {burnAllowances?.owner.exists && burnAllowances.owner.account && (
            <div style={{ marginTop: '0.25rem', fontSize: '0.9rem' }}>
              <div>Burns today: <span style={{ fontFamily: 'monospace' }}>{burnAllowances.owner.account?.burnsToday}</span></div>
              <div>Last burn: <span style={{ fontFamily: 'monospace' }}>{formatTimestamp(burnAllowances.owner.account?.lastBurnTimestamp)}</span></div>
            </div>
          )}
          </div>
          <div style={{ backgroundColor: '#f5f5f5', padding: '0.5rem', borderRadius: '4px' }}>
            <div style={{ fontWeight: 600 }}>For other pools</div>
            <div style={{ fontFamily: 'monospace', wordBreak: 'break-all' }}>
              {burnAllowances ? burnAllowances.nonOwner.address.toString() : '...'}
            </div>
            <div style={{ color: '#666', fontSize: '0.85rem' }}>
              {burnAllowances ? (burnAllowances.nonOwner.exists ? 'Initialized' : 'Not initialized') : ''}
            </div>
          {burnAllowances?.nonOwner.exists && burnAllowances.nonOwner.account && (
            <div style={{ marginTop: '0.25rem', fontSize: '0.9rem' }}>
              <div>Burns today: <span style={{ fontFamily: 'monospace' }}>{burnAllowances.nonOwner.account?.burnsToday}</span></div>
              <div>Last burn: <span style={{ fontFamily: 'monospace' }}>{formatTimestamp(burnAllowances.nonOwner.account?.lastBurnTimestamp)}</span></div>
            </div>
          )}
          </div>
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
                  user: selectedWallet.address,
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

      {!isLoadingPool && !userPool && (
        <div style={{ 
          marginTop: '1.5rem',
          padding: '1rem',
          backgroundColor: '#fff3cd',
          borderRadius: '8px',
          border: '1px solid #ffc107',
        }}>
          <strong>Pool Info:</strong> No pool found for this wallet
          <div style={{ marginTop: '0.75rem' }}>
            <button
              onClick={async () => {
                try {
                  const mintAddress = localStorage.getItem('mint_address');
                  if (!mintAddress) throw new Error('mint_address missing');
                  if (!selectedWallet) throw new Error('selected wallet not ready');
                  await createPool({ user: selectedWallet, mint: address(mintAddress) });
                } catch (e) {
                  console.error('create pool failed', e);
                  alert('Failed to create pool');
                }
              }}
              disabled={isCreatingPool}
              style={{
                padding: '0.5rem 1rem',
                backgroundColor: isCreatingPool ? '#ccc' : '#28a745',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: isCreatingPool ? 'not-allowed' : 'pointer',
                fontFamily: 'monospace',
              }}
            >
              {isCreatingPool ? 'Creating...' : 'Create Pool'}
            </button>
          </div>
        </div>
      )}

      {showDeleteDialog && (
        <div style={{
          position: 'fixed',
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          backgroundColor: 'rgba(0, 0, 0, 0.5)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          zIndex: 1000,
        }}>
          <div style={{
            backgroundColor: 'white',
            padding: '2rem',
            borderRadius: '8px',
            maxWidth: '400px',
            width: '90%',
            textAlign: 'center',
          }}>
            <h3 style={{ marginTop: 0, color: '#dc3545' }}>Delete Wallet</h3>
            <p>Are you sure you want to delete this wallet? This action cannot be undone.</p>
            <div style={{ display: 'flex', gap: '0.5rem', justifyContent: 'center', marginTop: '1rem' }}>
              <button
                onClick={() => setShowDeleteDialog(false)}
                style={{
                  padding: '0.5rem 1rem',
                  backgroundColor: '#6c757d',
                  color: 'white',
                  border: 'none',
                  borderRadius: '4px',
                  cursor: 'pointer',
                }}
              >
                Cancel
              </button>
              <button
                onClick={async () => {
                  await deleteWallet(selectedWallet);
                  setShowDeleteDialog(false);
                }}
                style={{
                  padding: '0.5rem 1rem',
                  backgroundColor: '#dc3545',
                  color: 'white',
                  border: 'none',
                  borderRadius: '4px',
                  cursor: 'pointer',
                }}
              >
                Delete
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

