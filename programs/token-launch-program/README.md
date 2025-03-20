# PumpFun Token Launchpad

A Solana smart contract for launching tokens with advanced features including bonding curves, vesting with market cap unlocks, and PumpFun integration.

## Features

1. **Bonding Curve Implementation**
   - Multiple curve types (linear, exponential, logarithmic, sigmoid)
   - Flexible pricing based on token supply
   - Buy and sell functionality

2. **Vesting with Market Cap Unlocks**
   - Lock tokens until a market cap threshold is reached
   - Linear vesting after unlock
   - Configurable vesting periods

3. **Token Metadata Integration**
   - Uses Metaplex Token Metadata program
   - Sets token name, symbol, and URI during launch

4. **PumpFun Integration**
   - Option to launch tokens directly to PumpFun
   - Signal completion for external integration

5. **Voice Launch**
   - Token launch with voice verification
   - Integration with Deepgram for transcript verification

## Launch Options

### 1. Bonding Curve Launch

Launches tokens with a bonding curve that enables price discovery:

```bash
launchtokenwithvoice(
  name, symbol, uri, deepgram_transcript_id, 
  initial_price, slope, curve_type, 
  launch_type=0 # Bonding Curve
)
```

### 2. PumpFun Launch

Prepares tokens for launch on PumpFun:

```bash
launchtokenwithvoice(
  name, symbol, uri, deepgram_transcript_id, 
  initial_price, slope, curve_type, 
  launch_type=1 # PumpFun
)
```

## Vesting and Market Cap Unlocks

After creating a vesting schedule, tokens will be unlocked when the market cap (current price × total supply) reaches the target:

```bash
unlock_market_cap_vesting()
```

Then tokens can be claimed according to the vesting schedule:

```bash
claim_vested_tokens()
```

## Program IDs

- **Program ID**: `VoiceLaunch11111111111111111111111111111111` (placeholder)
- **PumpFun Program ID**: `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`
- **Token Metadata Program ID**: `metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s`

## Usage

The program can be interacted with using the provided TypeScript SDK or direct program calls.

## Security Considerations

- The program uses PDA signing for secure token transfers
- Overflow protection with saturating math operations
- Authority validation for sensitive operations

## Testing

Refer to `TESTING.md` for comprehensive testing options including:
- Simple mock testing
- Devnet testing with test keypairs
- Mainnet deployment guidelines
