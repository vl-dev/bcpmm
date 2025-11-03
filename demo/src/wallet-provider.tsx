'use client';

import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from 'react';
import type { KeyPairSigner } from '@solana/kit';
import { createKeyPairSignerFromPrivateKeyBytes } from '@solana/kit';
import { utils } from '@noble/ed25519';
import { hashes } from '@noble/ed25519';
import { sha512 } from '@noble/hashes/sha512';
import { useQueryClient } from '@tanstack/react-query';
hashes.sha512 = sha512;

type WalletContextValue = {
  wallet: KeyPairSigner | null;
  setWallet: (value: KeyPairSigner | null) => void;
  createKeypair: () => Promise<KeyPairSigner>;
  deleteWallet: (walletToDelete: KeyPairSigner) => void;
};

type LocalStorageKeypairs = {
  wallets: number[][]
}

const WalletContext = createContext<WalletContextValue | undefined>(undefined);

type WalletProviderProps = {
  children: ReactNode;
  initialWallet?: KeyPairSigner | null;
};

export function WalletProvider({ children, initialWallet = null }: WalletProviderProps) {
  const [wallet, setWalletState] = useState<KeyPairSigner | null>(initialWallet);
  const queryClient = useQueryClient();

  const setWallet = useCallback((value: KeyPairSigner | null) => {
    setWalletState(value);
  }, []);

  const createKeypair = useCallback( async () => {
    const privateScalar = utils.randomSecretKey();

    const localStorageWallets = localStorage.getItem("localStorageWallets")
    if (localStorageWallets) {
      const localStorageWalletsArray = JSON.parse(localStorageWallets) as LocalStorageKeypairs;
      localStorageWalletsArray.wallets.push( Array.from(privateScalar));
      localStorage.setItem("localStorageWallets", JSON.stringify(localStorageWalletsArray));
    } else {
      const localStorageWalletsArray: LocalStorageKeypairs = { wallets: [Array.from(privateScalar)] };
      localStorage.setItem("localStorageWallets", JSON.stringify(localStorageWalletsArray));
    }
    const keyPairSigner = await createKeyPairSignerFromPrivateKeyBytes(privateScalar);
    setWallet(keyPairSigner);
    queryClient.invalidateQueries({ queryKey: ['localWallets'] });
    return keyPairSigner;
  }, []);

  const deleteWallet = useCallback(async (walletToDelete: KeyPairSigner) => {
    const localStorageWallets = localStorage.getItem("localStorageWallets");
    if (!localStorageWallets) return;

    const localStorageWalletsArray = JSON.parse(localStorageWallets) as LocalStorageKeypairs;

    // Convert stored wallets to KeyPairSigner objects to compare public keys
    const recreatedWallets = await Promise.all(
      localStorageWalletsArray.wallets.map(async (keyPair) => {
        return await createKeyPairSignerFromPrivateKeyBytes(new Uint8Array(keyPair));
      })
    );

    // Find the index of the wallet to delete by comparing public keys
    const walletIndex = recreatedWallets.findIndex(
      recreatedWallet => recreatedWallet.address.toString() === walletToDelete.address.toString()
    );

    if (walletIndex !== -1) {
      // Remove the wallet from the stored array
      localStorageWalletsArray.wallets.splice(walletIndex, 1);
      localStorage.setItem("localStorageWallets", JSON.stringify(localStorageWalletsArray));
    }

    // If the deleted wallet is currently selected, clear it
    if (wallet && wallet.address.toString() === walletToDelete.address.toString()) {
      setWallet(null);
    }

    queryClient.invalidateQueries({ queryKey: ['localWallets'] });
  }, [wallet, setWallet, queryClient]);

  const value = useMemo<WalletContextValue>(() => ({ wallet, setWallet, createKeypair, deleteWallet }), [wallet, setWallet, createKeypair, deleteWallet]);
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

export function useCreateKeypair() {
  const ctx = useContext(WalletContext);
  if (!ctx) throw new Error('useCreateKeypair must be used within a WalletProvider');
  return ctx.createKeypair;
}

export function useDeleteWallet() {
  const ctx = useContext(WalletContext);
  if (!ctx) throw new Error('useDeleteWallet must be used within a WalletProvider');
  return ctx.deleteWallet;
}
