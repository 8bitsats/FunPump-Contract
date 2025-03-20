use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use solana_program::{program::invoke, system_program};
use crate::{state::{EnhancedBondingCurve, Vesting}, utils::pumpfun_integration::{MPL_TOKEN_METADATA_PROGRAM_ID, PumpFunLaunchType}};
use anchor_spl::metadata::{create_metadata_accounts_v3, mpl_token_metadata::types::DataV2, Metadata, CreateMetadataAccountsV3};

// Account structure for voice launches
#[account]
#[derive(InitSpace)]
pub struct VoiceLaunch {
    pub creator: Pubkey,                  // The creator of the token
    pub token_mint: Pubkey,               // The token mint address
    pub deepgram_transcript_id: [u8; 64],  // Deepgram transcript ID for verification
    pub launch_timestamp: i64,            // When the token was launched
    pub bump: u8,                         // Bump seed for PDA derivation
}

impl VoiceLaunch {
    pub const SEED_PREFIX: &'static [u8; 13] = b"voice-launch-";
}

#[derive(Accounts)]
pub struct LaunchTokenWithVoice<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    
    #[account(
        init,
        payer = creator,
        space = 8 + VoiceLaunch::INIT_SPACE,
        seeds = [VoiceLaunch::SEED_PREFIX, creator.key().as_ref()],
        bump
    )]
    pub voice_launch: Account<'info, VoiceLaunch>,
    
    // Token mint that will be created
    pub token_mint: Account<'info, Mint>,
    
    // Bonding curve account (conditionally initialized based on launch type)
    #[account(
        init_if_needed,
        payer = creator,
        space = 8 + EnhancedBondingCurve::INIT_SPACE,
        seeds = [EnhancedBondingCurve::SEED_PREFIX, token_mint.key().as_ref()],
        bump
    )]
    pub curve: Account<'info, EnhancedBondingCurve>,
    
    // Token account for the bonding curve (where tokens will be minted)
    #[account(mut)]
    pub curve_token_account: Account<'info, TokenAccount>,
    
    /// CHECK: Metadata account, validated in instruction
    #[account(mut)]
    pub metadata: UncheckedAccount<'info>,
    
    /// CHECK: Metadata program ID
    #[account(address = MPL_TOKEN_METADATA_PROGRAM_ID)]
    pub metadata_program: UncheckedAccount<'info>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UnlockMarketCapVesting<'info> {
    #[account(mut)]
    pub beneficiary: Signer<'info>,
    
    #[account(
        mut,
        seeds = [Vesting::SEED_PREFIX, vesting.token_mint.as_ref(), vesting.beneficiary.as_ref()],
        bump = vesting.bump,
        constraint = vesting.beneficiary == beneficiary.key() @ VestingError::Unauthorized,
    )]
    pub vesting: Account<'info, Vesting>,
    
    #[account(
        seeds = [EnhancedBondingCurve::SEED_PREFIX, vesting.token_mint.as_ref()],
        bump = curve.bump,
    )]
    pub curve: Account<'info, EnhancedBondingCurve>,
}

#[derive(Accounts)]
pub struct ClaimVestedTokens<'info> {
    #[account(mut)]
    pub beneficiary: Signer<'info>,
    
    #[account(
        mut,
        seeds = [Vesting::SEED_PREFIX, vesting.token_mint.as_ref(), vesting.beneficiary.as_ref()],
        bump = vesting.bump,
        constraint = vesting.beneficiary == beneficiary.key() @ VestingError::Unauthorized,
        constraint = vesting.is_unlocked @ VestingError::NotUnlocked,
    )]
    pub vesting: Account<'info, Vesting>,
    
    // The token mint
    pub token_mint: Account<'info, Mint>,
    
    // Vesting token account that holds tokens
    #[account(mut)]
    pub vesting_token_account: Account<'info, TokenAccount>,
    
    // Beneficiary token account to receive tokens
    #[account(mut)]
    pub beneficiary_token_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CompleteBondingCurve<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        mut,
        seeds = [EnhancedBondingCurve::SEED_PREFIX, curve.token_mint.as_ref()],
        bump = curve.bump,
        constraint = curve.authority == authority.key() @ CurveError::Unauthorized,
    )]
    pub curve: Account<'info, EnhancedBondingCurve>,
    
    pub system_program: Program<'info, System>,
}

// Instruction implementations
pub fn launch_token_with_voice(
    ctx: Context<LaunchTokenWithVoice>,
    name: String,
    symbol: String,
    uri: String,
    deepgram_transcript_id: [u8; 64],
    initial_price: u64,
    slope: u64,
    curve_type: u8,
    launch_type: u8, // 0 = Bonding Curve, 1 = PumpFun
) -> Result<()> {
    // Initialize voice launch account
    let voice_launch = &mut ctx.accounts.voice_launch;
    voice_launch.creator = ctx.accounts.creator.key();
    voice_launch.token_mint = ctx.accounts.token_mint.key();
    voice_launch.deepgram_transcript_id = deepgram_transcript_id;
    voice_launch.launch_timestamp = Clock::get()?.unix_timestamp;
    voice_launch.bump = ctx.bumps.voice_launch;

    // If bonding curve launch, initialize the curve
    if launch_type == PumpFunLaunchType::BondingCurve as u8 {
        let curve = &mut ctx.accounts.curve;
        curve.authority = ctx.accounts.creator.key();
        curve.token_mint = ctx.accounts.token_mint.key();
        curve.initial_price = initial_price;
        curve.slope = slope;
        curve.curve_type = curve_type;
        curve.total_supply = 0;
        curve.treasury_balance = 0;
        curve.virtual_sol_reserves = 1_000_000; // Default virtual reserves
        curve.virtual_token_reserves = 1_000_000_000; // Default virtual reserves
        curve.real_sol_reserves = 0;
        curve.real_token_reserves = 0;
        curve.is_complete = false;
        curve.launch_timestamp = Clock::get()?.unix_timestamp;
        curve.bump = ctx.bumps.curve;
    }

    // Set Token Metadata
    let token_data = DataV2 {
        name: name.clone(),
        symbol: symbol.clone(),
        uri: uri.clone(),
        seller_fee_basis_points: 0,
        creators: None,
        collection: None,
        uses: None,
    };

    // Create metadata accounts
    let metadata_infos = vec![
        ctx.accounts.metadata_program.to_account_info(),
        ctx.accounts.metadata.to_account_info(),
        ctx.accounts.token_mint.to_account_info(),
        ctx.accounts.creator.to_account_info(),
        ctx.accounts.system_program.to_account_info(),
        ctx.accounts.rent.to_account_info(),
    ];

    let metadata_ctx = CpiContext::new(
        ctx.accounts.metadata_program.to_account_info(),
        CreateMetadataAccountsV3 {
            metadata: ctx.accounts.metadata.to_account_info(),
            mint: ctx.accounts.token_mint.to_account_info(),
            mint_authority: ctx.accounts.creator.to_account_info(),
            payer: ctx.accounts.creator.to_account_info(),
            update_authority: ctx.accounts.creator.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            rent: ctx.accounts.rent.to_account_info(),
        },
    );

    create_metadata_accounts_v3(metadata_ctx, token_data, true, true, None)?;

    // Emit events based on launch type
    if launch_type == PumpFunLaunchType::DirectPumpFun as u8 {
        emit!(PumpFunLaunchEvent {
            creator: ctx.accounts.creator.key(),
            token_mint: ctx.accounts.token_mint.key(),
            timestamp: Clock::get()?.unix_timestamp,
            name: name.clone(),
            symbol: symbol.clone(),
        });
    } else {
        emit!(VoiceLaunchEvent {
            creator: ctx.accounts.creator.key(),
            token_mint: ctx.accounts.token_mint.key(),
            name: name.clone(),
            symbol: symbol.clone(),
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    Ok(())
}

pub fn unlock_market_cap_vesting(ctx: Context<UnlockMarketCapVesting>) -> Result<()> {
    let vesting = &mut ctx.accounts.vesting;
    let curve = &ctx.accounts.curve;

    // Check if market cap target has been reached
    let market_cap = curve.market_cap()?;
    require!(market_cap >= vesting.market_cap_target, VestingError::MarketCapTargetNotReached);

    vesting.is_unlocked = true;
    vesting.unlock_timestamp = Clock::get()?.unix_timestamp;

    emit!(VestingUnlockedEvent {
        beneficiary: vesting.beneficiary,
        token_mint: vesting.token_mint,
        market_cap,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

pub fn claim_vested_tokens(ctx: Context<ClaimVestedTokens>) -> Result<()> {
    let vesting = &mut ctx.accounts.vesting;
    let clock = Clock::get()?;
    
    // Calculate claimable amount based on vesting schedule
    let claimable = vesting.claimable_amount(clock.unix_timestamp);
    require!(claimable > 0, VestingError::NothingToClaim);

    // Transfer tokens from vesting account to beneficiary
    // We would need to setup proper account authorities and token transfers here
    // For example:
    /*
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.vesting_token_account.to_account_info(),
                to: ctx.accounts.beneficiary_token_account.to_account_info(),
                authority: ctx.accounts.vesting.to_account_info(),
            },
            &[&[Vesting::SEED_PREFIX, vesting.token_mint.as_ref(), vesting.beneficiary.as_ref(), &[vesting.bump]]],
        ),
        claimable,
    )?;
    */

    emit!(TokensClaimedEvent {
        beneficiary: vesting.beneficiary,
        token_mint: vesting.token_mint,
        amount: claimable,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

pub fn complete_bonding_curve(ctx: Context<CompleteBondingCurve>) -> Result<()> {
    let curve = &mut ctx.accounts.curve;
    curve.is_complete = true;

    emit!(BondingCurveCompletedEvent {
        authority: curve.authority,
        token_mint: curve.token_mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    // Signal PumpFun that token is ready (optional)
    emit!(PumpFunReadyEvent {
        token_mint: curve.token_mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

// Events
#[event]
pub struct VoiceLaunchEvent {
    pub creator: Pubkey,
    pub token_mint: Pubkey,
    pub name: String,
    pub symbol: String,
    pub timestamp: i64,
}

#[event]
pub struct PumpFunLaunchEvent {
    pub creator: Pubkey,
    pub token_mint: Pubkey,
    pub name: String,
    pub symbol: String,
    pub timestamp: i64,
}

#[event]
pub struct VestingUnlockedEvent {
    pub beneficiary: Pubkey,
    pub token_mint: Pubkey,
    pub market_cap: u64,
    pub timestamp: i64,
}

#[event]
pub struct TokensClaimedEvent {
    pub beneficiary: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct BondingCurveCompletedEvent {
    pub authority: Pubkey,
    pub token_mint: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct PumpFunReadyEvent {
    pub token_mint: Pubkey,
    pub timestamp: i64,
}

// Errors
#[error_code]
pub enum VestingError {
    #[msg("Unauthorized access to vesting account")]
    Unauthorized,
    
    #[msg("Market cap target not reached")]
    MarketCapTargetNotReached,
    
    #[msg("Vesting period not complete")]
    VestingNotComplete,
    
    #[msg("Vesting is not unlocked")]
    NotUnlocked,
    
    #[msg("Nothing to claim at this time")]
    NothingToClaim,
}

#[error_code]
pub enum CurveError {
    #[msg("Unauthorized access to curve account")]
    Unauthorized,
    
    #[msg("Invalid curve type")]
    InvalidCurveType,
}
