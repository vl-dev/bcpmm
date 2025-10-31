// Migrations are an early feature. Currently, they're nothing more than this
// single deploy script that's invoked from the CLI, injecting a provider
// configured from the workspace's Anchor.toml.

import * as anchor from "@coral-xyz/anchor";
import { Cbmm } from "../target/types/cbmm";
import { Program } from "@coral-xyz/anchor";
import { Keypair } from "@solana/web3.js";
import fs from "fs";
import { PublicKey } from "@solana/web3.js";
import { SystemProgram } from "@solana/web3.js";

module.exports = async function (provider: anchor.AnchorProvider) {
  // Configure client to use the provider.
  anchor.setProvider(provider);

  const program = anchor.workspace.cbmm as Program<Cbmm>;
  // /Users/matusvla/go/src/github.com/matusvla/bcpmm/.keypairs/authority.json
  const programAuthority = Keypair.fromSecretKey(Buffer.from(JSON.parse(fs.readFileSync(".keypairs/authority.json", "utf8"))));
  const BPF_LOADER_UPGRADEABLE_PROGRAM_ID = new PublicKey(
    'BPFLoaderUpgradeab1e11111111111111111111111'
  );

  const [programDataAddress] = PublicKey.findProgramAddressSync(
    [program.programId.toBuffer()],
    BPF_LOADER_UPGRADEABLE_PROGRAM_ID
  );
  const [centralStatePDA] = PublicKey.findProgramAddressSync(
    [Buffer.from('central_state')],
    program.programId
  );
  const initializeCentralStateArgs = {
    maxUserDailyBurnCount: 10,
    maxCreatorDailyBurnCount: 10,
    userBurnBpX100: 1000, // 10%
    creatorBurnBpX100: 500, // 5%
    burnResetTimeOfDaySeconds: Date.now() / 1000 + 86400, // 24 hours from now
    creatorFeeBasisPoints: 100,
    buybackFeeBasisPoints: 200,
    platformFeeBasisPoints: 300,
    admin: provider.wallet.publicKey,
  };
  const initializeCentralStateAccounts = {
    authority: programAuthority.publicKey,
    centralState: centralStatePDA,
    systemProgram: SystemProgram.programId,
    programData: programDataAddress,
  };
  await program.methods
    .initializeCentralState(initializeCentralStateArgs)
    .accounts(initializeCentralStateAccounts)
    .rpc();

};
