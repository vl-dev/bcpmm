import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { CpmmPoc } from "../target/types/cpmm_poc";
import { Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
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

  it("Can swap acs to ct", async () => {
    const provider = anchor.getProvider();
    const payer = await provider.wallet.payer;
    const secondPayer = new Keypair();

    // Top up second payer
    await provider.connection.requestAirdrop(secondPayer.publicKey, LAMPORTS_PER_SOL * 10);

    // Create ACS token mint and mint tokens
    const aMint = await createMint(
      provider.connection as any,
      payer,
      provider.wallet.publicKey,
      null,
      9,
    );

    const payerAta = await createAssociatedTokenAccount(
      provider.connection as any,
      payer,
      aMint,
      provider.wallet.publicKey
    );

    await mintTo(
      provider.connection as any,
      payer,
      aMint,
      payerAta,
      payer, // Use payer as the mint authority signer
      BigInt("1000000000000000000"), // 1B tokens
    );

    const bMintKeypair = new Keypair();
    const bMint = bMintKeypair.publicKey;
    // Create CPMM Pool
    console.log("Creating CPMM Pool");
    const [pool] = PublicKey.findProgramAddressSync(
      [Buffer.from('bcpmm_pool'), bMint.toBuffer()],
      program.programId
    );

    const poolAta = await getAssociatedTokenAddress(
      aMint,
      pool,
      true
    );

    const createPoolAccounts = {
      payer: provider.wallet.publicKey,
      aMint: aMint,
      bMint: bMint,
      pool: pool,
      poolAta: poolAta,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    };
    const createPoolArgs = {
      bInitialSupply: new BN(10_000_000_000_000),
      bDecimals: 6,
      aVirtualReserve: new BN("2000000000000000000"),
      creatorFeeBasisPoints: 500,
      buybackFeeBasisPoints: 100,
    };

    const createPoolSx = await program.methods
      .createPool(createPoolArgs)
      .accounts(createPoolAccounts)
      .rpc();

    console.log("Create pool tx: ", createPoolSx);

    // Create CT Account
    console.log("Creating CT Account");
    const [virtualTokenAccountAddress] = PublicKey.findProgramAddressSync(
      [Buffer.from('virtual_token_account'), pool.toBuffer(), provider.wallet.publicKey.toBuffer()],
      program.programId
    );
    const initVirtualTokenAccountAccounts = {
      payer: provider.wallet.publicKey,
      owner: provider.wallet.publicKey,
      virtualTokenAccount: virtualTokenAccountAddress,
      pool: pool,
      systemProgram: SystemProgram.programId,
    };
    const initVirtualTokenAccountSx = await program.methods
      .initializeVirtualTokenAccount()
      .accounts(initVirtualTokenAccountAccounts)
      .rpc();

    console.log("Initialize virtual token account tx: ", initVirtualTokenAccountSx);

    // Burn tokens
    console.log("Burning tokens");
    const burnVirtualTokenArgs = {
      bAmountBasisPoints: 9000,
    };
    const burnVirtualTokenAccounts = {
      payer: provider.wallet.publicKey,
      pool: pool,
    };
    const burnVirtualTokenSx = await program.methods
      .burnVirtualToken(burnVirtualTokenArgs)
      .accounts(burnVirtualTokenAccounts)
      .signers([payer])
      .rpc();

    console.log("Burn virtual token tx: ", burnVirtualTokenSx);

    // Verify the burn was successful and pool updated
    console.log("Verifying the burn was successful and pool updated");
    let poolAccount = await program.account.bcpmmPool.fetch(pool);
    console.log("B reserve: ", poolAccount.bReserve.toString());
    console.log("Virtual ACS Reserve: ", poolAccount.aVirtualReserve.toString());
    console.log("Creator Fees Balance: ", poolAccount.creatorFeesBalance.toString());
    console.log("Buyback Fees Balance: ", poolAccount.buybackFeesBalance.toString());
    console.log("Creator Fee Basis Points: ", poolAccount.creatorFeeBasisPoints.toString());
    console.log("Buyback Fee Basis Points: ", poolAccount.buybackFeeBasisPoints.toString());

    // Buy tokens
    console.log("Buying tokens");
    const buyVirtualTokenArgs = {
      aAmount: new BN(1_000_000_000_000),
    };
    const buyVirtualTokenAccounts = {
      payer: provider.wallet.publicKey,
      payerAta: payerAta,
      virtualTokenAccount: virtualTokenAccountAddress,
      pool: pool,
      poolAta: poolAta,
      aMint: aMint,
      bMint: bMint,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    };
    const buyVirtualTokenSx = await program.methods
      .buyVirtualToken(buyVirtualTokenArgs)
      .accounts(buyVirtualTokenAccounts)
      .signers([payer])
      .rpc();

    console.log("Buy virtual token tx: ", buyVirtualTokenSx);
    // Verify the swap was successful
    console.log("Verifying the swap was successful");
    let virtualTokenAccount = await program.account.virtualTokenAccount.fetch(virtualTokenAccountAddress);
    console.log("CT balance: ", virtualTokenAccount.balance.toNumber());
    console.log("Fees collected: ", virtualTokenAccount.feesPaid.toNumber());

    // Print whole pool formatted fields
    poolAccount = await program.account.bcpmmPool.fetch(pool);
    console.log(`Pool ${pool.toBase58()}:`);
    console.log("Mint A Reserve: ", poolAccount.aReserve.toString());
    console.log("Mint B Reserve: ", poolAccount.bReserve.toString());
    console.log("Virtual ACS Reserve: ", poolAccount.aVirtualReserve.toString());
    console.log("Mint A: ", poolAccount.aMint.toBase58());
    console.log("Mint B: ", poolAccount.bMint.toBase58());
    console.log("Creator Fees Balance: ", poolAccount.creatorFeesBalance.toString());
    console.log("Buyback Fees Balance: ", poolAccount.buybackFeesBalance.toString());
    console.log("Creator Fee Basis Points: ", poolAccount.creatorFeeBasisPoints.toString());
    console.log("Buyback Fee Basis Points: ", poolAccount.buybackFeeBasisPoints.toString());

    // Sell tokens
    console.log("Selling tokens");
    const sellVirtualTokenArgs = {
      bAmount: virtualTokenAccount.balance,
    };
    const sellVirtualTokenAccounts = {
      payer: provider.wallet.publicKey,
      payerAta: payerAta,
      virtualTokenAccount: virtualTokenAccountAddress,
      pool: pool,
      poolAta: poolAta,
      aMint: aMint,
      bMint: bMint,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    };
    const sellVirtualTokenSx = await program.methods
      .sellVirtualToken(sellVirtualTokenArgs)
      .accounts(sellVirtualTokenAccounts)
      .signers([payer])
      .rpc();

    console.log("Sell virtual token tx: ", sellVirtualTokenSx);

    // Verify the swap was successful
    console.log("Verifying the swap was successful");
    virtualTokenAccount = await program.account.virtualTokenAccount.fetch(virtualTokenAccountAddress);
    assert(virtualTokenAccount.balance.toNumber() < 1_000_000_000, "CT balance should be less than 1B");
    console.log("CT balance: ", virtualTokenAccount.balance.toNumber());
    console.log("Fees collected: ", virtualTokenAccount.feesPaid.toNumber());

    // Close virtual token account
    console.log("Closing virtual token account");
    const closeVirtualTokenAccountAccounts = {
      owner: provider.wallet.publicKey,
      virtualTokenAccount: virtualTokenAccountAddress,
    };
    const closeVirtualTokenAccountSx = await program.methods
      .closeVirtualTokenAccount()
      .accounts(closeVirtualTokenAccountAccounts)
      .signers([payer])
      .rpc();
    console.log("Close virtual token account tx: ", closeVirtualTokenAccountSx);

    // Verify the virtual token account was closed
    console.log("Verifying the virtual token account was closed");
    const virtualTokenAccountExists = await accountExists(virtualTokenAccountAddress);
    assert(!virtualTokenAccountExists, "Virtual token account should not exist");
  });
});
