// TODO:
// 1. Other gas tokens
// 2. AP
// 3. Withdraw gas builtins.
// 4. refund gas

use cairo_lang_sierra::extensions::core::{CoreLibfunc, CoreType};
use cairo_lang_sierra::extensions::gas::CostTokenType;
use cairo_lang_sierra::extensions::ConcreteType;
use cairo_lang_sierra::ids::{ConcreteLibfuncId, ConcreteTypeId};
use cairo_lang_sierra::program::{Invocation, Program, Statement, StatementIdx};
use cairo_lang_sierra::program_registry::ProgramRegistry;
use cairo_lang_utils::iterators::zip_eq3;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use itertools::zip_eq;

use crate::core_libfunc_cost_base::core_libfunc_cost;
use crate::gas_info::GasInfo;
use crate::objects::{BranchCost, ConstCost, CostInfoProvider, PreCost};
use crate::CostError;

type VariableValues = OrderedHashMap<(StatementIdx, CostTokenType), i64>;

/// Implementation of [CostInfoProvider] given a [program registry](ProgramRegistry).
pub struct ComputeCostInfoProviderImpl<'a> {
    registry: &'a ProgramRegistry<CoreType, CoreLibfunc>,
}
impl<'a> ComputeCostInfoProviderImpl<'a> {
    pub fn new(registry: &'a ProgramRegistry<CoreType, CoreLibfunc>) -> Self {
        Self { registry }
    }
}
impl<'a> CostInfoProvider for ComputeCostInfoProviderImpl<'a> {
    fn type_size(&self, ty: &ConcreteTypeId) -> usize {
        // TODO: fix `as usize`.
        self.registry.get_type(ty).unwrap().info().size as usize
    }
}

/// A trait for the cost type (either [PreCost] for pre-cost computation, or `i32` for the post-cost
/// computation).
pub trait CostTypeTrait: std::fmt::Debug + Default + Clone + Eq {
    fn max(values: impl Iterator<Item = Self>) -> Self;
}

impl CostTypeTrait for i32 {
    fn max(values: impl Iterator<Item = Self>) -> Self {
        values.max().unwrap_or_default()
    }
}

impl CostTypeTrait for PreCost {
    fn max(values: impl Iterator<Item = Self>) -> Self {
        let mut res = Self::default();
        for value in values {
            for (token_type, val) in value.0 {
                res.0.insert(token_type, std::cmp::max(*res.0.get(&token_type).unwrap_or(&0), val));
            }
        }
        res
    }
}

pub fn compute_postcost_info(
    program: &Program,
    get_ap_change_fn: &dyn Fn(&StatementIdx) -> usize,
) -> Result<GasInfo, CostError> {
    let registry = ProgramRegistry::<CoreType, CoreLibfunc>::new(program)?;
    let info_provider = ComputeCostInfoProviderImpl { registry: &registry };
    let specific_cost_context = PostcostContext { get_ap_change_fn };
    Ok(compute_costs(
        program,
        &(|libfunc_id| {
            let core_libfunc = registry
                .get_libfunc(libfunc_id)
                .expect("Program registry creation would have already failed.");
            core_libfunc_cost(core_libfunc, &info_provider)
        }),
        &specific_cost_context,
    ))
}

/// Computes the [GasInfo] for a given program.
///
/// The `specific_cost_context` argument controls whether the computation is pre-cost or post-cost.
pub fn compute_costs<
    CostType: CostTypeTrait,
    SpecificCostContext: SpecificCostContextTrait<CostType>,
>(
    program: &Program,
    get_cost_fn: &dyn Fn(&ConcreteLibfuncId) -> Vec<BranchCost>,
    specific_cost_context: &SpecificCostContext,
) -> GasInfo {
    let mut context = CostContext { program, costs: UnorderedHashMap::default(), get_cost_fn };

    for i in 0..program.statements.len() {
        context.compute_wallet_at(&StatementIdx(i), specific_cost_context);
    }

    let mut variable_values = VariableValues::default();
    for i in 0..program.statements.len() {
        analyze_gas_statements(
            &context,
            specific_cost_context,
            &StatementIdx(i),
            &mut variable_values,
        );
    }

    let function_costs = program
        .funcs
        .iter()
        .map(|func| {
            let res = SpecificCostContext::to_cost_map(context.wallet_at(&func.entry_point));
            (func.id.clone(), res)
        })
        .collect();

    GasInfo { variable_values, function_costs }
}

/// For every `branch_align` and `withdraw_gas` statements, computes the required cost variables.
///
/// * For `branch_align` this is the amount of cost *reduced* from the wallet.
/// * For `withdraw_gas` this is the amount that should be withdrawn and added to the wallet.
fn analyze_gas_statements<
    CostType: CostTypeTrait,
    SpecificCostContext: SpecificCostContextTrait<CostType>,
>(
    context: &CostContext<'_, CostType>,
    specific_context: &SpecificCostContext,
    idx: &StatementIdx,
    variable_values: &mut VariableValues,
) {
    let Statement::Invocation(invocation) = &context.program.get_statement(idx).unwrap() else {
            return;
        };
    let libfunc_cost: Vec<BranchCost> = context.get_cost(&invocation.libfunc_id);
    let branch_requirements: Vec<CostType> = specific_context.get_branch_requirements(
        &mut |statement_idx| context.wallet_at(statement_idx),
        idx,
        invocation,
        &libfunc_cost,
    );

    let wallet_value = context.wallet_at(idx);

    if invocation.branches.len() > 1 {
        for (branch_info, branch_cost, branch_requirement) in
            zip_eq3(&invocation.branches, &libfunc_cost, &branch_requirements)
        {
            let future_wallet_value = context.wallet_at(&idx.next(&branch_info.target));
            if let BranchCost::WithdrawGas { success: true, .. } = branch_cost {
                for (token_type, amount) in specific_context.get_withdraw_gas_values(
                    branch_cost,
                    &wallet_value,
                    future_wallet_value,
                ) {
                    let insert_res =
                        variable_values.insert((*idx, token_type), std::cmp::max(amount, 0));
                    assert!(insert_res.is_none());

                    let insert_res = variable_values.insert(
                        (idx.next(&branch_info.target), token_type),
                        std::cmp::max(-amount, 0),
                    );
                    assert!(insert_res.is_none());
                }
            } else {
                // TODO: Consider checking this is indeed branch align.
                for (token_type, amount) in
                    specific_context.get_branch_align_values(&wallet_value, branch_requirement)
                {
                    let res =
                        variable_values.insert((idx.next(&branch_info.target), token_type), amount);
                    assert!(res.is_none());
                }
            }
        }
    }
}

pub trait SpecificCostContextTrait<CostType: CostTypeTrait> {
    /// Converts a `CostType` to a [OrderedHashMap] from [CostTokenType] to i64.
    fn to_cost_map(cost: CostType) -> OrderedHashMap<CostTokenType, i64>;

    /// Computes the value that should be withdrawn and added to the wallet for each token type.
    fn get_withdraw_gas_values(
        &self,
        branch_cost: &BranchCost,
        wallet_value: &CostType,
        future_wallet_value: CostType,
    ) -> OrderedHashMap<CostTokenType, i64>;

    /// Computes the value that should be reduced from the wallet for each token type.
    fn get_branch_align_values(
        &self,
        wallet_value: &CostType,
        branch_requirement: &CostType,
    ) -> OrderedHashMap<CostTokenType, i64>;

    /// Returns the required value for the wallet for each branch.
    fn get_branch_requirements(
        &self,
        wallet_at_fn: &mut dyn FnMut(&StatementIdx) -> CostType,
        idx: &StatementIdx,
        invocation: &Invocation,
        libfunc_cost: &[BranchCost],
    ) -> Vec<CostType>;
}

/// Represents the status of the computation of the wallet at a given statement.
#[derive(Eq, PartialEq)]
enum CostComputationStatus<CostType> {
    /// The computation is in progress.
    InProgress,
    /// The computation was completed.
    Done(CostType),
}

/// Helper struct for computing the wallet value at each statement.
struct CostContext<'a, CostType> {
    /// The Sierra program.
    program: &'a Program,
    /// A callback function returning the cost of a libfunc for every output branch.
    get_cost_fn: &'a dyn Fn(&ConcreteLibfuncId) -> Vec<BranchCost>,
    /// The cost before executing a Sierra statement.
    costs: UnorderedHashMap<StatementIdx, CostComputationStatus<CostType>>,
}
impl<'a, CostType: CostTypeTrait> CostContext<'a, CostType> {
    /// Returns the cost of a libfunc for every output branch.
    fn get_cost(&self, libfunc_id: &ConcreteLibfuncId) -> Vec<BranchCost> {
        (self.get_cost_fn)(libfunc_id)
    }

    /// Returns the required value in the wallet before executing statement `idx`.
    ///
    /// Assumes that [Self::compute_wallet_at] was called before.
    ///
    /// For `branch_align` the function returns the result as if the alignment is zero (since the
    /// alignment is not know at this point).
    fn wallet_at(&self, idx: &StatementIdx) -> CostType {
        match self.costs.get(idx) {
            Some(CostComputationStatus::Done(res)) => res.clone(),
            _ => {
                panic!("Wallet value for statement {idx} was not yet computed.")
            }
        }
    }

    /// Same as [Self::wallet_at], but computes the value if it was not yet computed.
    fn compute_wallet_at<SpecificCostContext: SpecificCostContextTrait<CostType>>(
        &mut self,
        idx: &StatementIdx,
        specific_cost_context: &SpecificCostContext,
    ) -> CostType {
        match self.costs.get_mut(idx) {
            Some(CostComputationStatus::InProgress) => {
                panic!("Found an unexpected cycle during cost computation.")
            }
            Some(CostComputationStatus::Done(res)) => {
                return res.clone();
            }
            None => {}
        }

        // Mark the statement's computation as in-progress.
        self.costs.insert(*idx, CostComputationStatus::InProgress);

        // Compute the value.
        let res = self.no_cache_compute_wallet_at(idx, specific_cost_context);

        // Update the cache with the result.
        assert!(
            self.costs.insert(*idx, CostComputationStatus::Done(res.clone()))
                == Some(CostComputationStatus::InProgress),
            "Unexpected cost computation status."
        );
        println!("Cost of {idx} is {res:?}.");
        res
    }

    /// Same as [Self::compute_wallet_at], except that the cache is not used.
    ///
    /// Calls [Self::compute_wallet_at] to get the wallet value of the following instructions.
    fn no_cache_compute_wallet_at<SpecificCostContext: SpecificCostContextTrait<CostType>>(
        &mut self,
        idx: &StatementIdx,
        specific_cost_context: &SpecificCostContext,
    ) -> CostType {
        match &self.program.get_statement(idx).unwrap() {
            Statement::Return(_) => Default::default(),
            Statement::Invocation(invocation) => {
                let libfunc_cost: Vec<BranchCost> = self.get_cost(&invocation.libfunc_id);

                // For each branch, compute the required value for the wallet.
                let branch_requirements: Vec<CostType> = specific_cost_context
                    .get_branch_requirements(
                        &mut |statement_idx| {
                            self.compute_wallet_at(statement_idx, specific_cost_context)
                        },
                        idx,
                        invocation,
                        &libfunc_cost,
                    );

                // The wallet value at the beginning of the statement is the maximal value
                // required by all the branches.
                CostType::max(branch_requirements.into_iter())
            }
        }
    }
}

pub struct PreCostContext {}

impl SpecificCostContextTrait<PreCost> for PreCostContext {
    fn to_cost_map(cost: PreCost) -> OrderedHashMap<CostTokenType, i64> {
        let res = cost.0;
        res.into_iter().map(|(token_type, val)| (token_type, val as i64)).collect()
    }

    fn get_withdraw_gas_values(
        &self,
        _branch_cost: &BranchCost,
        wallet_value: &PreCost,
        future_wallet_value: PreCost,
    ) -> OrderedHashMap<CostTokenType, i64> {
        let res = (future_wallet_value - wallet_value.clone()).0;
        CostTokenType::iter_precost()
            .map(|token_type| (*token_type, *res.get(token_type).unwrap_or(&0) as i64))
            .collect()
    }

    fn get_branch_align_values(
        &self,
        wallet_value: &PreCost,
        branch_requirement: &PreCost,
    ) -> OrderedHashMap<CostTokenType, i64> {
        let res = (wallet_value.clone() - branch_requirement.clone()).0;
        CostTokenType::iter_precost()
            .map(|token_type| (*token_type, *res.get(token_type).unwrap_or(&0) as i64))
            .collect()
    }

    fn get_branch_requirements(
        &self,
        wallet_at_fn: &mut dyn FnMut(&StatementIdx) -> PreCost,
        idx: &StatementIdx,
        invocation: &Invocation,
        libfunc_cost: &[BranchCost],
    ) -> Vec<PreCost> {
        zip_eq(&invocation.branches, libfunc_cost)
            .map(|(branch_info, branch_cost)| {
                let branch_cost = match &*branch_cost {
                    BranchCost::Regular { const_cost: _, pre_cost } => pre_cost.clone(),
                    BranchCost::BranchAlign => Default::default(),
                    BranchCost::FunctionCall { const_cost: _, function } => {
                        wallet_at_fn(&function.entry_point)
                    }
                    BranchCost::WithdrawGas { const_cost: _, success, with_builtin_costs: _ } => {
                        if *success {
                            // If withdraw_gas succeeds, we don't need to take
                            // future_wallet_value into account, so we simply return.
                            return Default::default();
                        } else {
                            Default::default()
                        }
                    }
                    BranchCost::RedepositGas => todo!(),
                };
                let future_wallet_value = wallet_at_fn(&idx.next(&branch_info.target));
                branch_cost + future_wallet_value
            })
            .collect()
    }
}

struct PostcostContext<'a> {
    get_ap_change_fn: &'a dyn Fn(&StatementIdx) -> usize,
}

impl<'a> SpecificCostContextTrait<i32> for PostcostContext<'a> {
    fn to_cost_map(cost: i32) -> OrderedHashMap<CostTokenType, i64> {
        if cost == 0 {
            Default::default()
        } else {
            [(CostTokenType::Const, cost as i64)].into_iter().collect()
        }
    }

    fn get_withdraw_gas_values(
        &self,
        branch_cost: &BranchCost,
        wallet_value: &i32,
        future_wallet_value: i32,
    ) -> OrderedHashMap<CostTokenType, i64> {
        // TODO: with_builtin_costs.

        let BranchCost::WithdrawGas { const_cost, success: true, with_builtin_costs: _ } = branch_cost else {
            panic!("Unexpected BranchCost: {:?}.", branch_cost);
        };

        let amount = ((const_cost.cost() + future_wallet_value) as i64) - (*wallet_value as i64);
        [(CostTokenType::Const, amount as i64)].into_iter().collect()
    }

    fn get_branch_align_values(
        &self,
        wallet_value: &i32,
        branch_requirement: &i32,
    ) -> OrderedHashMap<CostTokenType, i64> {
        let amount = (wallet_value - branch_requirement) as i64;
        [(CostTokenType::Const, amount as i64)].into_iter().collect()
    }

    fn get_branch_requirements(
        &self,
        wallet_at_fn: &mut dyn FnMut(&StatementIdx) -> i32,
        idx: &StatementIdx,
        invocation: &Invocation,
        libfunc_cost: &[BranchCost],
    ) -> Vec<i32> {
        zip_eq(&invocation.branches, libfunc_cost)
            .map(|(branch_info, branch_cost)| {
                let branch_cost = match &*branch_cost {
                    BranchCost::Regular { const_cost, pre_cost: _ } => const_cost.cost(),
                    BranchCost::BranchAlign => {
                        let ap_change = (self.get_ap_change_fn)(idx);
                        if ap_change == 0 {
                            0
                        } else {
                            ConstCost { steps: 1, holes: ap_change as i32, range_checks: 0 }.cost()
                        }
                    }
                    BranchCost::FunctionCall { const_cost, function } => {
                        wallet_at_fn(&function.entry_point) + const_cost.cost()
                    }
                    BranchCost::WithdrawGas { const_cost, success, with_builtin_costs: _ } => {
                        let cost = const_cost.cost();
                        // TODO: with_builtins.
                        // If withdraw_gas succeeds, we don't need to take
                        // future_wallet_value into account, so we simply return.
                        if *success {
                            return cost;
                        }
                        cost
                    }
                    BranchCost::RedepositGas => todo!(),
                };
                let future_wallet_value = wallet_at_fn(&idx.next(&branch_info.target));
                branch_cost + future_wallet_value
            })
            .collect()
    }
}
