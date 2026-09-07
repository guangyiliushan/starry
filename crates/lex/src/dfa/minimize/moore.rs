//! Moore 算法：不动点轮次细化，O(|Σ|·n²)

use std::collections::HashMap;

use super::{initial_blocks, Block};
use crate::dfa::DFA;

pub fn moore(dfa: &DFA) -> DFA {
    let dead_i = dfa.state_count();
    let total = dead_i + 1;
    let atoms = dfa.alphabet_atoms();

    let (mut blocks, mut block_of) = initial_blocks(dfa, dead_i);

    loop {
        // 签名 = (自身块, 各原子目标块)，按首次出现重编号（确定性）
        let mut sig_ids: HashMap<Vec<usize>, usize> = HashMap::new();
        let mut new_blocks: Vec<Block> = Vec::new();
        let mut new_block_of = vec![0usize; total];
        for p in 0..total {
            let mut sig = Vec::with_capacity(atoms.len() + 1);
            sig.push(block_of[p]);
            for atom in &atoms {
                sig.push(block_of[super::total_target(dfa, dead_i, p, atom.0)]);
            }
            let b = *sig_ids.entry(sig).or_insert_with(|| {
                new_blocks.push(Block {
                    color: if p == dead_i { None } else { dfa.token_kind(p) },
                    members: Vec::new(),
                });
                new_blocks.len() - 1
            });
            new_block_of[p] = b;
            new_blocks[b].members.push(p);
        }

        let converged = new_blocks.len() == blocks.len();
        blocks = new_blocks;
        block_of = new_block_of;
        if converged {
            break;
        }
    }

    let sink = block_of[dead_i];
    super::build(dfa, &blocks, &block_of, sink)
}
