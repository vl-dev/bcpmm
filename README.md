# CBMM: Constant Burn Market Maker

> **⚠️ Work in Progress**: This project is currently under active development. Some features may not be fully functional, and breaking changes may occur.

An experimental Solana program that enables turning off-chain activity into on-chain price impact. CBMM is a novel mathematical extension of the classic constant product market maker—we add virtual reserves and built-in burns to let you reward verifiable off-chain behavior with real token supply reduction and price appreciation.

Think of it as a CPMM pool that actually burns tokens when people do off-chain stuff—not just when they trade. The math is new, the safety proofs are formal, and we're exploring whether this actually works in practice.

## What's Inside

- **Solana Program** (`programs/cbmm/`) - The on-chain Anchor program with trading, burns, and Continuous Conditional Buybacks (CCB)
- **SDKs** (`sdk/`) - TypeScript and Rust clients for interacting with CBMM pools
- **Demo App** (`demo/`) - Next.js frontend showing pool creation, trading, and burn mechanics

## How It Works

CBMM pools start with a virtual quote reserve that sets the initial price. When burns happen (tied to off-chain events), the virtual reserve adjusts to maintain solvency while the base token supply shrinks. Trading fees automatically flow into the real quote reserve via CCB, pushing prices higher and offsetting the virtual reserve reduction.

The novel part is ensuring that burns can't extract value from the pool—we derive closed-form safety conditions where fees and burn caps work together to keep everything provably safe. The [whitepaper](whitepaper/main.pdf) contains the full mathematical model, attack analysis, and formal proofs.

## Quick Start

```bash
# Build the program
anchor build

# Run tests
anchor test

# Start the demo app
cd demo && npm install && npm run dev
```

## Documentation

The complete mathematical model, safety proofs, and implementation details are in the [whitepaper PDF](whitepaper/main.pdf).

