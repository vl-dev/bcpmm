# CBMM Demo

Simple React app demonstrating the CBMM Solana program client.

## Setup

```bash
yarn install
```

## Run

```bash
yarn dev
```

Then open http://localhost:5173

## Features

- TypeScript support
- React 18
- Local js-client dependency integration (`@cbmm/js-client`)
- Vite for fast development and building
- Solana Kit support

## Local Dependency

The js-client from `../sdk/js-client` is imported as `@cbmm/js-client` using:
- Vite alias configuration (`vite.config.ts`)
- TypeScript path mapping (`tsconfig.json`)

