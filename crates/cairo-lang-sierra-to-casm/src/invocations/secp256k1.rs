use cairo_lang_sierra::extensions::secp256k1::Secp256K1EcConcreteLibfunc;

use super::starknet::build_syscalls;
use super::{CompiledInvocation, CompiledInvocationBuilder, InvocationError};

/// Builds instructions for Sierra secp256k1 operations.
pub fn build(
    libfunc: &Secp256K1EcConcreteLibfunc,
    builder: CompiledInvocationBuilder<'_>,
) -> Result<CompiledInvocation, InvocationError> {
    // TODO(yg): Is the size of point really 1? If not, fix wherever needed.
    match libfunc {
        Secp256K1EcConcreteLibfunc::Add(_) => {
            build_syscalls(builder, "Secp256K1EcAdd", [1, 1], [1])
        }
        Secp256K1EcConcreteLibfunc::Mul(_) => {
            build_syscalls(builder, "Secp256K1EcMul", [1, 1], [1])
        }
    }
}
