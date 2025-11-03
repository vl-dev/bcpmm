# CBMM Demo

Simple React app demonstrating the CBMM Solana program client.

## Setup

```bash
yarn install
```

## Environment Variables

Create a `.env.local` file with the following variables:
- `NEXT_PUBLIC_RPC_URL` - Solana RPC URL
- `NEXT_PUBLIC_WS_URL` - Solana WebSocket URL
- `NEXT_PUBLIC_ADMIN_KEYPAIR` - Admin keypair JSON array

## Run

```bash
yarn dev
```

Then open http://localhost:3000

## Features

- TypeScript support
- React 18
- Next.js 14
- Local js-client dependency integration (`@cbmm/js-client`)
- Solana Kit support

## Local Dependency

The js-client from `../sdk/js-client` is imported as `@cbmm/js-client` using:
- Next.js webpack alias configuration (`next.config.js`)
- TypeScript path mapping (`tsconfig.json`)

