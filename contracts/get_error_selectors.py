from web3 import Web3

# List of Solidity error signatures
error_signatures = [
    "INVALID_VERIFICATION_KEY()",
    "POINT_NOT_ON_CURVE()",
    "PUBLIC_INPUT_COUNT_INVALID(uint256,uint256)",
    "PUBLIC_INPUT_INVALID_BN128_G1_POINT()",
    "PUBLIC_INPUT_GE_P()",
    "MOD_EXP_FAILURE()",
    "PAIRING_PREAMBLE_FAILED()",
    "OPENING_COMMITMENT_FAILED()",
    "PAIRING_FAILED()",
    "DepositTooLow()",
    "DepositTooHigh()",
    "DepositFailed()",
    "exchangeRateZero()",
    "WithdrawFailed()",
    "LpDoesntExist()",
    "NotVaultOwner()",
    "TooManyLps()",
    "NotEnoughLiquidity()",
    "ReservationAmountTooLow()",
    "InvalidOrder()",
    "NotEnoughLiquidityConsumed()",
    "LiquidityReserved(uint256)",
    "LiquidityNotReserved()",
    "InvalidLpIndex()",
    "NoLiquidityToReserve()",
    "OrderComplete()",
    "ReservationFeeTooLow()",
    "InvalidVaultIndex()",
    "WithdrawalAmountError()",
    "InvalidEthereumAddress()",
    "InvalidBitcoinAddress()",
    "InvalidProof()",
    "InvaidSameExchangeRatevaultIndex()",
    "InvalidVaultUpdate()",
    "ReservationNotExpired()",
    "InvalidUpdateWithActiveReservations()",
    "StillInChallengePeriod()",
    "ReservationNotUnlocked()",
    "InvalidExchangeRate()",
    "NotVaultOwner()",
    "NotEnoughLiquidity()",
    "WithdrawalAmountError()",
    "UpdateExchangeRateError()",
    "ReservationNotExpired()",
    "ReservationNotProved()",
    "StillInChallengePeriod()",
    "OverwrittenProposedBlock()",
    "NewDepositsPaused()",
    "InvalidInputArrays()",
    "InvalidReservationState()",
    "AmountToReserveTooLow(uint256 index)",
    "InvalidExchangeRate()",
    "NotVaultOwner()",
    "NotEnoughLiquidity()",
    "WithdrawalAmountError()",
    "UpdateExchangeRateError()",
    "ReservationNotExpired()",
    "ReservationNotProved()",
    "StillInChallengePeriod()",
    "OverwrittenProposedBlock()",
    "NewDepositsPaused()",
    "InvalidInputArrays()",
    "InvalidReservationState()",
    "NotApprovedHypernode()",
    "AmountToReserveTooLow(uint256 index)",
]

def get_error_selector(error_signature):
    # Compute the keccak-256 hash of the error signature
    hashed = Web3.keccak(text=error_signature)
    # Return the first 4 bytes of the hash, converted to a hexadecimal string
    return hashed.hex()[:10]  # Includes '0x' prefix

# Calculate the selectors for each error signature
error_selectors = {error: get_error_selector(error) for error in error_signatures}

for error, selector in error_selectors.items():
    print(f"{error} -> {selector}")

