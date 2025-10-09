import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { CpmmPoc } from "../target/types/cpmm_poc";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { createMint, createAssociatedTokenAccount, mintTo } from "@solana/spl-token";
import { BN } from "bn.js";
import { assert } from "chai";
import { getAssociatedTokenAddress } from "@solana/spl-token";
import { ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID } from "@solana/spl-token";

describe("cpmm-poc", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.cpmmPoc as Program<CpmmPoc>;

  // Helper function to check if an account exists
  async function accountExists(pubkey: PublicKey): Promise<boolean> {
    const account = await program.provider.connection.getAccountInfo(pubkey);
    return account !== null;
  }

  beforeEach(async () => {
    // Get all PDAs we need to check
    const [centralStatePDA] = PublicKey.findProgramAddressSync(
      [Buffer.from('central_state')],
      program.programId
    );

    // If central state exists, we'll skip initialization in the test
    const centralStateExists = await accountExists(centralStatePDA);
    if (centralStateExists) {
      console.log("Central state already exists, skipping initialization");
    }
  });

  it("Is initialized!", async () => {
    const provider = anchor.getProvider();
    const payer = await provider.wallet.payer;

    // Create ACS token mint and mint tokens
    const acsMint = await createMint(
      provider.connection as any,
      payer,
      provider.wallet.publicKey,
      null,
      6
    );

    const acsAta = await createAssociatedTokenAccount(
      provider.connection as any,
      payer,
      acsMint,
      provider.wallet.publicKey
    );

    await mintTo(
      provider.connection as any,
      payer,
      acsMint,
      acsAta,
      payer, // Use payer as the mint authority signer
      1_000_000_000, // 1B tokens
    );

    // PDA for the page visits account
    const [centralStatePDA] = PublicKey.findProgramAddressSync([Buffer.from('central_state')], program.programId);

    // Check if account already exists
    const exists = await accountExists(centralStatePDA);
    if (exists) {
      console.log("Central state already initialized");
      return;
    }

    const centralStateAta = await getAssociatedTokenAddress(acsMint, centralStatePDA, true)
    const accounts = {
      payer: provider.wallet.publicKey,
      centralState: centralStatePDA,
      centralStateAta: centralStateAta,
      systemProgram: SystemProgram.programId,
      acsMint: acsMint,
      tokenProgram: TOKEN_PROGRAM_ID,
    };

    try {
      const tx = await program.methods.initialize().accounts(accounts).rpc();
      console.log("Your transaction signature", tx);
    } catch (e) {
      if (e.message.includes("already in use")) {
        console.log("Central state already initialized");
        return;
      }
      throw e;
    }
  });
  it("Can swap acs to ct", async () => {
    const provider = anchor.getProvider();
    const payer = await provider.wallet.payer;

    // Central state is already initialized from the previous test
    const [centralStatePDA] = PublicKey.findProgramAddressSync([Buffer.from('central_state')], program.programId);
    const centralStateAccount = await program.account.centralState.fetch(centralStatePDA);
    const acsMint = centralStateAccount.acsMint;

    // Create CPMM Pool
    console.log("Creating CPMM Pool");
    const [cpmmPoolPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from('cpmm_pool'), new BN(0).toArrayLike(Buffer, 'le', 8)],
      program.programId
    );
    const createPoolAccounts = {
      centralState: centralStatePDA,
      cpmmPool: cpmmPoolPDA,
      payer: provider.wallet.publicKey,
      systemProgram: SystemProgram.programId,
    };
    await program.methods
      .createPool(new BN(100), new BN(20))
      .accounts(createPoolAccounts)
      .rpc();

    // Create CT Account
    console.log("Creating CT Account");
    const [ctAccountPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from('token_account'), cpmmPoolPDA.toBuffer(), provider.wallet.publicKey.toBuffer()],
      program.programId
    );
    const initCtAccounts = {
      tokenAccount: ctAccountPDA,
      payer: provider.wallet.publicKey,
      systemProgram: SystemProgram.programId,
      cpmmPool: cpmmPoolPDA,
    };
    await program.methods
      .initializeCtAccount()
      .accounts(initCtAccounts)
      .rpc();

    // Buy tokens
    console.log("Buying tokens");
    const centralStateAta = await getAssociatedTokenAddress(acsMint, centralStatePDA, true)
    const acsAta = await getAssociatedTokenAddress(acsMint, provider.wallet.publicKey, true)
    const buyTokenAccounts = {
      ctAccount: ctAccountPDA,
      cpmmPool: cpmmPoolPDA,
      acsMint: acsMint,
      acsAta: acsAta,
      centralStateAta: centralStateAta,
      systemProgram: SystemProgram.programId,
    };
    await program.methods
      .buyToken(new BN(1000))
      .accounts(buyTokenAccounts)
      .signers([payer])
      .rpc();

    // Verify the swap was successful
    console.log("Verifying the swap was successful");
    let ctAccount = await program.account.ctAccount.fetch(ctAccountPDA);
    assert(ctAccount.balance.toNumber() > 0, "CT balance should be greater than 0");
    console.log("CT balance: ", ctAccount.balance.toNumber());

    // Print whole pool formatted fields
    const cpmmPool = await program.account.cpmmPool.fetch(cpmmPoolPDA);
    console.log("Pool:");
    console.log("Micro ACS Reserve: ", cpmmPool.microAcsReserve.toString());
    console.log("CT Reserve: ", cpmmPool.ctReserve.toString());
    console.log("Virtual ACS Reserve: ", cpmmPool.virtualAcsReserve.toString());
    console.log("Mint Index: ", cpmmPool.mintIndex.toString());

    // Sell tokens
    console.log("Selling tokens");
    const sellTokenAccounts = {
      ctAccount: ctAccountPDA,
      cpmmPool: cpmmPoolPDA,
      acsMint: acsMint,
      centralState: centralStatePDA,
      centralStateAta: centralStateAta,
      acsAta: acsAta,
      systemProgram: SystemProgram.programId,
      payer: provider.wallet.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
    };
    await program.methods
      .sellToken(new BN(1000))
      .accounts(sellTokenAccounts)
      .signers([payer])
      .rpc();

    // Verify the swap was successful
    console.log("Verifying the swap was successful");
    ctAccount = await program.account.ctAccount.fetch(ctAccountPDA);
    assert(ctAccount.balance.toNumber() < 1_000_000_000, "CT balance should be less than 1B");
    console.log("CT balance: ", ctAccount.balance.toNumber());
  });
});
