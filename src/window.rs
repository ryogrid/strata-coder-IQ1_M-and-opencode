// Windows: a binary-split tree (design/architecture.md). Each leaf window
// holds the buffer it displays, its own point/mark, and a scroll offset.
// Splitting a leaf replaces it with an internal node whose children are
// the original leaf and a new sibling. Slot indices stay stable.
pub struct Leaf {
    pub buffer: usize,
    pub point: usize,
    pub mark: usize,
    pub scroll: usize,
}

pub struct Node {
    pub vertical: bool,
    pub a: usize,
    pub b: usize,
}

pub struct Tree {
    pub kind: Vec<usize>, // slot tag: 0 = leaf, 1 = node
    pub which: Vec<usize>, // leaf index (kind 0) or node index (kind 1)
    pub leaves: Vec<Leaf>,
    pub nodes: Vec<Node>,
    pub root: usize,
}

pub struct Frame {
    pub leaf: usize,
    // 0 = window, 1 = horizontal boundary row, 2 = vertical boundary column.
    // Windows are separated on screen by a boundary line so split-window +
    // multi-buffer states are visually unambiguous.
    pub kind: usize,
    pub top: usize,
    pub height: usize,
    pub col: usize,
    pub width: usize,
}

pub fn new_tree(buffer: usize) -> Tree {
    Tree {
        kind: vec![0],
        which: vec![0],
        leaves: vec![Leaf { buffer, point: 0, mark: 0, scroll: 0 }],
        nodes: vec![],
        root: 0,
    }
}

impl Tree {
    pub fn collect(self: &Tree, slot: usize) -> Vec<usize> {
        if self.kind[slot] == 0 {
            vec![self.which[slot]]
        } else {
            let k = self.which[slot];
            let mut acc = self.collect(self.nodes[k].a);
            acc.extend(self.collect(self.nodes[k].b).iter());
            acc
        }
    }

    pub fn flatten(self: &Tree) -> Vec<usize> {
        self.collect(self.root)
    }

    pub fn layout(self: &Tree, slot: usize, top: usize, height: usize, col: usize, width: usize) -> Vec<Frame> {
        if self.kind[slot] == 0 {
            vec![Frame { leaf: self.which[slot], kind: 0, top, height, col, width }]
        } else {
            let k = self.which[slot];
            let vertical = self.nodes[k].vertical;
            if vertical && height >= 3 {
                // Stacked windows share one boundary row (drawn as '-').
                let h1 = height / 2;
                let mut a = self.layout(self.nodes[k].a, top, h1, col, width);
                a.push(Frame { leaf: 0, kind: 1, top: top + h1, height: 1, col, width });
                a.extend(self.layout(self.nodes[k].b, top + h1 + 1, height - h1 - 1, col, width));
                a
            } else if vertical && height >= 2 {
                // Too cramped for a boundary row: fall back to plain split.
                let h1 = height.div_ceil(2);
                let mut a = self.layout(self.nodes[k].a, top, h1, col, width);
                a.extend(self.layout(self.nodes[k].b, top + h1, height - h1, col, width));
                a
            } else if width >= 3 {
                // Side-by-side windows share one boundary column (drawn as '|').
                let w1 = width / 2;
                let mut a = self.layout(self.nodes[k].a, top, height, col, w1);
                a.push(Frame { leaf: 0, kind: 2, top, height, col: col + w1, width: 1 });
                a.extend(self.layout(self.nodes[k].b, top, height, col + w1 + 1, width - w1 - 1));
                a
            } else if width >= 2 {
                let w1 = width.div_ceil(2);
                let mut a = self.layout(self.nodes[k].a, top, height, col, w1);
                a.extend(self.layout(self.nodes[k].b, top, height, col + w1, width - w1));
                a
            } else {
                let mut a = self.layout(self.nodes[k].a, top, height, col, width);
                a.extend(self.layout(self.nodes[k].b, top, height, col, width));
                a
            }
        }
    }

    pub fn find_slot(self: &Tree, slot: usize, leaf: usize) -> usize {
        if self.kind[slot] == 0 {
            if self.which[slot] == leaf {
                slot
            } else {
                self.kind.len()
            }
        } else {
            let k = self.which[slot];
            let mut s = self.find_slot(self.nodes[k].a, leaf);
            if s == self.kind.len() {
                s = self.find_slot(self.nodes[k].b, leaf);
            }
            s
        }
    }

    pub fn split(self: &mut Tree, leaf: usize, vertical: bool) -> bool {
        let s = self.find_slot(self.root, leaf);
        if s >= self.kind.len() {
            return false;
        }
        if self.kind[s] == 0 {
            let copy = Leaf {
                buffer: self.leaves[leaf].buffer,
                point: self.leaves[leaf].point,
                mark: self.leaves[leaf].mark,
                scroll: self.leaves[leaf].scroll,
            };
            let copy_idx = self.leaves.len();
            self.leaves.push(copy);
            let sa = self.kind.len();
            self.kind.push(0);
            self.which.push(leaf);
            let sb = self.kind.len();
            self.kind.push(0);
            self.which.push(copy_idx);
            let k = self.nodes.len();
            self.nodes.push(Node { vertical, a: sa, b: sb });
            self.kind[s] = 1;
            self.which[s] = k;
            return true;
        }
        false
    }

    // Collapse the tree to the single window showing `leaf` (C-x 1).
    // S-3: REBUILDS the arrays around the surviving leaf, freeing every
    // other leaf/slot/node (the old push-only growth made repeated
    // split + C-x 1 cycles grow kind/which/nodes/leaves forever).
    // Returns the surviving leaf's NEW index (callers remap `active`).
    pub fn collapse(self: &mut Tree, leaf: usize) -> usize {
        let s = self.find_slot(self.root, leaf);
        if s < self.kind.len() {
            let keep = Leaf {
                buffer: self.leaves[leaf].buffer,
                point: self.leaves[leaf].point,
                mark: self.leaves[leaf].mark,
                scroll: self.leaves[leaf].scroll,
            };
            self.leaves = vec![keep];
            self.kind = vec![0]; // single leaf slot
            self.which = vec![0];
            self.nodes = vec![];
            self.root = 0;
            return 0;
        }
        leaf // leaf not in this tree: untouched (callers pass the active leaf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout_geometry_splits() {
        // side by side (C-x 3): 80 cols -> left 40, boundary col 40, right 39
        let mut t = new_tree(0);
        assert!(t.split(0, false));
        let f = t.layout(t.root, 0, 10, 0, 80);
        assert!(f.len() == 3);
        assert!(f[0].kind == 0 && f[0].col == 0 && f[0].width == 40);
        assert!(f[1].kind == 2 && f[1].col == 40 && f[1].width == 1);
        assert!(f[2].kind == 0 && f[2].col == 41 && f[2].width == 39);
        // stacked (C-x 2): 24 rows -> 12 + boundary row 12 + 11
        let mut t = new_tree(0);
        assert!(t.split(0, true));
        let f = t.layout(t.root, 0, 24, 0, 80);
        assert!(f.len() == 3);
        assert!(f[0].kind == 0 && f[0].top == 0 && f[0].height == 12);
        assert!(f[1].kind == 1 && f[1].top == 12 && f[1].height == 1);
        assert!(f[2].kind == 0 && f[2].top == 13 && f[2].height == 11);
        // smallest geometries that still keep a boundary: 1 + row/col + 1
        let f = t.layout(t.root, 5, 3, 0, 80);
        assert!(f.len() == 3 && f[0].top == 5 && f[0].height == 1
            && f[1].kind == 1 && f[1].top == 6
            && f[2].top == 7 && f[2].height == 1);
        let mut t = new_tree(0);
        assert!(t.split(0, false));
        let f = t.layout(t.root, 0, 5, 9, 3);
        assert!(f.len() == 3 && f[0].col == 9 && f[0].width == 1
            && f[1].kind == 2 && f[1].col == 10
            && f[2].col == 11 && f[2].width == 1);
    }
    #[test]
    fn layout_cramped_falls_back_without_boundary() {
        // 2 rows: a boundary row would starve one window -> plain split
        let mut t = new_tree(0);
        assert!(t.split(0, true));
        let f = t.layout(t.root, 0, 2, 0, 80);
        assert!(f.len() == 2 && f.iter().all(|x| x.kind == 0));
        assert!(f[0].top == 0 && f[0].height == 1 && f[1].top == 1 && f[1].height == 1);
        // 2 cols: no boundary column
        let mut t = new_tree(0);
        assert!(t.split(0, false));
        let f = t.layout(t.root, 0, 5, 0, 2);
        assert!(f.len() == 2 && f[0].width == 1 && f[1].col == 1 && f[1].width == 1);
        // 1 col: unsplittable, both children take the full width
        let f = t.layout(t.root, 0, 5, 0, 1);
        assert!(f.len() == 2 && f[0].width == 1 && f[1].col == 0 && f[1].width == 1);
    }
    #[test]
    fn flatten_order_and_find_slot() {
        // nested splits keep tree order: leaf 0, then the stacked copy split right
        let mut t = new_tree(0);
        assert!(t.split(0, true)); // leaves 0,1 stacked
        assert!(t.split(1, false)); // leaf 1 splits right -> leaf 2
        assert!(t.flatten() == vec![0, 1, 2]);
        assert!(t.find_slot(t.root, 2) < t.kind.len());
        assert!(t.find_slot(t.root, 3) == t.kind.len()); // unknown leaf: not found
        assert!(!t.split(3, true)); // split on an unknown leaf fails
        t.collapse(1); // survivor (leaf 1's copy) moves to index 0
        assert!(t.flatten() == vec![0]);
        // unknown leaf: tree untouched, the index is echoed back
        assert!(t.collapse(9) == 9 && t.leaves.len() == 1 && t.kind.len() == 1);
    }
    #[test]
    fn collapse_shrinks_to_one_leaf() {
        let mut t = new_tree(3);
        t.leaves[0].point = 5; // non-zero so the 4-field copy is asserted
        t.leaves[0].mark = 2;
        t.leaves[0].scroll = 1;
        assert!(t.split(0, true));
        let second = t.leaves.len() - 1;
        let new_idx = t.collapse(second);
        assert!(new_idx == 0);
        assert!(t.leaves.len() == 1 && t.kind.len() == 1 && t.which.len() == 1);
        assert!(t.nodes.is_empty() && t.root == 0);
        // the survivor is the PASSED leaf (the split copy): all four fields kept
        assert!(t.leaves[0].buffer == 3 && t.leaves[0].point == 5
            && t.leaves[0].mark == 2 && t.leaves[0].scroll == 1);
        // repeated cycles stay compact
        for i in 0..5 {
            assert!(t.split(0, i % 2 == 0));
            let b = t.leaves[t.leaves.len() - 1].buffer;
            assert!(b == 3);
            assert!(t.collapse(t.leaves.len() - 1) == 0);
            assert!(t.leaves.len() == 1 && t.kind.len() == 1);
        }
    }
}
