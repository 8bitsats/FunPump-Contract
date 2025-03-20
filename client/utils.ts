import * as anchor from "@coral-xyz/anchor";
import type { TokenLaunchProgram } from "../target/types/token_launch_program";

// Configure the client to use the local cluster
anchor.setProvider(anchor.AnchorProvider.env());

const program = anchor.workspace.TokenLaunchProgram as anchor.Program<TokenLaunchProgram>;

export const calculateFee = (amount: bigint, fee: number): bigint => {
  return (amount * BigInt(fee)) / 10000n;
};