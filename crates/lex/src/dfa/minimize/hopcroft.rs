//! Hopcroft 算法：分裂细化，O(|Σ|·n·log n)
//!
//! 经典机制：state→block 成员索引、按 splitter 计数分裂、smaller-half
//! 入队（等长 tie-break 取 min block id）。worklist 为 `VecDeque` FIFO；
//! splitter 自分裂时两部分都回队。可复现性不依赖弹出顺序
//! （粗稳定划分唯一）。

use std::collections::{HashMap, VecDeque};

use super::{initial_blocks, Block};
use crate::dfa::DFA;

pub fn hopcroft(dfa: &DFA) -> DFA {
    let dead_i = dfa.state_count();
    let total = dead_i + 1;
    let atoms = dfa.alphabet_atoms();

    // 反向邻接：每原子 (target, pred) 按目标排序，等值区间二分取段
    let mut rev: Vec<Vec<(usize, usize)>> = vec![Vec::new(); atoms.len()];
    for p in 0..total {
        for (a, atom) in atoms.iter().enumerate() {
            let t = super::total_target(dfa, dead_i, p, atom.0);
            rev[a].push((t, p));
        }
    }
    for list in &mut rev {
        list.sort_unstable();
    }

    let (mut blocks, mut block_of) = initial_blocks(dfa, dead_i);
    let mut worklist: VecDeque<usize> = (0..blocks.len()).collect();
    let mut in_worklist = vec![true; blocks.len()];

    while let Some(s) = worklist.pop_front() {
        if blocks[s].members.is_empty() {
            in_worklist[s] = false;
            continue;
        }
        in_worklist[s] = false;
        let s_members = blocks[s].members.clone();

        for (a, _atom) in atoms.iter().enumerate() {
            // X = {p : target(p, rep) ∈ block s}，按块计数
            let mut touched: Vec<usize> = Vec::new();
            let mut counts: HashMap<usize, usize> = HashMap::new();
            let mut in_x: HashMap<usize, Vec<usize>> = HashMap::new();
            for &q in &s_members {
                let lo = rev[a].partition_point(|&(t, _)| t < q);
                let hi = rev[a].partition_point(|&(t, _)| t <= q);
                for &(_, p) in &rev[a][lo..hi] {
                    let b = block_of[p];
                    if counts.get(&b).copied().unwrap_or(0) == 0 {
                        touched.push(b);
                    }
                    *counts.entry(b).or_insert(0) += 1;
                    in_x.entry(b).or_default().push(p);
                }
            }

            for b in touched {
                let cnt = counts[&b];
                let len = blocks[b].members.len();
                if cnt == len {
                    continue; // 整块命中，不分裂
                }
                // 分裂：b 保留 X 外部分，新块 = X 内部分
                let mut b1 = in_x.remove(&b).unwrap();
                b1.sort_unstable();
                let in_set: std::collections::BTreeSet<usize> = b1.iter().copied().collect();
                let b2: Vec<usize> = blocks[b]
                    .members
                    .iter()
                    .copied()
                    .filter(|p| !in_set.contains(p))
                    .collect();
                let (b1_len, b2_len) = (b1.len(), b2.len());
                let color = blocks[b].color;
                blocks[b] = Block {
                    color,
                    members: b2,
                };
                for &p in &blocks[b].members {
                    block_of[p] = b;
                }
                let new_id = blocks.len();
                blocks.push(Block {
                    color,
                    members: b1,
                });
                for &p in &blocks[new_id].members {
                    block_of[p] = new_id;
                }

                // 入队规则：b 原在队 ⇒ 两部分都入队（replace 规则）；
                // 否则入队较小半（等长 tie-break 取 min id = b）
                if b == s {
                    // splitter 自分裂：两部分都必须回 worklist
                    in_worklist[b] = true;
                    worklist.push_back(b);
                    in_worklist.push(true);
                    worklist.push_back(new_id);
                } else if in_worklist[b] {
                    in_worklist.push(true);
                    worklist.push_back(new_id);
                } else if b1_len < b2_len {
                    in_worklist.push(true);
                    worklist.push_back(new_id);
                } else {
                    in_worklist[b] = true;
                    worklist.push_back(b);
                }
            }
        }
    }

    let sink = block_of[dead_i];
    super::build(dfa, &blocks, &block_of, sink)
}
