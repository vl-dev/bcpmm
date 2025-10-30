import { useQuery } from '@tanstack/react-query';
import { createKeyPairSignerFromPrivateKeyBytes, type KeyPairSigner, } from '@solana/kit';
import { getAdminKeypair } from '../admin-keypair';

type LocalStorageKeypairs = {
  wallets: number[][]
}

export async function getLocalWallets(): Promise<KeyPairSigner[]> {
  const adminSigner = await getAdminKeypair();
  const localStorageWallets = localStorage.getItem("localStorageWallets");
  const localStorageWalletsObject = localStorageWallets ? JSON.parse(localStorageWallets) as LocalStorageKeypairs : { wallets: [] };
  const createdWallets = await Promise.all(localStorageWalletsObject.wallets.map(async (keyPair) => {
    const keyPairSigner = await createKeyPairSignerFromPrivateKeyBytes(new Uint8Array(keyPair));
    return keyPairSigner;
  }));

  return [adminSigner, ...createdWallets];
}

export function useLocalWallets() {
  return useQuery({
    queryKey: ['localWallets'],
    queryFn: getLocalWallets,
  });
}