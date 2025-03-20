# FunPump.Ai Solana Anchor Program Architecture

Below is a detailed explanation of how to ensure that the [FunPump.Ai](http://FunPump.Ai) Solana Anchor program maintains its core functionalities—vault locking/unlocking and vesting mechanisms—while integrating a bonding curve for token price discovery and dynamic supply issuance, along with launch options (bonding curve or PumpFun) and Token Metadata integration.

---

## Core Functionalities Maintained

The original [FunPump.Ai](http://FunPump.Ai) program provides mechanisms for vault-based token locking and vesting with time and market-cap conditions. These will remain intact and serve as the foundation for the new features.

### Vault Mechanism

1. **Initialize a Vault**:
   - A Program-Derived Address (PDA) is created to manage locked tokens.
   - Stores the owner, locked amount, and unlock timestamp.

2. **Lock Tokens**:
   - Transfers tokens from a user's token account to the vault's token account.
   - Sets the locked amount and unlock time based on a specified duration.

3. **Unlock Tokens**:
   - Transfers tokens back to the user's token account once the lock period ends.

### Vesting Mechanism

1. **Initialize a Vesting Account**:
   - Creates a vesting PDA with a specified token amount, start time, end time, and target market cap.

2. **Lock Tokens for Vesting**:
   - Transfers tokens into the vesting PDA's token account.

3. **Unlock Vested Tokens**:
   - Releases tokens when both the vesting period ends (`current_time >= end_time`) and the market cap exceeds the target (`current_market_cap >= target_market_cap`).

These mechanisms ensure time-based locks (e.g., liquidity or team tokens) and vesting schedules (e.g., gradual or cliff releases) with optional conditions like market cap.

---

## Integrating a Bonding Curve

A bonding curve enables token price discovery and dynamic supply issuance by adjusting the price based on the current supply. We'll use a **linear bonding curve** (price increases linearly with supply) for simplicity, though other formulas (e.g., polynomial) could be adapted.

### Bonding Curve Mechanism

- **Purpose**: Allows users to buy tokens with SOL, where the price depends on the current supply.
- **Integration**: After purchase, tokens can be locked in a vault or vesting account to prevent immediate dumping.

#### New Account: Bonding Curve

```rust
#[account]
pub struct BondingCurve {
    pub authority: Pubkey,       // Program authority
    pub token_mint: Pubkey,      // Token being sold
    pub initial_price: u64,      // Starting price in SOL lamports
    pub slope: u64,              // Price increase per token
    pub total_supply: u64,       // Current minted supply
    pub treasury_balance: u64,   // SOL collected
    pub bump: u8,                // PDA bump seed
}
```

#### Instruction: Buy Tokens

```rust
pub fn buy_tokens(ctx: Context<BuyTokens>, sol_amount: u64) -> Result<()> {
    let curve = &mut ctx.accounts.curve;
    let price = curve.initial_price + (curve.total_supply * curve.slope); // Linear curve
    let tokens_to_mint = sol_amount / price;

    // Mint tokens to buyer
    token::mint_to(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::MintTo {
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.buyer_token_account.to_account_info(),
                authority: curve.to_account_info(),
            },
        ),
        tokens_to_mint,
    )?;

    // Update state
    curve.total_supply += tokens_to_mint;
    curve.treasury_balance += sol_amount;

    Ok(())
}
```

- **Accounts**:
  - `curve`: BondingCurve PDA
  - `token_mint`: Token mint account
  - `buyer_token_account`: Buyer's token account
  - `token_program`: SPL Token program
  - Buyer sends SOL via system transfer (not shown for brevity).

#### Linking to Vault and Vesting

- **Option 1: Immediate Transfer**: Tokens are minted directly to the buyer's account (as above).
- **Option 2: Lock in Vault**: After minting, call `lock_tokens` to transfer tokens to a vault PDA for a set duration.
- **Option 3: Vesting**: After minting, initialize a vesting account and lock tokens with time and market-cap conditions.

Example with vault locking:

```rust
pub fn buy_and_lock(ctx: Context<BuyAndLock>, sol_amount: u64, lock_duration: i64) -> Result<()> {
    // Buy tokens (simplified)
    let curve = &mut ctx.accounts.curve;
    let price = curve.initial_price + (curve.total_supply * curve.slope);
    let tokens_to_mint = sol_amount / price;
    token::mint_to(/* mint to vault_token_account */, tokens_to_mint)?;

    // Lock tokens
    let vault = &mut ctx.accounts.vault;
    vault.locked_amount = tokens_to_mint;
    vault.locked_until = Clock::get()?.unix_timestamp + lock_duration;

    curve.total_supply += tokens_to_mint;
    curve.treasury_balance += sol_amount;
    Ok(())
}
```

---

## Launch Options: Bonding Curve or PumpFun

Users can launch tokens via the bonding curve or directly to PumpFun.

### Instruction: Launch Token

```rust
pub fn launch_token(
    ctx: Context<LaunchToken>,
    name: String,
    symbol: String,
    uri: String,
    launch_type: u8, // 0 = Bonding Curve, 1 = PumpFun
    initial_price: u64,
    slope: u64,
) -> Result<()> {
    if launch_type == 0 {
        // Bonding curve launch
        let curve = &mut ctx.accounts.curve;
        curve.authority = ctx.accounts.authority.key();
        curve.token_mint = ctx.accounts.token_mint.key();
        curve.initial_price = initial_price;
        curve.slope = slope;
        curve.total_supply = 0;
        curve.treasury_balance = 0;
        curve.bump = ctx.bumps.curve;
    } else if launch_type == 1 {
        // PumpFun launch (emit event for off-chain integration)
        emit!(PumpFunLaunchEvent {
            creator: ctx.accounts.authority.key(),
            token_mint: ctx.accounts.token_mint.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    // Set token metadata
    let metadata_pda = Pubkey::find_program_address(
        &[b"metadata", &MPL_TOKEN_METADATA_PROGRAM_ID, ctx.accounts.token_mint.key().as_ref()],
        &MPL_TOKEN_METADATA_PROGRAM_ID,
    ).0;

    invoke(
        &create_metadata_accounts_v3(
            MPL_TOKEN_METADATA_PROGRAM_ID,
            metadata_pda,
            ctx.accounts.token_mint.key(),
            ctx.accounts.authority.key(),
            ctx.accounts.authority.key(),
            ctx.accounts.authority.key(),
            name,
            symbol,
            uri,
            None,
            0,
            true,
            false,
            None,
            None,
            None,
        ),
        &[
            ctx.accounts.metadata_program.to_account_info(),
            ctx.accounts.metadata.to_account_info(),
            ctx.accounts.token_mint.to_account_info(),
            ctx.accounts.authority.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.rent.to_account_info(),
        ],
    )?;

    Ok(())
}
```

- **Accounts**:
  - `authority`: Token creator
  - `token_mint`: New token mint
  - `curve`: BondingCurve PDA (if launch_type == 0)
  - `metadata_program`: MPL Token Metadata program
  - `metadata`: Metadata PDA
  - `system_program`, `rent`: For account creation

- **Bonding Curve (launch_type = 0)**: Initializes the bonding curve for token sales.
- **PumpFun (launch_type = 1)**: Emits an event for off-chain PumpFun integration (actual integration depends on PumpFun's API/program).

---

## Token Metadata Integration

The `launch_token` instruction uses the **MPL Token Metadata program** to set the token's name, symbol, and URI during launch, ensuring compliance with Solana token standards.

---

## Combining Everything

### Workflow Example

1. **Launch**: User calls `launch_token` with `launch_type = 0` (bonding curve), setting metadata and initializing the curve.
2. **Buy Tokens**: User sends SOL to `buy_tokens`, receiving tokens based on the current supply and price.
3. **Lock/Vest**:
   - Tokens are locked via `lock_tokens` for a duration (e.g., 6 months).
   - Alternatively, `initialize_vesting` and `lock_tokens_for_vesting` set a vesting schedule (e.g., unlock after 1 year and market cap > $1M).
4. **Unlock**: After the lock period or vesting conditions are met, `unlock_tokens` or `unlock_vested_tokens` releases the tokens.

### Code Snippets

- **Vault Lock**:

```rust
pub fn lock_tokens(ctx: Context<LockTokens>, amount: u64, lock_duration: i64) -> Result<()> {
    token::transfer(/* from user to vault */, amount)?;
    let vault = &mut ctx.accounts.vault;
    vault.locked_amount = amount;
    vault.locked_until = Clock::get()?.unix_timestamp + lock_duration;
    Ok(())
}
```

- **Vesting Unlock**:

```rust
pub fn unlock_vested_tokens(ctx: Context<UnlockVestedTokens>, current_market_cap: u64) -> Result<()> {
    let vesting = &mut ctx.accounts.vesting;
    require!(vesting.is_locked && Clock::get()?.unix_timestamp >= vesting.end_time && 
             current_market_cap >= vesting.target_market_cap, "Conditions not met");
    token::transfer(/* from vesting to owner */, vesting.amount)?;
    vesting.is_locked = false;
    Ok(())
}
```

---

## Conclusion

This enhanced Solana Anchor program:

- **Maintains Core Features**: Vault locking/unlocking and vesting with time/market-cap conditions.
- **Adds Bonding Curve**: Enables dynamic pricing and supply issuance, with purchased tokens optionally locked or vested.
- **Supports Launch Options**: Bonding curve or PumpFun, with metadata set via the Token Metadata program.
- **Ensures Flexibility**: Tokens can be immediately available, time-locked, or vested based on project needs.

This setup provides a robust foundation for token launches, balancing price discovery with controlled release mechanisms.
