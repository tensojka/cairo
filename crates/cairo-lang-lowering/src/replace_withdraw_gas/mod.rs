use cairo_lang_defs::diagnostic_utils::StableLocationOption;
use cairo_lang_diagnostics::Maybe;
use cairo_lang_semantic::corelib::{
    core_submodule, core_withdraw_gas, get_function_id, get_ty_by_name,
};

use crate::db::LoweringGroup;
use crate::ids::{ConcreteFunctionWithBodyId, FunctionLongId, SemanticFunctionIdEx};
use crate::lower::context::{VarRequest, VariableAllocator};
use crate::{FlatBlockEnd, FlatLowered, MatchExternInfo, MatchInfo, Statement, StatementCall};

/// Main function for the replace_withdraw_gas lowering phase. Replaces `withdraw_gas` calls with
/// `withdraw_gas_all` calls where necessary.
pub fn replace_withdraw_gas(
    db: &dyn LoweringGroup,
    function: ConcreteFunctionWithBodyId,
    lowered: &mut FlatLowered,
) -> Maybe<()> {
    for block in lowered.blocks.iter_mut() {
        let info = match &block.end {
            FlatBlockEnd::Match { info: MatchInfo::Extern(info) } => {
                match db.lookup_intern_lowering_function(info.function) {
                    FunctionLongId::AutoWithdrawGas => info,
                    _ => break,
                }
            }
            _ => break,
        };

        if needs_withdraw_gas_all(db, function)? {
            replace_block_to_withdraw_gas_all(db, function, lowered, info, block)?;
        } else {
            block.end = FlatBlockEnd::Match {
                info: MatchInfo::Extern(MatchExternInfo {
                    function: core_withdraw_gas(db.upcast()).lowered(db),
                    ..info.clone()
                }),
            }
        }
    }

    Ok(())
}

// TODO(yg): doc, implement.
// TODO(yg): Compute direct-implicits (implicits used by this SCC without dependent SCCs). Return true here if your direct implicits includes a builtin. Document why this is good.
fn needs_withdraw_gas_all(db: &dyn LoweringGroup, function: ConcreteFunctionWithBodyId) -> Maybe<bool> {
    // TODO(yg): call db.function_implicits(function) instead. This is not a query.
    let my_implicits = db.function_implicits(function)?;
    // TODO(yg): if it contains a builtin
    Ok(my_implicits.contains(...))
}

/// Replaces a block ending with a match-extern of `withdraw_gas` to call `withdraw_gas_all`.
fn replace_block_to_withdraw_gas_all(
    db: &dyn LoweringGroup,
    function: ConcreteFunctionWithBodyId,
    lowered: &mut FlatLowered,
    info: &MatchExternInfo,
    block: &mut crate::FlatBlock,
) -> Maybe<()> {
    let gas_module = core_submodule(db.upcast(), "gas");

    // Add variable of type BuiltinCosts.
    let mut variables = VariableAllocator::new(
        db,
        function.function_with_body_id(db).base_semantic_function(db),
        lowered.variables.clone(),
    )?;
    let builtin_costs_var = variables.new_var(VarRequest {
        ty: get_ty_by_name(db.upcast(), gas_module, "BuiltinCosts".into(), Vec::new()),
        location: StableLocationOption::None,
    });
    lowered.variables = variables.variables;

    // Add a statement call to `get_builtin_costs`.
    block.statements.push(Statement::Call(StatementCall {
        function: get_function_id(db.upcast(), gas_module, "get_builtin_costs".into(), vec![])
            .lowered(db),
        inputs: vec![],
        outputs: vec![builtin_costs_var],
        location: StableLocationOption::None,
    }));

    // Modify block end to call `withdraw_gas_all`.
    block.end = FlatBlockEnd::Match {
        info: MatchInfo::Extern(MatchExternInfo {
            function: get_function_id(db.upcast(), gas_module, "withdraw_gas_all".into(), vec![])
                .lowered(db),
            inputs: vec![builtin_costs_var],
            ..info.clone()
        }),
    };

    Ok(())
}
