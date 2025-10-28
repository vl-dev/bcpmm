import { type Address } from "@solana/kit";
import { type BcpmmPool } from "@bcpmm/js-client";

type Props = {
  poolAddress: Address;
  pool: BcpmmPool;
  showOwner?: boolean;
};

const nf = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });

function formatBigintAmountWith6Decimals(value: bigint) {
  const base = 1_000_000n;
  const whole = value / base;
  return nf.format(Number(whole));
}

export default function PoolDetails({ poolAddress, pool, showOwner }: Props) {
  return (
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
  );
}


