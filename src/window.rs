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
    pub top: usize,
    pub height: usize,
    pub col: usize,
    pub width: usize,
}

pub fn new_tree(buffer: usize) -> Tree {
    Tree {
        kind: vec![0],
        which: vec![0],
        leaves: vec![Leaf { buffer: buffer, point: 0, mark: 0, scroll: 0 }],
        nodes: vec![],
        root: 0,
    }
}

impl Tree {
    pub fn collect(self: &Tree, mut slot: usize) -> Vec<usize> {
        if self.kind[slot] == 0 {
            vec![self.which[slot]]
        } else {
            let mut k = self.which[slot];
            let mut acc = self.collect(self.nodes[k].a);
            acc.extend(self.collect(self.nodes[k].b).iter());
            acc
        }
    }

    pub fn flatten(self: &Tree) -> Vec<usize> {
        self.collect(self.root)
    }

    pub fn layout(self: &Tree, mut slot: usize, mut top: usize, mut height: usize, mut col: usize, mut width: usize) -> Vec<Frame> {
        if self.kind[slot] == 0 {
            vec![Frame { leaf: self.which[slot], top: top, height: height, col: col, width: width }]
        } else {
            let mut k = self.which[slot];
            let vertical = self.nodes[k].vertical;
            if vertical && height >= 2 {
                let h1 = (height + 1) / 2;
                let mut a = self.layout(self.nodes[k].a, top, h1, col, width);
                a.extend(self.layout(self.nodes[k].b, top + h1, height - h1, col, width));
                a
            } else if width >= 2 {
                let w1 = (width + 1) / 2;
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

    pub fn find_slot(self: &Tree, mut slot: usize, leaf: usize) -> usize {
        if self.kind[slot] == 0 {
            if self.which[slot] == leaf {
                slot
            } else {
                self.kind.len()
            }
        } else {
            let mut k = self.which[slot];
            let mut s = self.find_slot(self.nodes[k].a, leaf);
            if s == self.kind.len() {
                s = self.find_slot(self.nodes[k].b, leaf);
            }
            s
        }
    }

    pub fn split(self: &mut Tree, mut leaf: usize, vertical: bool) -> bool {
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
            self.nodes.push(Node { vertical: vertical, a: sa, b: sb });
            self.kind[s] = 1;
            self.which[s] = k;
            return true;
        }
        false
    }

    pub fn collapse(self: &mut Tree, mut leaf: usize) {
        let s = self.find_slot(self.root, leaf);
        if s < self.kind.len() {
            self.root = self.kind.len();
            self.kind.push(0);
            self.which.push(leaf);
        }
    }
}
