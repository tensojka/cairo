use super::felt252::Felt252Type;
use super::starknet::syscalls::SyscallGenericLibfunc;
use crate::define_libfunc_hierarchy;
use crate::extensions::lib_func::SignatureSpecializationContext;
use crate::extensions::{NamedType, NoGenericArgsGenericType, SpecializationError};
use crate::ids::GenericTypeId;

define_libfunc_hierarchy! {
    pub enum Secp256K1EcLibfunc {
         Add(Secp256K1EcAddLibfunc),
         Mul(Secp256K1EcMulLibfunc),
    }, Secp256K1EcConcreteLibfunc
}

#[derive(Default)]
pub struct Secp256K1EcPointType {}
impl NoGenericArgsGenericType for Secp256K1EcPointType {
    const ID: GenericTypeId = GenericTypeId::new_inline("Secp256K1EcPoint");
    const STORABLE: bool = true;
    const DUPLICATABLE: bool = true;
    const DROPPABLE: bool = true;
    // TODO(yg): Is the size really 1? If not, fix wherever needed.
    const SIZE: i16 = 1;
}

/// Libfunc for a secp256k1 elliptic curve multiplication system call.
#[derive(Default)]
pub struct Secp256K1EcMulLibfunc {}
impl SyscallGenericLibfunc for Secp256K1EcMulLibfunc {
    const STR_ID: &'static str = "secp256k1_ec_mul_syscall";

    fn input_tys(
        context: &dyn SignatureSpecializationContext,
    ) -> Result<Vec<crate::ids::ConcreteTypeId>, SpecializationError> {
        Ok(vec![
            // Point `p`.
            context.get_concrete_type(Secp256K1EcPointType::id(), &[])?,
            // Scalar `m`.
            context.get_concrete_type(Felt252Type::id(), &[])?,
        ])
    }

    fn success_output_tys(
        context: &dyn SignatureSpecializationContext,
    ) -> Result<Vec<crate::ids::ConcreteTypeId>, SpecializationError> {
        Ok(vec![context.get_concrete_type(Secp256K1EcPointType::id(), &[])?])
    }
}

/// Libfunc for a secp256k1 elliptic curve addition system call.
#[derive(Default)]
pub struct Secp256K1EcAddLibfunc {}
impl SyscallGenericLibfunc for Secp256K1EcAddLibfunc {
    const STR_ID: &'static str = "secp256k1_ec_add_syscall";

    fn input_tys(
        context: &dyn SignatureSpecializationContext,
    ) -> Result<Vec<crate::ids::ConcreteTypeId>, SpecializationError> {
        Ok(vec![
            // Point `p0`.
            context.get_concrete_type(Secp256K1EcPointType::id(), &[])?,
            // Point `p1`.
            context.get_concrete_type(Secp256K1EcPointType::id(), &[])?,
        ])
    }

    fn success_output_tys(
        context: &dyn SignatureSpecializationContext,
    ) -> Result<Vec<crate::ids::ConcreteTypeId>, SpecializationError> {
        Ok(vec![context.get_concrete_type(Secp256K1EcPointType::id(), &[])?])
    }
}
