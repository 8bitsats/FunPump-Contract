// voice_ai_launchpad.rs
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use solana_program::clock::UnixTimestamp;

// Import the new modules
use crate::state::{EnhancedBondingCurve, Vesting};
use crate::instructions::voice_launch::{self, *};
use crate::utils::pumpfun_integration::{PUMPFUN_PROGRAM_ID, MPL_TOKEN_METADATA_PROGRAM_ID};

declare_id!("VoiceLaunch11111111111111111111111111111111");

const MINIMUM_VESTING_PERIOD: i64 = 24 * 60 * 60; // 1 day
const MAXIMUM_VESTING_PERIOD: i64 = 365 * 24 * 60 * 60; // 1 year
const MINIMUM_AMOUNT: u64 = 1_000_000; // Minimum token amount (6 decimals)

#[program]
pub mod voice_ai_launchpad {
    use super::*;

    // Initialize the launchpad with global parameters
    pub fn initialize(
        ctx: Context<Initialize>,
        fee_basis_points: u64,
        initial_virtual_token_reserves: u64,
        initial_virtual_sol_reserves: u64,
        initial_real_token_reserves: u64,
    ) -> Result<()> {
        let global = &mut ctx.accounts.global;
        global.authority = ctx.accounts.authority.key();
        global.fee_recipient = ctx.accounts.fee_recipient.key();
        global.initial_virtual_token_reserves = initial_virtual_token_reserves;
        global.initial_virtual_sol_reserves = initial_virtual_sol_reserves;
        global.initial_real_token_reserves = initial_real_token_reserves;
        global.fee_basis_points = fee_basis_points;
        global.initialized = true;

        emit!(LaunchpadInitialized {
            authority: global.authority,
            fee_recipient: global.fee_recipient,
            timestamp: Clock::get()?.unix_timestamp
        });
        Ok(())
    }

    // Initialize vault for token locking
    pub fn initialize_vault(ctx: Context<InitializeVault>) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        vault.owner = ctx.accounts.owner.key();
        vault.bump = ctx.bumps.vault;
        vault.locked_amount = 0;
        vault.locked_until = 0;

        emit!(VaultInitialized {
            owner: vault.owner,
            vault: ctx.accounts.vault.key()
        });
        Ok(())
    }

    // Lock tokens in vault
    pub fn lock_tokens(
        ctx: Context<LockTokens>,
        amount: u64,
        lock_duration: i64
    ) -> Result<()> {
        require!(amount >= MINIMUM_AMOUNT, LaunchpadError::AmountTooLow);
        require!(
            lock_duration >= MINIMUM_VESTING_PERIOD && lock_duration <= MAXIMUM_VESTING_PERIOD,
            LaunchpadError::InvalidDuration
        );

        let clock = Clock::get()?;
        let vault = &mut ctx.accounts.vault;
        vault.locked_amount = amount;
        vault.locked_until = clock.unix_timestamp + lock_duration;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.vault_token_account.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            amount,
        )?;

        emit!(TokensLocked {
            vault: ctx.accounts.vault.key(),
            amount,
            locked_until: vault.locked_until
        });
        Ok(())
    }

    // Initialize vesting schedule
    pub fn initialize_vesting(
        ctx: Context<InitializeVesting>,
        amount: u64,
        start_time: i64,
        end_time: i64,
        target_market_cap: u64
    ) -> Result<()> {
        let clock = Clock::get()?;
        require!(start_time > clock.unix_timestamp, LaunchpadError::InvalidStartTime);
        require!(end_time > start_time, LaunchpadError::InvalidEndTime);
        let duration = end_time - start_time;
        require!(
            duration >= MINIMUM_VESTING_PERIOD && duration <= MAXIMUM_VESTING_PERIOD,
            LaunchpadError::InvalidDuration
        );
        require!(amount >= MINIMUM_AMOUNT, LaunchpadError::AmountTooLow);

        let vesting = &mut ctx.accounts.vesting;
        vesting.owner = ctx.accounts.owner.key();
        vesting.token_mint = ctx.accounts.token_mint.key();
        vesting.amount = amount;
        vesting.start_time = start_time;
        vesting.end_time = end_time;
        vesting.target_market_cap = target_market_cap;
        vesting.is_locked = true;
        vesting.bump = ctx.bumps.vesting;

        emit!(VestingInitialized {
            owner: vesting.owner,
            token_mint: vesting.token_mint,
            amount,
            start_time,
            end_time,
            target_market_cap
        });
        Ok(())
    }

    // Lock tokens for vesting
    pub fn lock_tokens_for_vesting(ctx: Context<LockTokensForVesting>) -> Result<()> {
        let vesting = &ctx.accounts.vesting;
        require!(vesting.is_locked, LaunchpadError::NotLocked);

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.owner_token_account.to_account_info(),
                    to: ctx.accounts.vesting_token_account.to_account_info(),
                    authority: ctx.accounts.owner.to_account_info(),
                },
            ),
            vesting.amount,
        )?;

        emit!(VestingTokensLocked {
            vesting: ctx.accounts.vesting.key(),
            amount: vesting.amount
        });
        Ok(())
    }

    // Unlock vested tokens
    pub fn unlock_vested_tokens(
        ctx: Context<UnlockVestedTokens>,
        current_market_cap: u64
    ) -> Result<()> {
        let vesting = &mut ctx.accounts.vesting;
        let clock = Clock::get()?;
        
        require!(vesting.is_locked, LaunchpadError::NotLocked);
        require!(clock.unix_timestamp >= vesting.end_time, LaunchpadError::VestingNotComplete);
        require!(
            current_market_cap >= vesting.target_market_cap,
            LaunchpadError::MarketCapNotReached
        );

        let seeds = &[
            b"vesting",
            vesting.token_mint.as_ref(),
            vesting.owner.as_ref(),
            &[vesting.bump],
        ];
        let signer = &[&seeds[..]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.vesting_token_account.to_account_info(),
                    to: ctx.accounts.owner_token_account.to_account_info(),
                    authority: ctx.accounts.vesting.to_account_info(),
                },
                signer,
            ),
            vesting.amount,
        )?;

        vesting.is_locked = false;

        emit!(TokensUnlocked {
            vesting: ctx.accounts.vesting.key(),
            amount: vesting.amount,
            market_cap: current_market_cap
        });
        Ok(())
    }

    // Buy tokens using bonding curve
    pub fn buy_tokens(
        ctx: Context<BuyTokens>,
        amount: u64,
        max_sol_cost: u64
    ) -> Result<()> {
        let bonding_curve = &mut ctx.accounts.bonding_curve;
        require!(!bonding_curve.complete, LaunchpadError::CurveComplete);

        let amm = AMM {
            virtual_sol_reserves: bonding_curve.virtual_sol_reserves,
            virtual_token_reserves: bonding_curve.virtual_token_reserves,
            real_sol_reserves: bonding_curve.real_sol_reserves,
            real_token_reserves: bonding_curve.real_token_reserves,
            initial_virtual_token_reserves: ctx.accounts.global.initial_virtual_token_reserves,
        };

        let buy_result = amm.apply_buy(amount.into());
        require!(buy_result.sol_amount <= max_sol_cost.into(), LaunchpadError::SlippageExceeded);

        // Update bonding curve state
        bonding_curve.virtual_sol_reserves = amm.virtual_sol_reserves;
        bonding_curve.virtual_token_reserves = amm.virtual_token_reserves;
        bonding_curve.real_sol_reserves = amm.real_sol_reserves;
        bonding_curve.real_token_reserves = amm.real_token_reserves;

        // Transfer SOL and tokens
        **ctx.accounts.user.lamports.borrow_mut() -= buy_result.sol_amount as u64;
        **ctx.accounts.bonding_curve.to_account_info().lamports.borrow_mut() += buy_result.sol_amount as u64;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.bonding_curve_token_account.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.bonding_curve.to_account_info(),
                },
            ),
            buy_result.token_amount.try_into()?,
        )?;

        emit!(TradeEvent {
            user: ctx.accounts.user.key(),
            mint: ctx.accounts.mint.key(),
            sol_amount: buy_result.sol_amount,
            token_amount: buy_result.token_amount,
            is_buy: true,
            timestamp: Clock::get()?.unix_timestamp,
        });
        Ok(())
    }

    // Launch a token with voice verification and optional bonding curve or PumpFun integration
    pub fn launch_token_with_voice(
        ctx: Context<LaunchTokenWithVoice>,
        name: String,
        symbol: String,
        uri: String,
        deepgram_transcript_id: [u8; 64],
        initial_price: u64,
        slope: u64,
        curve_type: u8,
        launch_type: u8,
    ) -> Result<()> {
        voice_launch::launch_token_with_voice(
            ctx,
            name,
            symbol,
            uri,
            deepgram_transcript_id,
            initial_price,
            slope,
            curve_type,
            launch_type,
        )
    }
    
    // Unlock tokens after market cap target is reached
    pub fn unlock_market_cap_vesting(ctx: Context<UnlockMarketCapVesting>) -> Result<()> {
        voice_launch::unlock_market_cap_vesting(ctx)
    }
    
    // Claim vested tokens after unlock
    pub fn claim_vested_tokens(ctx: Context<ClaimVestedTokens>) -> Result<()> {
        voice_launch::claim_vested_tokens(ctx)
    }
    
    // Mark a bonding curve as complete and ready for PumpFun
    pub fn complete_bonding_curve(ctx: Context<CompleteBondingCurve>) -> Result<()> {
        voice_launch::complete_bonding_curve(ctx)
    }
}

// Account structures
#[account]
pub struct Global {
    pub initialized: bool,
    pub authority: Pubkey,
    pub fee_recipient: Pubkey,
    pub initial_virtual_token_reserves: u64,
    pub initial_virtual_sol_reserves: u64,
    pub initial_real_token_reserves: u64,
    pub fee_basis_points: u64,
}

#[account]
pub struct Vault {
    pub owner: Pubkey,
    pub bump: u8,
    pub locked_amount: u64,
    pub locked_until: i64,
}

#[account]
pub struct Vesting {
    pub owner: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub start_time: i64,
    pub end_time: i64,
    pub target_market_cap: u64,
    pub is_locked: bool,
    pub bump: u8,
}

#[account]
pub struct BondingCurve {
    pub virtual_sol_reserves: u64,
    pub virtual_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub token_total_supply: u64,
    pub complete: bool,
}

// Context structs
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 1 + 32 + 32 + 8 + 8 + 8 + 8,
        seeds = [b"global"],
        bump
    )]
    pub global: Account<'info, Global>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub fee_recipient: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(
        init,
        payer = owner,
        space = 8 + 32 + 1 + 8 + 8,
        seeds = [b"vault", owner.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct LockTokens<'info> {
    #[account(mut, has_one = owner)]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault_token_account: Account<'info, TokenAccount>,
    pub authority: Signer<'info>,
    pub owner: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct InitializeVesting<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    pub token_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = owner,
        space = 8 + 32 + 32 + 8 + 8 + 8 + 8 + 1 + 1,
        seeds = [b"vesting", token_mint.key().as_ref(), owner.key().as_ref()],
        bump
    )]
    pub vesting: Account<'info, Vesting>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct LockTokensForVesting<'info> {
    #[account(mut, has_one = owner)]
    pub vesting: Account<'info, Vesting>,
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vesting_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct UnlockVestedTokens<'info> {
    #[account(mut, has_one = owner)]
    pub vesting: Account<'info, Vesting>,
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub vesting_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct BuyTokens<'info> {
    #[account()]
    pub global: Account<'info, Global>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint: Account<'info, Mint>,
    #[account(
        mut,
        seeds = [b"bonding-curve", mint.key().as_ref()],
        bump
    )]
    pub bonding_curve: Account<'info, BondingCurve>,
    #[account(mut)]
    pub bonding_curve_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct LaunchTokenWithVoice<'info> {
    // Add accounts and constraints for this instruction
}

#[derive(Accounts)]
pub struct UnlockMarketCapVesting<'info> {
    // Add accounts and constraints for this instruction
}

#[derive(Accounts)]
pub struct ClaimVestedTokens<'info> {
    // Add accounts and constraints for this instruction
}

#[derive(Accounts)]
pub struct CompleteBondingCurve<'info> {
    // Add accounts and constraints for this instruction
}

// Events
#[event]
pub struct LaunchpadInitialized {
    pub authority: Pubkey,
    pub fee_recipient: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct VaultInitialized {
    pub owner: Pubkey,
    pub vault: Pubkey,
}

#[event]
pub struct TokensLocked {
    pub vault: Pubkey,
    pub amount: u64,
    pub locked_until: i64,
}

#[event]
pub struct VestingInitialized {
    pub owner: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub start_time: i64,
    pub end_time: i64,
    pub target_market_cap: u64,
}

#[event]
pub struct VestingTokensLocked {
    pub vesting: Pubkey,
    pub amount: u64,
}

#[event]
pub struct TokensUnlocked {
    pub vesting: Pubkey,
    pub amount: u64,
    pub market_cap: u64,
}

#[event]
pub struct TradeEvent {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub sol_amount: u64,
    pub token_amount: u64,
    pub is_buy: bool,
    pub timestamp: i64,
}

// Errors
#[error_code]
pub enum LaunchpadError {
    #[msg("Amount below minimum required")]
    AmountTooLow,
    #[msg("Invalid duration specified")]
    InvalidDuration,
    #[msg("Vesting schedule not locked")]
    NotLocked,
    #[msg("Vesting period not complete")]
    VestingNotComplete,
    #[msg("Target market cap not reached")]
    MarketCapNotReached,
    #[msg("Invalid start time")]
    InvalidStartTime,
    #[msg("Invalid end time")]
    InvalidEndTime,
    #[msg("Bonding curve is complete")]
    CurveComplete,
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
}

// AMM implementation (simplified from original)
#[derive(Clone, Copy)]
pub struct AMM {
    pub virtual_sol_reserves: u64,
    pub virtual_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub initial_virtual_token_reserves: u64,
}

impl AMM {
    pub fn apply_buy(&self, token_amount: u64) -> BuyResult {
        let final_token_amount = if token_amount > self.real_token_reserves {
            self.real_token_reserves
        } else {
            token_amount
        };
        
        let product = self.virtual_sol_reserves * self.virtual_token_reserves;
        let new_virtual_token_reserves = self.virtual_token_reserves - final_token_amount;
        let new_virtual_sol_reserves = product / new_virtual_token_reserves + 1;
        let sol_amount = if new_virtual_sol_reserves > self.virtual_sol_reserves {
            new_virtual_sol_reserves - self.virtual_sol_reserves
        } else {
            0
        };

        BuyResult {
            token_amount: final_token_amount,
            sol_amount,
        }
    }
}

#[derive(Clone, Copy)]
pub struct BuyResult {
    pub token_amount: u64,
    pub sol_amount: u64,
}
