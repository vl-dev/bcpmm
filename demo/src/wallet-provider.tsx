import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from 'react';
import type { KeyPairSigner } from '@solana/kit';

type WalletContextValue = {
  wallet: KeyPairSigner | null;
  setWallet: (value: KeyPairSigner | null) => void;
};

const WalletContext = createContext<WalletContextValue | undefined>(undefined);

type WalletProviderProps = {
  children: ReactNode;
  initialWallet?: KeyPairSigner | null;
};

export function WalletProvider({ children, initialWallet = null }: WalletProviderProps) {
  const [wallet, setWalletState] = useState<KeyPairSigner | null>(initialWallet);

  const setWallet = useCallback((value: KeyPairSigner | null) => {
    setWalletState(value);
  }, []);

  const value = useMemo<WalletContextValue>(() => ({ wallet, setWallet }), [wallet, setWallet]);
  return (
    <WalletContext.Provider value={value}>
      {children}
    </WalletContext.Provider>
  );
}

export function useWallet() {
  const ctx = useContext(WalletContext);
  if (!ctx) throw new Error('useWallet must be used within a WalletProvider');
  return ctx.wallet;
}

export function useSetWallet() {
  const ctx = useContext(WalletContext);
  if (!ctx) throw new Error('useSetWallet must be used within a WalletProvider');
  return ctx.setWallet;
}

export function useWalletStore() {
  const ctx = useContext(WalletContext);
  if (!ctx) throw new Error('useWalletStore must be used within a WalletProvider');
  return ctx;
}


