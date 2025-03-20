use anchor_lang::prelude::*;

// Definition for curve types
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum CurveType {
    Linear = 0,
    Quadratic = 1,
    Exponential = 2,
}

#[account]
pub struct Curve {
    pub curve_type: u8,  // 0 = Linear, 1 = Quadratic, 2 = Exponential
    pub initial_price: u64,
    pub slope: u64,
    pub creator: Pubkey,
    pub bump: u8,
}

impl Curve {
    pub const SEED_PREFIX: &'static [u8; 6] = b"curve-";
}
