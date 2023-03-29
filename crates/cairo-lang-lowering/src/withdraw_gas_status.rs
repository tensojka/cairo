use cairo_lang_semantic::corelib::{core_submodule, get_function_id};

use crate::db::LoweringGroup;
use crate::graph_algorithms::mandatory_blocks::find_mandatory_blocks;
use crate::{FlatLowered, Statement};

/// The status of the function with regards to `withdraw_gas`/`withdraw_gas_all` mandatory calls. A
/// mandatory statement is a statement that must happen if the function was called (for example, the
/// first statement in a function is a mandatory statement).
pub enum WithdrawGasStatus {
    /// The function has a mandatory call to `withdraw_gas_all`.
    WithdrawGasAll,
    /// The function has a mandatory call to `withdraw_gas`, but has no mandatory call to
    /// `withdraw_gas_all`.
    WithdrawGas,
    /// The function doesn't have a mandatory call to either `withdraw_gas_all`/`withdraw_gas`.
    None,
}

// TODO(yg): make this a query?
pub fn find_withdraw_status(
    db: &dyn LoweringGroup,
    lowered: &mut FlatLowered,
) -> WithdrawGasStatus {
    let semantic_db = db.upcast();
    let mandatory_blocks = find_mandatory_blocks(lowered);
    let gas_module = core_submodule(semantic_db, "gas");
    let withdraw_gas_all_id =
        get_function_id(semantic_db, gas_module, "withdraw_gas_all".into(), vec![]);
    let withdraw_gas_id = get_function_id(semantic_db, gas_module, "withdraw_gas".into(), vec![]);

    let mut has_withdraw_gas = false;
    for block_id in mandatory_blocks {
        let block = &lowered.blocks[block_id];
        for stmnt in &block.statements {
            if let Statement::Call(call) = stmnt {
                if call.function.is_semantic_and_eq(db, withdraw_gas_all_id) {
                    return WithdrawGasStatus::WithdrawGasAll;
                } else if call.function.is_semantic_and_eq(db, withdraw_gas_id) {
                    has_withdraw_gas = true;
                }
            }
        }
    }

    if has_withdraw_gas { WithdrawGasStatus::WithdrawGas } else { WithdrawGasStatus::None }
}
