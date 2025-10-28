import { KeyPairSigner, type Address } from "@solana/kit";
import { useSolBalance } from "../hooks/use-sol-balance";
import { useTokenBalance } from "../hooks/use-token-balance";
import { useMintToAccount } from "../hooks/use-mint-to-account";
import { useUserPool } from "../hooks/use-user-pool";
import { useState } from "react";
import { useCreatePool } from "../hooks/use-create-pool";
import { address } from "@solana/kit";

export default function AccountDetails({ selectedWallet }: { selectedWallet: KeyPairSigner }) {
  const walletAddress = selectedWallet?.address;
  const { data: balance, isLoading } = useSolBalance(walletAddress);
  const { data: tokenBalance, isLoading: isLoadingToken } = useTokenBalance(walletAddress);
  const { data: userPool, isLoading: isLoadingPool } = useUserPool(walletAddress);
  const { mutateAsync: mintTokens, isPending: isMinting } = useMintToAccount();
  const [mintAmount, setMintAmount] = useState('100000');
  const { mutateAsync: createPool, isPending: isCreatingPool } = useCreatePool();


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
          {walletAddress?.toString()}
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

      {isLoadingPool ? (
        <div style={{ marginBottom: '1rem' }}>
          <strong>Pool Info:</strong> Loading...
        </div>
      ) : userPool ? (
        <div style={{ 
          marginTop: '1.5rem',
          padding: '1rem',
          backgroundColor: '#f9f9f9',
          borderRadius: '8px',
          border: '1px solid #ddd',
        }}>
          <h3 style={{ marginTop: 0 }}>Pool Info</h3>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>Pool Address:</strong>
            <div style={{
              fontFamily: 'monospace',
              fontSize: '0.85rem',
              marginTop: '0.25rem',
              wordBreak: 'break-all',
            }}>
              {userPool.poolAddress.toString()}
            </div>
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>A Reserve:</strong> {(userPool.pool.aReserve / BigInt(10n ** 6n)).toString()}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>B Reserve:</strong> {(userPool.pool.bReserve / BigInt(10n ** 6n)).toString()}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>A Virtual Reserve:</strong> {(userPool.pool.aVirtualReserve / BigInt(10n ** 6n)).toString()}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>A Remaining Topup:</strong> {(userPool.pool.aRemainingTopup / BigInt(10n ** 6n)).toString()}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>Creator Fees Balance:</strong> {(userPool.pool.creatorFeesBalance / BigInt(10n ** 6n)).toString()}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>Buyback Fees Accumulated:</strong> {(userPool.pool.buybackFeesAccumulated / BigInt(10n ** 6n)).toString()}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>Creator Fee (bp):</strong> {userPool.pool.creatorFeeBasisPoints}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>Buyback Fee (bp):</strong> {userPool.pool.buybackFeeBasisPoints}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>Burns Today:</strong> {userPool.pool.burnsToday}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>B Mint Index:</strong> {userPool.pool.bMintIndex.toString()}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>B Mint Decimals:</strong> {userPool.pool.bMintDecimals}
          </div>
        </div>
      ) : (
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
    </div>
  );
}

