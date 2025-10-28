import { createKeyPairFromBytes, createSignerFromKeyPair, type KeyPairSigner, } from '@solana/kit';
import { getAdminKeypair } from './admin-keypair';
import { TEST_KEYPAIRS } from './test-keypairs';

export async function getLocalWallets(): Promise<KeyPairSigner[]> {
  
  const adminSigner = await getAdminKeypair();
  const testWallets = await Promise.all(TEST_KEYPAIRS.map(async (keyPair) => {
    return await createSignerFromKeyPair(await createKeyPairFromBytes(new Uint8Array(keyPair)));
  }));

  return [adminSigner, ...testWallets];
}
