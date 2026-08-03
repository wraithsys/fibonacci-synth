//! The Fibonacci algorithm space and its compiler.
//!
//! A patch structure is defined recursively, mirroring `F(n) = F(n-1) + F(n-2)`:
//! `F(1)` and `F(2)` are a single operator, and every `F(n)` for `n >= 3`
//! combines an `F(n-1)` cluster with an `F(n-2)` cluster. At `F(5)` this yields
//! exactly [`NUM_OPS`] = 5 operators and [`NUM_NODES`] = 4 combination nodes.
//!
//! Each node makes one choice ([`Combine`]):
//!
//! - `Series`: the larger sub-cluster `F(n-1)` phase-modulates the *primary
//!   carrier* of the smaller `F(n-2)` (every carrier of the larger cluster
//!   becomes a modulator of that one operator).
//! - `Parallel`: both sub-clusters remain carriers and are summed.
//!
//! The 4 choices form an [`AlgorithmId`] — a 4-bit number, so the full
//! generative space has 16 points. We *ship* a Fibonacci number of them
//! ([`ALGORITHMS`], currently 5): the "unfolding" sequence that flips one node
//! from Series to Parallel per step, taking the instrument from one deep FM
//! tree (1 carrier, modulation depth 4) to five parallel sines (5 carriers,
//! depth 1). Growing the shipped count to 8 or 13 later means curating more
//! IDs from the same space, not redesigning it.
//!
//! [`compile`] flattens an ID into a [`Compiled`] routing: per-operator
//! modulator masks and a deepest-first evaluation order, so the audio loop is
//! a linear pass over 5 operators with no tree-walking.

/// Number of sine operators: `F(5) = 5`.
pub const NUM_OPS: usize = 5;
/// Number of combination nodes in the `F(5)` expansion.
pub const NUM_NODES: usize = 4;

/// How one combination node joins its two sub-clusters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Combine {
    /// The larger sub-cluster `F(n-1)` phase-modulates the smaller `F(n-2)`
    /// cluster's primary carrier. Output comes from the smaller side.
    Series,
    /// Both sub-clusters are carriers; their outputs are summed.
    Parallel,
}

/// A point in the algorithm space: one [`Combine`] choice per node.
///
/// Bit `k` corresponds to node `k` in preorder — `0 = F(5)` (root),
/// `1 = F(4)`, `2 = F(3)` inside `F(4)`, `3 = F(3)` directly under the root.
/// A set bit means [`Combine::Parallel`]; clear means [`Combine::Series`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlgorithmId(pub u8);

impl AlgorithmId {
    /// The [`Combine`] choice at combination node `k` (preorder).
    pub fn node(self, k: usize) -> Combine {
        debug_assert!(k < NUM_NODES);
        if self.0 >> k & 1 == 1 {
            Combine::Parallel
        } else {
            Combine::Series
        }
    }

    /// This ID with node `k` set to `choice`.
    pub fn with_node(self, k: usize, choice: Combine) -> Self {
        debug_assert!(k < NUM_NODES);
        match choice {
            Combine::Parallel => AlgorithmId(self.0 | 1 << k),
            Combine::Series => AlgorithmId(self.0 & !(1 << k)),
        }
    }
}

/// The shipped algorithms — a Fibonacci number of them (5).
///
/// This is the *unfolding sequence*: each step flips exactly one node from
/// Series to Parallel (in order: the root-side F(3), the root, F(4), the
/// deep F(3)). Along it, carrier count runs 1, 2, 3, 4, 5 and maximum
/// modulation depth falls 4, 4, 3, 2, 1.
pub const ALGORITHMS: [AlgorithmId; 5] = [
    AlgorithmId(0b0000), // I:   1 carrier — full FM tree, depth 4
    AlgorithmId(0b1000), // II:  2 carriers — a true 4-op stack + 1 sine
    AlgorithmId(0b1001), // III: 3 carriers — 3-op stack + 2 sines
    AlgorithmId(0b1011), // IV:  4 carriers — 2-op pair + 3 sines
    AlgorithmId(0b1111), // V:   5 carriers — pure additive
];

/// A flattened, allocation-free routing for the audio loop.
#[derive(Clone, Copy, Debug)]
pub struct Compiled {
    /// `modulators[i]` is a bitmask of the operators whose output
    /// phase-modulates operator `i`.
    pub modulators: [u8; NUM_OPS],
    /// Bitmask of the operators whose output reaches the mix bus.
    pub carriers: u8,
    /// Operator indices sorted deepest-first — a valid evaluation order,
    /// since every modulator is strictly deeper than the operator it feeds.
    pub eval_order: [usize; NUM_OPS],
    /// Distance from the mix bus: carriers are depth 1, their modulators 2, …
    pub depth: [u8; NUM_OPS],
    /// The operator carrying self-feedback: the deepest leaf of the tree
    /// (lowest index on ties). With this tree's left-deep operator numbering
    /// that is always operator 0, so feedback has a fixed home in the UI.
    pub feedback_op: usize,
    /// `carriers.count_ones()`, cached for the mix normalization.
    pub carrier_count: u8,
}

/// Intermediate cluster state during tree expansion.
struct Cluster {
    /// Bitmask of this cluster's current carriers.
    carriers: u8,
    /// The operator a Series parent will aim modulation at. For a leaf it is
    /// the operator itself; for a Parallel join it is the larger side's
    /// primary.
    primary: usize,
}

/// Expand the Fibonacci tree for `F(n)`, assigning operator and node indices
/// in preorder and recording Series modulation edges into `modulators`.
fn expand(
    n: u32,
    id: AlgorithmId,
    next_op: &mut usize,
    next_node: &mut usize,
    modulators: &mut [u8; NUM_OPS],
) -> Cluster {
    if n <= 2 {
        let op = *next_op;
        *next_op += 1;
        return Cluster {
            carriers: 1 << op,
            primary: op,
        };
    }
    let node = *next_node;
    *next_node += 1;
    let larger = expand(n - 1, id, next_op, next_node, modulators);
    let smaller = expand(n - 2, id, next_op, next_node, modulators);
    match id.node(node) {
        Combine::Series => {
            modulators[smaller.primary] |= larger.carriers;
            Cluster {
                carriers: smaller.carriers,
                primary: smaller.primary,
            }
        }
        Combine::Parallel => Cluster {
            carriers: larger.carriers | smaller.carriers,
            primary: larger.primary,
        },
    }
}

/// Compile an [`AlgorithmId`] into its flat routing.
///
/// Runs at patch-change time, never on the audio thread (though it happens to
/// be allocation-free anyway).
pub fn compile(id: AlgorithmId) -> Compiled {
    let mut modulators = [0u8; NUM_OPS];
    let mut next_op = 0;
    let mut next_node = 0;
    let root = expand(5, id, &mut next_op, &mut next_node, &mut modulators);
    debug_assert_eq!(next_op, NUM_OPS);
    debug_assert_eq!(next_node, NUM_NODES);

    // Depths, walking modulator edges away from the mix bus. Each operator
    // modulates at most one target (a Series join consumes its carriers), so
    // every operator gets exactly one depth.
    let mut depth = [0u8; NUM_OPS];
    let mut stack = [0usize; NUM_OPS];
    let mut top = 0;
    for op in 0..NUM_OPS {
        if root.carriers >> op & 1 == 1 {
            depth[op] = 1;
            stack[top] = op;
            top += 1;
        }
    }
    while top > 0 {
        top -= 1;
        let target = stack[top];
        for m in 0..NUM_OPS {
            if modulators[target] >> m & 1 == 1 {
                depth[m] = depth[target] + 1;
                stack[top] = m;
                top += 1;
            }
        }
    }

    let mut eval_order = [0usize; NUM_OPS];
    for (slot, op) in eval_order.iter_mut().zip(0..NUM_OPS) {
        *slot = op;
    }
    eval_order.sort_unstable_by_key(|&op| (core::cmp::Reverse(depth[op]), op));

    Compiled {
        modulators,
        carriers: root.carriers,
        eval_order,
        depth,
        feedback_op: eval_order[0],
        carrier_count: root.carriers.count_ones() as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Structural sanity across the *entire* 16-point space, not just the
    /// shipped 5, so future curation can pick any ID safely.
    #[test]
    fn full_space_compiles_to_valid_routings() {
        for raw in 0..16u8 {
            let c = compile(AlgorithmId(raw));

            // Every operator reachable from the mix bus.
            assert!(c.depth.iter().all(|&d| d >= 1), "id {raw:04b}: unreachable op");

            // At least one carrier, and carriers are exactly the depth-1 ops.
            assert!(c.carrier_count >= 1);
            for op in 0..NUM_OPS {
                assert_eq!(c.carriers >> op & 1 == 1, c.depth[op] == 1, "id {raw:04b}");
            }

            // Modulators are evaluated before the operator they feed.
            let mut pos = [0usize; NUM_OPS];
            for (i, &op) in c.eval_order.iter().enumerate() {
                pos[op] = i;
            }
            for target in 0..NUM_OPS {
                for m in 0..NUM_OPS {
                    if c.modulators[target] >> m & 1 == 1 {
                        assert!(pos[m] < pos[target], "id {raw:04b}: bad eval order");
                        assert_eq!(c.depth[m], c.depth[target] + 1, "id {raw:04b}");
                    }
                }
            }

            // Tree property: each operator modulates at most one target.
            for m in 0..NUM_OPS {
                let fan_out = (0..NUM_OPS)
                    .filter(|&t| c.modulators[t] >> m & 1 == 1)
                    .count();
                assert!(fan_out <= 1, "id {raw:04b}: op {m} modulates {fan_out} targets");
            }
        }
    }

    /// The shipped sequence is the documented gradient: one bit flipped per
    /// step, carriers 1..=5, max depth 4,4,3,2,1, feedback always on op 0.
    #[test]
    fn shipped_algorithms_form_the_unfolding_gradient() {
        let mut prev: Option<AlgorithmId> = None;
        for (i, &id) in ALGORITHMS.iter().enumerate() {
            let c = compile(id);
            assert_eq!(c.carrier_count as usize, i + 1, "algorithm {}", i + 1);
            let max_depth = *c.depth.iter().max().unwrap();
            assert_eq!(max_depth, [4, 4, 3, 2, 1][i], "algorithm {}", i + 1);
            assert_eq!(c.feedback_op, 0, "algorithm {}", i + 1);
            if let Some(p) = prev {
                assert_eq!((p.0 ^ id.0).count_ones(), 1, "step {} flips one node", i);
            }
            prev = Some(id);
        }
    }

    #[test]
    fn node_bit_round_trip() {
        let id = AlgorithmId(0b1001);
        assert_eq!(id.node(0), Combine::Parallel);
        assert_eq!(id.node(1), Combine::Series);
        assert_eq!(id.node(3), Combine::Parallel);
        assert_eq!(id.with_node(1, Combine::Parallel).0, 0b1011);
        assert_eq!(id.with_node(0, Combine::Series).0, 0b1000);
    }
}
