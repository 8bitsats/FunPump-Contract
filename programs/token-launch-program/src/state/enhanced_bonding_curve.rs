use anchor_lang::prelude::*;
use std::fmt;

#[account]
#[derive(InitSpace)]
pub struct EnhancedBondingCurve {
    pub authority: Pubkey,                // The authority that can complete/modify the curve
    pub token_mint: Pubkey,               // The token mint address
    pub initial_price: u64,               // Initial price of the token in lamports
    pub slope: u64,                       // Slope parameter for curve calculations
    pub curve_type: u8,                   // Type of curve (0=linear, 1=exponential, 2=logarithmic, 3=sigmoid)
    pub total_supply: u64,                // Total token supply
    pub treasury_balance: u64,            // Balance in the treasury
    pub virtual_sol_reserves: u64,        // Virtual SOL reserves for price calculation
    pub virtual_token_reserves: u64,      // Virtual token reserves for price calculation
    pub real_sol_reserves: u64,           // Real SOL reserves from buys/sells
    pub real_token_reserves: u64,         // Real token reserves from buys/sells
    pub is_complete: bool,                // Whether curve is complete (ready for PumpFun)
    pub launch_timestamp: i64,            // When the token was launched
    pub bump: u8,                         // Bump seed for PDA derivation
}

impl EnhancedBondingCurve {
    pub const SEED_PREFIX: &'static [u8; 6] = b"curve-";
    
    // Calculate current price based on curve type
    pub fn current_price(&self) -> Result<u64> {
        match self.curve_type {
            0 => Ok(self.initial_price.saturating_add( // Linear
                self.total_supply.saturating_mul(self.slope).saturating_div(1_000_000)
            )),
            1 => { // Exponential
                let exp_factor = self.total_supply.saturating_mul(self.slope).saturating_div(1_000_000);
                Ok(self.initial_price.saturating_mul(exp_factor.saturating_add(1_000_000)).saturating_div(1_000_000))
            },
            2 => { // Logarithmic
                // Simplified logarithmic implementation
                let log_factor = ((self.total_supply as f64).ln() as u64).saturating_mul(self.slope).saturating_div(1_000_000);
                Ok(self.initial_price.saturating_add(log_factor))
            },
            3 => { // Sigmoid
                let x = self.total_supply.saturating_mul(self.slope).saturating_div(1_000_000);
                let sigmoid = 1_000_000_000.saturating_div(1_000_000.saturating_add(x.saturating_mul(x)));
                Ok(self.initial_price.saturating_mul(sigmoid).saturating_div(1_000_000))
            },
            _ => Err(ProgramError::InvalidArgument.into()),
        }
    }
    
    // Calculate market cap
    pub fn market_cap(&self) -> Result<u64> {
        let price = self.current_price()?;
        Ok(price.saturating_mul(self.total_supply))
    }
    
    // Calculate tokens out for a given SOL amount (buy)
    pub fn calculate_tokens_out(&self, sol_amount: u64) -> Result<u64> {
        // Calculate based on curve type
        // For simplicity, we'll use a constant price formula
        let price = self.current_price()?;
        if price == 0 {
            return Err(ProgramError::InvalidArgument.into());
        }
        
        // Formula: tokens = sol_amount / price
        Ok(sol_amount.saturating_mul(1_000_000).saturating_div(price))
    }
    
    // Calculate SOL out for a given token amount (sell)
    pub fn calculate_sol_out(&self, token_amount: u64) -> Result<u64> {
        // Calculate based on curve type
        // For simplicity, we'll use a constant price formula
        let price = self.current_price()?;
        
        // Formula: sol = token_amount * price
        Ok(token_amount.saturating_mul(price).saturating_div(1_000_000))
    }
}

impl fmt::Display for EnhancedBondingCurve {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "EnhancedBondingCurve {{ authority: {}, token_mint: {}, initial_price: {}, \
            slope: {}, curve_type: {}, total_supply: {}, treasury_balance: {}, \
            is_complete: {}, launch_timestamp: {} }}",
            self.authority,
            self.token_mint,
            self.initial_price,
            self.slope,
            self.curve_type,
            self.total_supply,
            self.treasury_balance,
            self.is_complete,
            self.launch_timestamp
        )
    }
}
