import { useAllPools } from '../hooks/use-all-pools';
import PoolDetails from './pool-details';

export default function PoolList() {
  const { data: allPools, isLoading: isLoadingAllPools } = useAllPools();

  return (
    <div style={{
      backgroundColor: '#fff',
      border: '1px solid #ddd',
      padding: '1.5rem',
      borderRadius: '8px',
      marginTop: '1.5rem',
    }}>
      <h2 style={{ marginTop: 0 }}>All Pools</h2>
      
      {isLoadingAllPools ? (
        <div style={{
          fontFamily: 'monospace',
          backgroundColor: '#f5f5f5',
          padding: '0.5rem',
          borderRadius: '4px',
          marginTop: '0.25rem',
        }}>
          Loading pools...
        </div>
      ) : allPools && allPools.length > 0 ? (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
          {allPools.map((p) => (
            <PoolDetails
              key={p.poolAddress.toString()}
              poolAddress={p.poolAddress}
              pool={p.pool}
              showOwner
              allowBurn
              allowBuy
            />
          ))}
        </div>
      ) : (
        <div style={{
          fontFamily: 'monospace',
          backgroundColor: '#f5f5f5',
          padding: '0.5rem',
          borderRadius: '4px',
          marginTop: '0.25rem',
        }}>
          No pools found
        </div>
      )}
    </div>
  );
}
