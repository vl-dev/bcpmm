import { type Address } from "@solana/kit";
import { type BcpmmPool } from "@cbmm/js-client";
import { useBurnTokens } from "../hooks/use-burn-tokens";
import { useWallet } from "../wallet-provider";
import { useState, useMemo } from "react";
import { useBuyVirtualToken } from "../hooks/use-buy-virtual-token";
import { useSellVirtualToken } from "../hooks/use-sell-virtual-token";
import { useVirtualTokenBalance } from "../hooks/use-virtual-token-balance";
import { useTokenBalance } from "../hooks/use-token-balance";
import { calculateVirtualTokenPrice, calculateBuyOutput } from "../virtual-token-price";


type PoolDetailsProps = {
  poolAddress: Address;
  pool: BcpmmPool;
  showOwner?: boolean;
  allowBurn?: boolean;
  allowBuy?: boolean;
};

const nf = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });

function formatBigintAmountWith6Decimals(value: bigint) {
  const base = 1_000_000n;
  const whole = value / base;
  return nf.format(Number(whole));
}

export default function PoolDetails({ poolAddress, pool, showOwner, allowBurn, allowBuy: allowBuy }: PoolDetailsProps) {
  const { mutateAsync: burnTokens, isPending: isBurning } = useBurnTokens();
  const { mutateAsync: buyVirtualToken, isPending: isBuying } = useBuyVirtualToken();
  const { mutateAsync: sellVirtualToken, isPending: isSelling } = useSellVirtualToken();
  const [buyAmount, setBuyAmount] = useState<string>("");
  const [sellAmount, setSellAmount] = useState<string>("");
  const user = useWallet();
  const { data: virtualTokenBalance } = useVirtualTokenBalance(user?.address || null, poolAddress);
  const { data: tokenBalance } = useTokenBalance(user?.address || null);
  const isOwner = user?.address?.toString() === pool.creator.toString();

  const parsedBuyAmount = (() => {
    const n = Number(buyAmount);
    if (!isFinite(n) || n <= 0) return 0;
    return Math.floor(n * 1_000_000); // assume 6 decimals for Mint A
  })();

  const parsedSellAmount = (() => {
    const n = Number(sellAmount);
    if (!isFinite(n) || n <= 0) return 0;
    const factor = Math.pow(10, pool.bMintDecimals);
    return Math.floor(n * factor);
  })();

  // Convert token balance string to base units (assuming 6 decimals for Token A)
  const availableTokenA = (() => {
    if (!tokenBalance) return 0n;
    const num = parseFloat(tokenBalance);
    if (!isFinite(num) || num <= 0) return 0n;
    return BigInt(Math.floor(num * 1_000_000));
  })();

  // Get available virtual token B balance
  const availableTokenB = virtualTokenBalance?.balance || 0n;

  const hasEnoughTokenA = BigInt(parsedBuyAmount) <= availableTokenA;
  const hasEnoughTokenB = BigInt(parsedSellAmount) <= availableTokenB;

  const virtualTokenPrice = useMemo(() => {
    return calculateVirtualTokenPrice(pool, pool.bMintDecimals);
  }, [pool]);

  const estimatedBuyOutput = useMemo(() => {
    if (parsedBuyAmount <= 0) return 0;
    return calculateBuyOutput(BigInt(parsedBuyAmount), pool, pool.bMintDecimals);
  }, [parsedBuyAmount, pool]);

  return (
    <div style={{ 
      marginTop: '1.5rem',
      padding: '1rem',
      backgroundColor: '#f9f9f9',
      borderRadius: '8px',
      border: '1px solid #ddd',
    }}>
      <div style={{ display: 'flex', flexDirection: 'row', gap: '1rem', alignItems: 'flex-start' }}>
        <div style={{ width: '70%' }}>
          <h3 style={{ marginTop: 0, color: isOwner ? 'green' : '#000' }}>{isOwner ? 'Your Pool' : 'Pool Info'}</h3>
          <div style={{ marginBottom: '0.5rem' }}>
        <strong>Pool Address:</strong>
        <div style={{
          fontFamily: 'monospace',
          fontSize: '0.85rem',
          marginTop: '0.25rem',
          wordBreak: 'break-all',
        }}>
          {poolAddress.toString()}
        </div>
          </div>
          {showOwner && (
            <div style={{ marginBottom: '0.5rem' }}>
              <strong>Owner:</strong>
              <div style={{
                fontFamily: 'monospace',
                fontSize: '0.85rem',
                marginTop: '0.25rem',
                wordBreak: 'break-all',
              }}>
                {pool.creator.toString()}
              </div>
            </div>
          )}
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>A Reserve:</strong> {formatBigintAmountWith6Decimals(pool.aReserve)}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>B Reserve:</strong> {formatBigintAmountWith6Decimals(pool.bReserve)}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>Virtual Token Price:</strong> {virtualTokenPrice > 0 ? `${virtualTokenPrice.toFixed(6)} A per B` : 'N/A'}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>A Virtual Reserve:</strong> {formatBigintAmountWith6Decimals(pool.aVirtualReserve)}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>A Remaining Topup:</strong> {formatBigintAmountWith6Decimals(pool.aRemainingTopup)}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>Creator Fees Balance:</strong> {formatBigintAmountWith6Decimals(pool.creatorFeesBalance)}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>Buyback Fees Accumulated:</strong> {formatBigintAmountWith6Decimals(pool.buybackFeesAccumulated)}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>Creator Fee (bp):</strong> {nf.format(pool.creatorFeeBasisPoints)}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>Buyback Fee (bp):</strong> {nf.format(pool.buybackFeeBasisPoints)}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>Burns Today:</strong> {nf.format(pool.burnsToday)}
          </div>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>B Mint Decimals:</strong> {nf.format(pool.bMintDecimals)}
          </div>
        </div>
        <div style={{ width: '30%', display: 'flex', gap: '1rem', alignItems: 'center', flexDirection: 'column' }}>
          {allowBurn && (
            <div style={{ width: '100%' }}>
              <button
                type="button"
                disabled={!user || isBurning}
                onClick={async () => {
                  if (!user) return;
                  await burnTokens({
                    user,
                    pool: poolAddress,
                    poolOwner: user.address.toString() === pool.creator.toString(),
                  });
                }}
                style={{
                  padding: '0.4rem 0.75rem',
                  width: '100%',
                  display: 'flex',
                  justifyContent: 'center',
                  alignItems: 'center',
                  backgroundColor: isBurning ? '#ccc' : '#d68f89',
                  border: 'none',
                  borderRadius: '4px',
                  cursor: !user || isBurning ? 'not-allowed' : 'pointer',
                  fontFamily: 'monospace',
                  gap: '0.35rem',
                }}
              >
                <span role="img" aria-label="fire">🔥</span>
                {isBurning ? 'Burning...' : 'Burn'}
              </button>
            </div>
          )}
          {allowBuy && (
            <div style={{ width: '100%', display: 'flex', flexDirection: 'column', gap: '0.5rem', marginTop: '1rem' }}>
              <div style={{ display: 'flex', gap: '0.5rem' }}>
                <input
                  type="number"
                  placeholder="Amount A"
                  value={buyAmount}
                  onChange={(e) => setBuyAmount(e.target.value)}
                  min={0}
                  step={0.000001}
                  style={{ flex: 1, padding: '0.4rem 0.5rem', border: '1px solid #ccc', borderRadius: '4px', fontFamily: 'monospace' }}
                />
                <button
                  type="button"
                  disabled={!user || isBuying || parsedBuyAmount <= 0 || !hasEnoughTokenA}
                  onClick={async () => {
                    if (!user) return;
                    const aAmount = BigInt(parsedBuyAmount);
                    await buyVirtualToken({
                      user,
                      pool: poolAddress,
                      aMint: pool.aMint,
                      aAmount,
                      bAmountMin: 0n,
                    });
                    setBuyAmount("");
                  }}
                  style={{
                    padding: '0.4rem 0.75rem',
                    minWidth: '96px',
                    display: 'flex',
                    justifyContent: 'center',
                    alignItems: 'center',
                    backgroundColor: (!user || isBuying || parsedBuyAmount <= 0 || !hasEnoughTokenA) ? '#ccc' : '#8fd689',
                    border: 'none',
                    borderRadius: '4px',
                    cursor: (!user || isBuying || parsedBuyAmount <= 0 || !hasEnoughTokenA) ? 'not-allowed' : 'pointer',
                    fontFamily: 'monospace',
                    gap: '0.35rem',
                  }}
                >
                  {isBuying ? 'Buying...' : 'Buy'}
                </button>
              </div>
              {parsedBuyAmount > 0 && (
                <div style={{
                  fontSize: '0.75rem',
                  color: '#666',
                  fontFamily: 'monospace',
                  textAlign: 'left',
                  marginTop: '-0.25rem',
                }}>
                  ≈ {estimatedBuyOutput.toFixed(6)} token B
                </div>
              )}
              {virtualTokenBalance && (
                <div style={{
                  fontSize: '0.85rem',
                  color: '#666',
                  padding: '0.25rem 0.5rem',
                  backgroundColor: '#f0f0f0',
                  borderRadius: '4px',
                  fontFamily: 'monospace',
                  textAlign: 'center',
                  marginTop: '1rem',
                }}>
                  Balance: {(() => {
                    const decimals = pool.bMintDecimals;
                    const divisor = BigInt(10 ** decimals);
                    const whole = virtualTokenBalance.balance / divisor;
                    const remainder = virtualTokenBalance.balance % divisor;
                    return remainder === 0n
                      ? whole.toString()
                      : `${whole}.${remainder.toString().padStart(decimals, '0')}`;
                  })()} token B
                </div>
              )}
              <div style={{ width: '100%', display: 'flex', flexDirection: 'column', gap: '0.5rem', marginTop: '0.5rem' }}>
                <div style={{ display: 'flex', gap: '0.5rem' }}>
                  <input
                    type="number"
                    placeholder={`Amount B (decimals: ${pool.bMintDecimals})`}
                    value={sellAmount}
                    onChange={(e) => setSellAmount(e.target.value)}
                    min={0}
                    step={Math.pow(10, -Math.min(6, pool.bMintDecimals))}
                    style={{ flex: 1, padding: '0.4rem 0.5rem', border: '1px solid #ccc', borderRadius: '4px', fontFamily: 'monospace' }}
                  />
                  <button
                    type="button"
                    disabled={!user || isSelling || parsedSellAmount <= 0 || !hasEnoughTokenB}
                    onClick={async () => {
                      if (!user) return;
                      const bAmount = BigInt(parsedSellAmount);
                      await sellVirtualToken({
                        user,
                        pool: poolAddress,
                        aMint: pool.aMint,
                        bAmount,
                      });
                      setSellAmount("");
                    }}
                    style={{
                      padding: '0.4rem 0.75rem',
                      minWidth: '96px',
                      display: 'flex',
                      justifyContent: 'center',
                      alignItems: 'center',
                      backgroundColor: (!user || isSelling || parsedSellAmount <= 0 || !hasEnoughTokenB) ? '#ccc' : '#89a9d6',
                      border: 'none',
                      borderRadius: '4px',
                      cursor: (!user || isSelling || parsedSellAmount <= 0 || !hasEnoughTokenB) ? 'not-allowed' : 'pointer',
                      fontFamily: 'monospace',
                      gap: '0.35rem',
                    }}
                  >
                    {isSelling ? 'Selling...' : 'Sell'}
                  </button>
                </div>
              </div>
            </div>
          )}
          {/* future buttons go here */}
        </div>
      </div>

    </div>
  );
}


