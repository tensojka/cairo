// This module contains functions and constructs related to elliptic curve operations on the
// secp256k1 curve.

use starknet::SyscallResult;

#[derive(Copy, Drop)]
extern type Secp256K1EcPoint;
// TODO(yg): use or delete.
type NonZeroSecp256K1EcPoint = NonZero<Secp256K1EcPoint>;

/// Computes the product of a secp256k1 EC point `p` by the given scalar `m`.
extern fn secp256k1_ec_mul_syscall(
    p: Secp256K1EcPoint,
    m: felt252
) -> SyscallResult<Secp256K1EcPoint> implicits(GasBuiltin, System) nopanic;

/// Computes the addition of secp256k1 EC points `p0 + p1`.
extern fn secp256k1_ec_add_syscall(
    p0: Secp256K1EcPoint,
    p1: Secp256K1EcPoint
) -> SyscallResult<Secp256K1EcPoint> implicits(GasBuiltin, System) nopanic;
