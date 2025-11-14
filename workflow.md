# GitHub Actions Workflow Plan for Automated Testing

## Goal
Set up GitHub Actions to automatically run all 103 Rust tests on every push and pull request.

## Current Test Setup
- **Command:** `anchor test` or `cargo test -p cbmm`
- **Tests:** 103 tests (82 unit + 21 integration)
- **Location:** `programs/cbmm/src/` (unit tests in instruction files, integration tests in `src/tests/`)
- **No feature flags needed** (tests are part of library with `#[cfg(test)]`)

## Workflow File Location
```
.github/
└── workflows/
    └── test.yml
```

## Workflow Configuration

### Triggers
- **Push to any branch** (to catch issues immediately)
- **Pull requests** (to validate before merging)
- **Manual trigger** (for debugging)

### Jobs

#### Job 1: Rust Tests
**Purpose:** Run all Rust tests (unit + integration)

**Steps:**
1. Checkout code
2. Install Rust toolchain (stable)
3. Install Solana CLI tools
4. Install Anchor CLI
5. Cache dependencies (Cargo)
6. Build Solana program
7. Run tests
8. Upload test results (optional)

#### Job 2: Clippy Linting
**Purpose:** Check code quality

**Steps:**
1. Checkout code
2. Install Rust + Clippy
3. Run `cargo clippy --all-targets --all-features -- -D warnings`

#### Job 3: Format Check
**Purpose:** Ensure consistent code formatting

**Steps:**
1. Checkout code
2. Install Rust + rustfmt
3. Run `cargo fmt -- --check`

## Detailed Workflow YAML

```yaml
name: Test

on:
  push:
    branches: [ "**" ]  # All branches
  pull_request:
    branches: [ "main", "master", "develop" ]
  workflow_dispatch:  # Allow manual trigger

env:
  SOLANA_VERSION: "1.18.22"  # Match your local version
  ANCHOR_VERSION: "0.32.1"   # Match Anchor.toml version

jobs:
  test:
    name: Run Tests
    runs-on: ubuntu-latest
    
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      
      - name: Cache Rust dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/bin/
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            bcpmm/target/
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-
      
      - name: Install Solana
        run: |
          sh -c "$(curl -sSfL https://release.solana.com/v${{ env.SOLANA_VERSION }}/install)"
          echo "$HOME/.local/share/solana/install/active_release/bin" >> $GITHUB_PATH
          export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
          solana --version
      
      - name: Install Anchor CLI
        run: |
          cargo install --git https://github.com/coral-xyz/anchor anchor-cli --tag v${{ env.ANCHOR_VERSION }} --locked --force
          anchor --version
      
      - name: Build program
        run: |
          cd bcpmm
          anchor build
      
      - name: Run Rust tests
        run: |
          cd bcpmm
          cargo test -p cbmm -- --nocapture --test-threads=1
      
      - name: Test summary
        if: success()
        run: |
          echo "✅ All 103 tests passed!"
          echo "- 82 unit tests"
          echo "- 21 integration tests"

  lint:
    name: Clippy Linting
    runs-on: ubuntu-latest
    
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      
      - name: Cache dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            bcpmm/target/
          key: ${{ runner.os }}-clippy-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Run Clippy
        run: |
          cd bcpmm/programs/cbmm
          cargo clippy --all-targets -- -D warnings

  format:
    name: Format Check
    runs-on: ubuntu-latest
    
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      
      - name: Check formatting
        run: |
          cd bcpmm/programs/cbmm
          cargo fmt -- --check
```

## Alternative: Minimal Workflow (Faster)

If you want a simpler, faster workflow (just tests, no linting):

```yaml
name: Test

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Cache
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            bcpmm/target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Install Solana
        run: sh -c "$(curl -sSfL https://release.solana.com/stable/install)"
      
      - name: Install Anchor
        run: cargo install --git https://github.com/coral-xyz/anchor anchor-cli --tag v0.32.1 --locked
      
      - name: Test
        run: |
          cd bcpmm
          anchor build
          cargo test -p cbmm
```

## Implementation Steps

### Step 1: Create workflow file
```bash
mkdir -p .github/workflows
# Create test.yml with the YAML above
```

### Step 2: Commit and push
```bash
git add .github/workflows/test.yml
git commit -m "Add GitHub Actions workflow for automated testing"
git push origin <your-branch>
```

### Step 3: Verify in GitHub
1. Go to your repository on GitHub
2. Click "Actions" tab
3. You should see the workflow running
4. Wait for green checkmark ✅

### Step 4: Add status badge to README (optional)
```markdown
[![Test](https://github.com/vl-dev/bcpmm/workflows/Test/badge.svg)](https://github.com/vl-dev/bcpmm/actions)
```

## Optimization Tips

### 1. Cache Solana Installation
```yaml
- name: Cache Solana
  uses: actions/cache@v3
  with:
    path: ~/.local/share/solana
    key: solana-${{ env.SOLANA_VERSION }}
```

### 2. Cache Anchor Binary
```yaml
- name: Cache Anchor
  uses: actions/cache@v3
  with:
    path: ~/.cargo/bin/anchor
    key: anchor-${{ env.ANCHOR_VERSION }}
```

### 3. Parallel Jobs
Run tests and linting in parallel (already done in full workflow above).

### 4. Skip CI for documentation changes
```yaml
on:
  push:
    paths-ignore:
      - '**.md'
      - 'docs/**'
```

## Expected Results

**On Push:**
```
✅ test / Run Tests (1m 30s)
✅ lint / Clippy Linting (45s)
✅ format / Format Check (20s)
```

**On PR:**
- Shows status checks at the bottom of PR
- Requires passing tests before merge (if branch protection enabled)

## Troubleshooting

### Issue: "anchor: command not found"
**Fix:** Make sure Anchor is in PATH:
```yaml
echo "$HOME/.cargo/bin" >> $GITHUB_PATH
```

### Issue: "solana: command not found"
**Fix:** Add Solana to PATH:
```yaml
echo "$HOME/.local/share/solana/install/active_release/bin" >> $GITHUB_PATH
```

### Issue: Tests timeout
**Fix:** Increase timeout:
```yaml
jobs:
  test:
    timeout-minutes: 30  # Default is 60
```

### Issue: Out of disk space
**Fix:** Clean up before building:
```yaml
- name: Free disk space
  run: |
    sudo rm -rf /usr/share/dotnet
    sudo rm -rf /opt/ghc
```

## Cost Consideration

- **GitHub Actions:** 2,000 minutes/month free for public repos
- **This workflow:** ~3 minutes per run
- **Estimate:** ~600 runs/month within free tier

For private repos: 2,000 minutes/month included in Pro plan.

## Next Steps After Implementation

1. ✅ Create `.github/workflows/test.yml`
2. ✅ Push to GitHub
3. ✅ Verify workflow runs successfully
4. ✅ Add branch protection rules (require passing tests)
5. ✅ Add status badge to README
6. ✅ Configure notifications (optional)

## Branch Protection (Recommended)

After workflow is set up:

1. Go to: Settings → Branches → Branch protection rules
2. Add rule for `main` branch
3. Enable: "Require status checks to pass before merging"
4. Select: `test / Run Tests`
5. Save

This prevents merging PRs with failing tests! 🛡️

---

**Ready to implement?** Create the workflow file and push to GitHub!

