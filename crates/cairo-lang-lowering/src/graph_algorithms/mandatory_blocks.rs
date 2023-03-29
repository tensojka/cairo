use itertools::Itertools;

use crate::blocks::FlatBlocks;
use crate::borrow_check::analysis::{Analyzer, BackAnalysis, StatementLocation};
use crate::{BlockId, FlatBlock, FlatBlockEnd, FlatLowered, MatchInfo, VarRemapping, VariableId};

// TODO(yg): make this a query?
pub fn find_mandatory_blocks(lowered: &mut FlatLowered) -> Vec<BlockId> {
    if lowered.blocks.is_empty() {
        return Vec::new();
    }

    let ctx = MandatoryBlocksAnalyzer { incoming_gotos: vec![0; lowered.blocks.len()] };
    let mut analysis =
        BackAnalysis { lowered: &*lowered, cache: Default::default(), analyzer: ctx };
    analysis.get_root_info();
    let ctx = analysis.analyzer;

    let mandatory_blocks_algo_blocks = ctx
        .incoming_gotos
        .iter()
        .map(|num_parents| MandatoryBlocksAlgoBlock {
            current_weight: 0.0,
            parents_left: *num_parents,
        })
        .collect_vec();
    let mut mandatory_blocks_algo_ctx = MandatoryBlocksAlgoContext {
        mandatory_blocks_algo_blocks,
        lowered_blocks: &lowered.blocks,
    };
    update_block_weight(&mut mandatory_blocks_algo_ctx, BlockId::root(), 1.0);

    let mandatory_blocks = mandatory_blocks_algo_ctx
        .mandatory_blocks_algo_blocks
        .into_iter()
        .enumerate()
        .filter_map(
            |(idx, block)| {
                if block.current_weight == 1.0 { Some(BlockId(idx)) } else { None }
            },
        )
        .collect_vec();
    mandatory_blocks
}

/// The current info about a block during the mandatory blocks algorithm.
#[derive(Clone)]
struct MandatoryBlocksAlgoBlock {
    /// The current accumulated weight of this block.
    current_weight: f32,
    /// Number of parents left to visit this block through.
    parents_left: usize,
}

/// The context of the mandatory blocks algorithm.
struct MandatoryBlocksAlgoContext<'a> {
    mandatory_blocks_algo_blocks: Vec<MandatoryBlocksAlgoBlock>,
    lowered_blocks: &'a FlatBlocks,
}

// TODO(yg): f## is not accurate enough to get exactly to 1... e.g. 1/3+1/3+1/3 != 1. collect parts.
// let mul = parts.mul() and check that parts.map(|part| mul/part).sum() == mul.
fn update_block_weight(
    ctx: &mut MandatoryBlocksAlgoContext,
    block_id: BlockId,
    weight_from_parent: f32,
) {
    let mandatory_blocks_algo_block = &mut ctx.mandatory_blocks_algo_blocks[block_id.0];
    mandatory_blocks_algo_block.current_weight += weight_from_parent;
    mandatory_blocks_algo_block.parents_left -= 1;
    if mandatory_blocks_algo_block.parents_left > 0
        || mandatory_blocks_algo_block.current_weight < 1.0
    {
        return;
    }

    let lowered_block = ctx.lowered_blocks[block_id].clone();
    // Recursive call for all children.
    match lowered_block.end {
        FlatBlockEnd::NotSet => unreachable!(),
        FlatBlockEnd::Return(_) | FlatBlockEnd::Panic(_) => {}
        FlatBlockEnd::Goto(child_id, _) => update_block_weight(ctx, child_id, 1.0),
        FlatBlockEnd::Match { info } => {
            let arms = info.arms();
            let child_weight = 1.0 / arms.len() as f32;
            for arm in arms {
                update_block_weight(ctx, arm.block_id, child_weight);
            }
        }
    }
}

struct MandatoryBlocksAnalyzer {
    /// The number of incoming gotos, indexed by block_id.
    incoming_gotos: Vec<usize>,
}

impl Analyzer<'_> for MandatoryBlocksAnalyzer {
    type Info = ();

    fn visit_block_start(
        &mut self,
        _info: &mut Self::Info,
        _block_id: BlockId,
        _block: &FlatBlock,
    ) {
    }

    fn visit_remapping(
        &mut self,
        _info: &mut Self::Info,
        _statement_location: StatementLocation,
        target_block_id: BlockId,
        _remapping: &VarRemapping,
    ) {
        self.incoming_gotos[target_block_id.0] += 1;
    }

    fn merge_match(
        &mut self,
        _statement_location: StatementLocation,
        _match_info: &MatchInfo,
        _infos: &[Self::Info],
    ) -> Self::Info {
    }

    fn info_from_return(
        &mut self,
        _statement_location: StatementLocation,
        _vars: &[VariableId],
    ) -> Self::Info {
    }

    fn info_from_panic(
        &mut self,
        _statement_location: StatementLocation,
        _data: &VariableId,
    ) -> Self::Info {
    }
}
