#![allow(unexpected_cfgs)]
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

// Use a proper base58 program ID (32 bytes)
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod complete_solana_project {
    use super::*;

    pub fn initialize_vault(ctx: Context<InitializeVault>, _bump: u8) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        vault.owner = *ctx.accounts.payer.key;
        vault.bump = _bump;
        Ok(())
    }

    pub fn lock_tokens(ctx: Context<LockTokens>, amount: u64, lock_duration: i64) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        let unlock_time = current_time + lock_duration;

        let cpi_accounts = token::Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        ctx.accounts.vault.locked_until = unlock_time;
        ctx.accounts.vault.locked_amount = amount;

        Ok(())
    }

    pub fn unlock_tokens(ctx: Context<UnlockTokens>) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        let vault = &ctx.accounts.vault;

        require!(current_time >= vault.locked_until, CustomError::TokensStillLocked);

        // Store the amount and bump before mutating anything
        let amount = vault.locked_amount;
        let bump = vault.bump;
        let vault_authority = ctx.accounts.vault.to_account_info();
        
        let cpi_accounts = token::Transfer {
            from: ctx.accounts.vault_token_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: vault_authority,
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let seeds = &[b"vault", ctx.accounts.authority.key.as_ref(), &[bump]];
        let signer = &[&seeds[..]];
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::transfer(cpi_ctx, amount)?;

        // Now mutate the vault
        let vault = &mut ctx.accounts.vault;
        vault.locked_amount = 0;
        Ok(())
    }

    pub fn initialize_launch(
        ctx: Context<InitializeLaunch>,
        total_supply: u64,
        curve_type: u8,
        custom_params: [u64; 3],
    ) -> Result<()> {
        let curve = &mut ctx.accounts.curve;
        curve.creator = ctx.accounts.creator.key();
        curve.mint = ctx.accounts.mint.key();
        curve.total_supply = total_supply;
        curve.reserve_token = total_supply;
        curve.reserve_sol = 0;
        curve.curve_type = curve_type;
        curve.custom_params = custom_params;
        curve.bump = *ctx.bumps.get("curve").unwrap();
        Ok(())
    }

    pub fn buy_tokens(ctx: Context<BuyTokens>, amount: u64) -> Result<()> {
        let curve = &mut ctx.accounts.curve;
        let tokens_out = calculate_tokens_out(curve, amount)?;

        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.buyer.to_account_info(),
                to: ctx.accounts.sol_vault.to_account_info(),
            },
        );
        anchor_lang::system_program::transfer(cpi_context, amount)?;

        let seeds = &[b"curve".as_ref(), &ctx.accounts.mint.key().to_bytes(), &[curve.bump]];
        let signer = &[&seeds[..]];
        let cpi_accounts = token::Transfer {
            from: ctx.accounts.token_vault.to_account_info(),
            to: ctx.accounts.buyer_token_account.to_account_info(),
            authority: curve.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::transfer(cpi_ctx, tokens_out)?;

        curve.reserve_token -= tokens_out;
        curve.reserve_sol += amount;

        Ok(())
    }

    pub fn sell_tokens(ctx: Context<SellTokens>, amount: u64) -> Result<()> {
        let curve = &mut ctx.accounts.curve;
        let sol_out = calculate_sol_out(curve, amount)?;

        let cpi_accounts = token::Transfer {
            from: ctx.accounts.seller_token_account.to_account_info(),
            to: ctx.accounts.token_vault.to_account_info(),
            authority: ctx.accounts.seller.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        let seeds = &[b"curve".as_ref(), &ctx.accounts.mint.key().to_bytes(), &[curve.bump]];
        let signer = &[&seeds[..]];
        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.sol_vault.to_account_info(),
                to: ctx.accounts.seller.to_account_info(),
            },
            signer,
        );
        anchor_lang::system_program::transfer(cpi_context, sol_out)?;

        curve.reserve_token += amount;
        curve.reserve_sol -= sol_out;

        Ok(())
    }

    pub fn initialize_vesting(
        ctx: Context<InitializeVesting>,
        amount: u64,
        start_time: i64,
        end_time: i64,
        target_market_cap: u64,
    ) -> Result<()> {
        let vesting = &mut ctx.accounts.vesting;
        vesting.owner = ctx.accounts.owner.key();
        vesting.token_mint = ctx.accounts.token_mint.key();
        vesting.amount = amount;
        vesting.start_time = start_time;
        vesting.end_time = end_time;
        vesting.target_market_cap = target_market_cap;
        vesting.is_locked = true;
        vesting.bump = *ctx.bumps.get("vesting").unwrap();
        Ok(())
    }

    pub fn lock_tokens_for_vesting(ctx: Context<LockTokensForVesting>, amount: u64) -> Result<()> {
        let vesting = &mut ctx.accounts.vesting;
        require!(amount == vesting.amount, CustomError::InvalidVestingAmount);

        // Create the transfer context directly
        let cpi_accounts = token::Transfer {
            from: ctx.accounts.owner_token_account.to_account_info(),
            to: ctx.accounts.vesting_token_account.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        Ok(())
    }

    pub fn unlock_vested_tokens(ctx: Context<UnlockVestedTokens>, current_market_cap: u64) -> Result<()> {
        let vesting = &mut ctx.accounts.vesting;
        let current_time = Clock::get()?.unix_timestamp;

        require!(vesting.is_locked, CustomError::TokensAlreadyUnlocked);
        require!(current_time >= vesting.end_time, CustomError::VestingPeriodNotEnded);
        require!(current_market_cap >= vesting.target_market_cap, CustomError::MarketCapNotReached);

        let seeds = &[
            b"vesting".as_ref(),
            &vesting.token_mint.to_bytes(),
            &vesting.owner.to_bytes(),
            &[vesting.bump],
        ];
        let signer = &[&seeds[..]];
        let cpi_accounts = token::Transfer {
            from: ctx.accounts.vesting_token_account.to_account_info(),
            to: ctx.accounts.owner_token_account.to_account_info(),
            authority: vesting.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::transfer(cpi_ctx, vesting.amount)?;

        vesting.is_locked = false;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(init, payer = payer, space = 8 + Vault::LEN)]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct LockTokens<'info> {
    #[account(mut)]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UnlockTokens<'info> {
    #[account(mut)]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct InitializeLaunch<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(
        init,
        payer = creator,
        space = 8 + 32 + 32 + 8 + 8 + 8 + 1 + 24 + 1,
        seeds = [b"curve", mint.key().as_ref()],
        bump
    )]
    pub curve: Account<'info, Curve>,
    pub mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct BuyTokens<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,
    #[account(mut)]
    pub curve: Account<'info, Curve>,
    #[account(mut)]
    pub sol_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub buyer_token_account: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct SellTokens<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,
    #[account(mut)]
    pub curve: Account<'info, Curve>,
    #[account(mut)]
    pub sol_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub seller_token_account: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
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
        space = 8 + Vesting::LEN,
        seeds = [b"vesting", token_mint.key().as_ref(), owner.key().as_ref()],
        bump
    )]
    pub vesting: Account<'info, Vesting>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct LockTokensForVesting<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut)]
    pub vesting: Account<'info, Vesting>,
    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vesting_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct UnlockVestedTokens<'info> {
    pub owner: Signer<'info>,
    #[account(mut)]
    pub vesting: Account<'info, Vesting>,
    #[account(mut)]
    pub vesting_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct Vault {
    pub owner: Pubkey,
    pub bump: u8,
    pub locked_amount: u64,
    pub locked_until: i64,
}

#[account]
pub struct Curve {
    pub creator: Pubkey,
    pub mint: Pubkey,
    pub total_supply: u64,
    pub reserve_token: u64,
    pub reserve_sol: u64,
    pub curve_type: u8,
    pub custom_params: [u64; 3],
    pub bump: u8,
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

impl Vault {
    pub const LEN: usize = 32 + 1 + 8 + 8;
}

impl Vesting {
    pub const LEN: usize = 32 + 32 + 8 + 8 + 8 + 8 + 1 + 1;
}

#[error_code]
pub enum CustomError {
    #[msg("Tokens are still locked")]
    TokensStillLocked,
    #[msg("Invalid vesting amount")]
    InvalidVestingAmount,
    #[msg("Tokens are already unlocked")]
    TokensAlreadyUnlocked,
    #[msg("Vesting period has not ended")]
    VestingPeriodNotEnded,
    #[msg("Market cap target not reached")]
    MarketCapNotReached,
}

fn calculate_tokens_out(curve: &Curve, sol_amount: u64) -> Result<u64> {
    let tokens_out = match curve.curve_type {
        0 => { // Linear curve
            sol_amount.saturating_mul(curve.custom_params[0]).saturating_div(1000)
        },
        1 => { // Quadratic curve
            let quadratic_component = sol_amount.saturating_mul(curve.custom_params[1]).saturating_div(10000);
            sol_amount.saturating_mul(curve.custom_params[0].saturating_add(quadratic_component)).saturating_div(1000)
        },
        2 => { // Exponential curve
            let base_tokens = sol_amount.saturating_mul(curve.custom_params[0]).saturating_div(1000);
            let supply_factor = curve.reserve_token.saturating_mul(curve.custom_params[1]).saturating_div(curve.total_supply);
            base_tokens.saturating_mul(1000 + supply_factor).saturating_div(1000)
        },
        _ => return Err(ProgramError::InvalidArgument.into())
    };
    
    if tokens_out > curve.reserve_token {
        return Err(ProgramError::InsufficientFunds.into());
    }
    
    Ok(tokens_out)
}

fn calculate_sol_out(curve: &Curve, token_amount: u64) -> Result<u64> {
    let sol_out = match curve.curve_type {
        0 => { // Linear curve
            token_amount.saturating_mul(1000).saturating_div(curve.custom_params[0])
        },
        1 => { // Quadratic curve
            let base_sol = token_amount.saturating_mul(1000).saturating_div(curve.custom_params[0]);
            base_sol.saturating_mul(950).saturating_div(1000) // Apply a 5% slippage
        },
        2 => { // Exponential curve
            let supply_factor = curve.reserve_token.saturating_mul(curve.custom_params[1]).saturating_div(curve.total_supply);
            let adjusted_tokens = token_amount.saturating_mul(1000).saturating_div(1000 + supply_factor);
            adjusted_tokens.saturating_mul(1000).saturating_div(curve.custom_params[0])
        },
        _ => return Err(ProgramError::InvalidArgument.into())
    };
    
    if sol_out > curve.reserve_sol {
        return Err(ProgramError::InsufficientFunds.into());
    }
    
    Ok(sol_out)
}
