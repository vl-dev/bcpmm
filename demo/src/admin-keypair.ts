import { createKeyPairFromBytes, createSignerFromKeyPair } from "@solana/kit";

export const getAdminKeypair = async () => {
  const adminKeypair = await createKeyPairFromBytes(new Uint8Array(
    JSON.parse(import.meta.env.VITE_ADMIN_KEYPAIR as string))
  );
  const adminSigner = await createSignerFromKeyPair(adminKeypair);
  return adminSigner;
};