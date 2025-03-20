use anchor_lang::prelude::*;
use std::fmt;

#[account]
#[derive(InitSpace)]
pub struct Vesting {
    pub beneficiary: Pubkey,                // Account that will receive tokens after vesting
    pub token_mint: Pubkey,                 // The token mint address
    pub amount: u64,                        // Amount of tokens being vested
    pub start_timestamp: i64,               // When vesting begins
    pub end_timestamp: i64,                 // When vesting is complete
    pub market_cap_target: u64,             // Target market cap for unlocking
    pub is_unlocked: bool,                  // Whether the market cap target has been reached
    pub unlock_timestamp: i64,              // When the vesting was unlocked
    pub creator: Pubkey,                    // The creator of the vesting schedule
    pub bump: u8,                           // Bump seed for PDA derivation
}

impl Vesting {
    pub const SEED_PREFIX: &'static [u8; 8] = b"vesting-";
    
    // Calculate how many tokens can be claimed based on time elapsed
    pub fn claimable_amount(&self, current_timestamp: i64) -> u64 {
        if !self.is_unlocked {
            return 0; // Market cap target not reached
        }

        // If after end date, everything is claimable
        if current_timestamp >= self.end_timestamp {
            return self.amount;
        }
        
        // If before start date, nothing is claimable
        if current_timestamp <= self.start_timestamp {
            return 0;
        }
        
        // Linear vesting between start and end
        let total_period = self.end_timestamp - self.start_timestamp;
        let elapsed = current_timestamp - self.start_timestamp;
        
        // Calculate proportional amount (using saturating_mul/div to prevent overflow)
        self.amount
            .saturating_mul(elapsed as u64)
            .saturating_div(total_period as u64)
    }
}

impl fmt::Display for Vesting {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Vesting {{ beneficiary: {}, token_mint: {}, amount: {}, start: {}, end: {}, market_cap_target: {}, is_unlocked: {}, unlock_timestamp: {} }}",
            self.beneficiary,
            self.token_mint,
            self.amount,
            self.start_timestamp,
            self.end_timestamp,
            self.market_cap_target,
            self.is_unlocked,
            self.unlock_timestamp
        )
    }
}
