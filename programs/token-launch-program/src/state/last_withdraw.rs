use anchor_lang::prelude::*;

#[account]
pub struct LastWithdraw {
    pub owner: Pubkey,
    pub timestamp: i64,
    pub bump: u8,
}
