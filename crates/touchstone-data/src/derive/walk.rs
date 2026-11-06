//! The walk over the text-block graph.
//!
//! NMR reads every script the game can reach and keeps who calls whom (TBInfo `Called From`) and
//! what the clauses do (a summoned monster, a given item, a cast or learned spell). The order of
//! its output follows the order in which it visited blocks, which was learned from the stock
//! `.mdb`: roots are read smallest block first, in two phases (what is in the game before any
//! script is read, then the rest), and a block more than [`MAX_DEPTH`] links from its root waits
//! for the second phase. The block a root belongs to (`Origin`) is passed down, because an item
//! a monster's ask tree gives is credited to the monster (`NPC #n`).
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

use super::script::{Act, Blocks, LineKind, Ref, RefKind, act, class_of, lines, reference};

/// Links from a root that are read in the first phase.
const MAX_DEPTH: u32 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Origin {
    Room,
    Monster(i32),
    Spell(i32),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Root {
    pub block: i32,
    pub origin: Origin,
}

/// One entry of a block's `Called From`: the calling block, whether it called through `random`
/// (`Textblock(rndm)`), and the percentage of the calling line when it is a random line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Caller {
    pub block: i32,
    pub random: bool,
    pub pct: Option<i64>,
}

#[derive(Default)]
pub(super) struct Walk {
    /// Script blocks in the order they were read, with their origin.
    pub order: Vec<(i32, Origin)>,
    pub called: HashMap<i32, Vec<Caller>>,
    /// Blocks that were reached only to be shown.
    pub shown: HashSet<i32>,
    pub scripts: HashSet<i32>,
}

struct State<'a> {
    blocks: &'a Blocks,
    spell_known: &'a dyn Fn(i32) -> bool,
    walk: Walk,
}

impl State<'_> {
    fn call(&mut self, target: i32, c: Caller) {
        let list = self.walk.called.entry(target).or_default();
        if !list.contains(&c) {
            list.push(c);
        }
    }

    /// A block that is shown: only its link goes on, as a script when it has actions.
    fn show(&mut self, t: i32, origin: Origin, depth: u32, queue: &mut Queue) {
        if !self.walk.shown.insert(t) {
            return;
        }
        let Some(link) = self.blocks.get(&t).map(|b| b.link) else {
            return;
        };
        if link != 0
            && let Some(next) = self.blocks.get(&link)
        {
            self.call(
                link,
                Caller {
                    block: t,
                    random: false,
                    pct: None,
                },
            );
            if next.acts {
                queue.push(self, link, origin, depth);
            } else {
                self.show(link, origin, depth, queue);
            }
        }
    }
}

struct Queue {
    heap: BinaryHeap<Reverse<i32>>,
    known: HashMap<i32, (Origin, u32)>,
    last: bool,
    deferred: Vec<Root>,
}

impl Queue {
    fn push(&mut self, st: &State<'_>, t: i32, origin: Origin, depth: u32) {
        if st.walk.scripts.contains(&t) || self.known.contains_key(&t) {
            return;
        }
        if depth > MAX_DEPTH && !self.last {
            self.deferred.push(Root { block: t, origin });
            return;
        }
        self.known
            .insert(t, (origin, if self.last { 0 } else { depth }));
        self.heap.push(Reverse(t));
    }
}

/// Walk from the roots of each phase in turn. `spell_known` tells whether a spell is part of
/// the game, which a `checkspell` clause needs before its branch counts.
pub(super) fn walk(
    blocks: &Blocks,
    phases: &[Vec<Root>],
    spell_known: &dyn Fn(i32) -> bool,
) -> Walk {
    let mut st = State {
        blocks,
        spell_known,
        walk: Walk::default(),
    };
    let mut pending: Vec<Root> = Vec::new();
    for (i, roots) in phases.iter().enumerate() {
        let last = i + 1 == phases.len();
        let mut q = Queue {
            heap: BinaryHeap::new(),
            known: HashMap::new(),
            last,
            deferred: Vec::new(),
        };
        let carried = if last {
            std::mem::take(&mut pending)
        } else {
            Vec::new()
        };
        for r in roots.iter().chain(carried.iter()) {
            if r.block != 0
                && blocks.contains_key(&r.block)
                && !q.known.contains_key(&r.block)
                && !st.walk.scripts.contains(&r.block)
            {
                q.known.insert(r.block, (r.origin, 0));
                q.heap.push(Reverse(r.block));
            }
        }
        while let Some(Reverse(n)) = q.heap.pop() {
            if st.walk.scripts.contains(&n) {
                continue;
            }
            let (origin, depth) = q.known[&n];
            st.walk.scripts.insert(n);
            st.walk.order.push((n, origin));
            visit(&mut st, &mut q, n, origin, depth);
        }
        pending.extend(q.deferred);
    }
    // Roots of the last phase that were never read still exist as rows.
    st.walk
}

fn visit(st: &mut State<'_>, q: &mut Queue, n: i32, origin: Origin, depth: u32) {
    let blocks = st.blocks;
    let Some(block) = blocks.get(&n) else { return };
    let mut refs: Vec<(Ref, Option<i64>)> = Vec::new();
    for line in lines(&block.text) {
        let pct = (line.kind == LineKind::Random).then_some(line.pct);
        for clause in &line.clauses {
            if let Some(r) = reference(clause) {
                refs.push((r, pct));
            }
        }
    }
    for (r, pct) in refs {
        let t = r.target;
        if t == 0 || !blocks.contains_key(&t) {
            continue;
        }
        if let Some(spell) = r.spell
            && !(st.spell_known)(spell)
        {
            continue;
        }
        st.call(
            t,
            Caller {
                block: n,
                random: r.kind == RefKind::Random,
                pct,
            },
        );
        match r.kind {
            RefKind::Random => q.push(st, t, origin, depth + 1),
            RefKind::Run => {
                if blocks[&t].acts {
                    q.push(st, t, origin, depth + 1);
                } else {
                    st.show(t, origin, depth + 1, q);
                }
            }
            RefKind::Text => st.show(t, origin, depth + 1, q),
        }
    }
    if block.link != 0 && blocks.contains_key(&block.link) {
        let l = block.link;
        st.call(
            l,
            Caller {
                block: n,
                random: false,
                pct: None,
            },
        );
        if blocks[&l].acts {
            q.push(st, l, origin, depth + 1);
        } else {
            st.show(l, origin, depth + 1, q);
        }
    }
}

/// What the clauses of the visited blocks do, in visit order, each once.
#[derive(Default, PartialEq)]
pub(super) struct Effects {
    /// `(monster, block, line percentage)`
    pub summons: Vec<(i32, i32, Option<i64>)>,
    /// `(item, block, line percentage, origin)`
    pub gives: Vec<(i32, i32, Option<i64>, Origin)>,
    /// `(spell, block, line percentage)`
    pub casts: Vec<(i32, i32, Option<i64>)>,
    /// `(spell, block, origin, classes of the line)`
    pub learns: Vec<(i32, i32, Origin, Vec<i32>)>,
}

pub(super) fn effects(blocks: &Blocks, order: &[(i32, Origin)]) -> Effects {
    let mut fx = Effects::default();
    for &(n, origin) in order {
        let Some(block) = blocks.get(&n) else {
            continue;
        };
        for line in lines(&block.text) {
            let pct = (line.kind == LineKind::Random).then_some(line.pct);
            let classes: Vec<i32> = line.clauses.iter().filter_map(|c| class_of(c)).collect();
            for clause in &line.clauses {
                let Some((a, x)) = act(clause) else { continue };
                match a {
                    Act::Summon => push_new(&mut fx.summons, (x, n, pct)),
                    Act::GiveItem => push_new(&mut fx.gives, (x, n, pct, origin)),
                    Act::Cast => push_new(&mut fx.casts, (x, n, pct)),
                    Act::LearnSpell => {
                        let e = (x, n, origin, classes.clone());
                        if !fx.learns.contains(&e) {
                            fx.learns.push(e);
                        }
                    }
                }
            }
        }
    }
    fx
}

fn push_new<T: PartialEq>(v: &mut Vec<T>, e: T) {
    if !v.contains(&e) {
        v.push(e);
    }
}
