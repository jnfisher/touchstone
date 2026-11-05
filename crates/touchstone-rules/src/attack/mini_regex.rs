//! A small backtracking matcher, just enough for the patterns `CalculateAttack` runs over a
//! weapon's spell text (VBScript.RegExp is not available here and the crate takes no regex
//! dependency). It follows the same rules as VBScript: leftmost match, greedy quantifiers that
//! backtrack, captured groups numbered by their opening bracket, ASCII `\d`.
//!
//! Supported: literals, `\x` escapes, `\d`, `[...]`/`[^...]` classes with escaped members, `(...)`,
//! `(?:...)`, `|`, and the quantifiers `?`, `+`, `*`. Anything else in a pattern is a bug in this
//! crate, so a malformed pattern panics.

#[derive(Debug)]
enum Node {
    Lit(char),
    Digit,
    Class { negated: bool, members: Vec<char> },
    Seq(Vec<Node>),
    Alt(Vec<Node>),
    Group(Box<Node>, Option<usize>),
    Repeat(Box<Node>, usize, Option<usize>),
}

/// One match: the whole text and the groups that took part (`None` for the others).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub text: String,
    pub groups: Vec<Option<String>>,
}

pub struct Regex {
    root: Node,
    groups: usize,
    ignore_case: bool,
}

type Caps = Vec<Option<(usize, usize)>>;

struct Parser<'a> {
    chars: Vec<char>,
    pos: usize,
    groups: usize,
    pattern: &'a str,
}

impl Parser<'_> {
    fn fail(&self, why: &str) -> ! {
        panic!("bad pattern {:?} at {}: {why}", self.pattern, self.pos)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn alt(&mut self) -> Node {
        let mut arms = vec![self.seq()];
        while self.peek() == Some('|') {
            self.pos += 1;
            arms.push(self.seq());
        }
        if arms.len() == 1 {
            arms.remove(0)
        } else {
            Node::Alt(arms)
        }
    }

    fn seq(&mut self) -> Node {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom();
            items.push(self.quantified(atom));
        }
        Node::Seq(items)
    }

    fn quantified(&mut self, atom: Node) -> Node {
        let (min, max) = match self.peek() {
            Some('?') => (0, Some(1)),
            Some('+') => (1, None),
            Some('*') => (0, None),
            _ => return atom,
        };
        self.pos += 1;
        Node::Repeat(Box::new(atom), min, max)
    }

    fn atom(&mut self) -> Node {
        let c = self.peek().unwrap_or_else(|| self.fail("ended early"));
        self.pos += 1;
        match c {
            '(' => {
                let capture = if self.chars[self.pos..].starts_with(&['?', ':']) {
                    self.pos += 2;
                    None
                } else {
                    self.groups += 1;
                    Some(self.groups - 1)
                };
                let inner = self.alt();
                if self.peek() != Some(')') {
                    self.fail("missing )");
                }
                self.pos += 1;
                Node::Group(Box::new(inner), capture)
            }
            '[' => {
                let negated = self.peek() == Some('^');
                if negated {
                    self.pos += 1;
                }
                let mut members = Vec::new();
                loop {
                    let m = self.peek().unwrap_or_else(|| self.fail("missing ]"));
                    self.pos += 1;
                    match m {
                        ']' => break,
                        '\\' => {
                            let e = self.peek().unwrap_or_else(|| self.fail("dangling \\"));
                            self.pos += 1;
                            members.push(e);
                        }
                        other => members.push(other),
                    }
                }
                Node::Class { negated, members }
            }
            '\\' => {
                let e = self.peek().unwrap_or_else(|| self.fail("dangling \\"));
                self.pos += 1;
                if e == 'd' { Node::Digit } else { Node::Lit(e) }
            }
            other => Node::Lit(other),
        }
    }
}

impl Regex {
    /// Compiles `pattern`; `ignore_case` folds ASCII letters in literals.
    #[must_use]
    pub fn new(pattern: &str, ignore_case: bool) -> Self {
        let mut p = Parser {
            chars: pattern.chars().collect(),
            pos: 0,
            groups: 0,
            pattern,
        };
        let root = p.alt();
        if p.pos != p.chars.len() {
            p.fail("unbalanced )");
        }
        Self {
            root,
            groups: p.groups,
            ignore_case,
        }
    }

    /// Every non-overlapping match, left to right.
    #[must_use]
    pub fn find_all(&self, text: &str) -> Vec<Match> {
        let s: Vec<char> = text.chars().collect();
        let mut out = Vec::new();
        let mut from = 0;
        while from <= s.len() {
            let mut found = None;
            for start in from..=s.len() {
                let mut caps: Caps = vec![None; self.groups];
                let mut end = 0;
                if self.m(&self.root, &s, start, &mut caps, &mut |j, _| {
                    end = j;
                    true
                }) {
                    found = Some((start, end, caps));
                    break;
                }
            }
            let Some((start, end, caps)) = found else {
                break;
            };
            let grab = |a: usize, b: usize| s[a..b].iter().collect::<String>();
            out.push(Match {
                text: grab(start, end),
                groups: caps.iter().map(|c| c.map(|(a, b)| grab(a, b))).collect(),
            });
            from = if end == start { end + 1 } else { end };
        }
        out
    }

    fn one_char(&self, n: &Node, c: char) -> Option<bool> {
        match n {
            Node::Lit(l) => Some(if self.ignore_case {
                l.eq_ignore_ascii_case(&c)
            } else {
                *l == c
            }),
            Node::Digit => Some(c.is_ascii_digit()),
            Node::Class { negated, members } => Some(members.contains(&c) != *negated),
            _ => None,
        }
    }

    fn m(
        &self,
        n: &Node,
        s: &[char],
        i: usize,
        caps: &mut Caps,
        k: &mut dyn FnMut(usize, &mut Caps) -> bool,
    ) -> bool {
        if let Some(c) = s.get(i)
            && let Some(ok) = self.one_char(n, *c)
        {
            return ok && k(i + 1, caps);
        }
        match n {
            Node::Lit(_) | Node::Digit | Node::Class { .. } => false,
            Node::Seq(items) => self.m_seq(items, s, i, caps, k),
            Node::Alt(arms) => {
                for arm in arms {
                    let saved = caps.clone();
                    if self.m(arm, s, i, caps, k) {
                        return true;
                    }
                    *caps = saved;
                }
                false
            }
            Node::Group(inner, None) => self.m(inner, s, i, caps, k),
            Node::Group(inner, Some(idx)) => {
                let idx = *idx;
                self.m(inner, s, i, caps, &mut |j, c| {
                    let old = c[idx];
                    c[idx] = Some((i, j));
                    if k(j, c) {
                        true
                    } else {
                        c[idx] = old;
                        false
                    }
                })
            }
            Node::Repeat(inner, min, max) => self.m_rep(inner, *min, *max, 0, s, i, caps, k),
        }
    }

    fn m_seq(
        &self,
        items: &[Node],
        s: &[char],
        i: usize,
        caps: &mut Caps,
        k: &mut dyn FnMut(usize, &mut Caps) -> bool,
    ) -> bool {
        match items.split_first() {
            None => k(i, caps),
            Some((first, rest)) => {
                self.m(first, s, i, caps, &mut |j, c| self.m_seq(rest, s, j, c, k))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn m_rep(
        &self,
        n: &Node,
        min: usize,
        max: Option<usize>,
        count: usize,
        s: &[char],
        i: usize,
        caps: &mut Caps,
        k: &mut dyn FnMut(usize, &mut Caps) -> bool,
    ) -> bool {
        // A repeated single character is a run: take it all, then give characters back.
        if count == 0 && matches!(n, Node::Lit(_) | Node::Digit | Node::Class { .. }) {
            let mut run = 0;
            while max.is_none_or(|mx| run < mx)
                && s.get(i + run)
                    .is_some_and(|c| self.one_char(n, *c) == Some(true))
            {
                run += 1;
            }
            let mut take = run;
            loop {
                if take >= min && k(i + take, caps) {
                    return true;
                }
                if take == 0 {
                    return false;
                }
                take -= 1;
            }
        }
        if max.is_none_or(|mx| count < mx) {
            let saved = caps.clone();
            let more = self.m(n, s, i, caps, &mut |j, c| {
                if j == i && count >= min {
                    return false;
                }
                self.m_rep(n, min, max, count + 1, s, j, c, k)
            });
            if more {
                return true;
            }
            *caps = saved;
        }
        count >= min && k(i, caps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leftmost_greedy_with_groups() {
        let r = Regex::new(r"\[([^\]]+), (\d+)%\]", false);
        let m = r.find_all("x [ab, 5%] y [c, 100%]");
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].text, "[ab, 5%]");
        assert_eq!(m[1].groups, vec![Some("c".into()), Some("100".into())]);
    }

    #[test]
    fn optional_groups_and_case() {
        let r = Regex::new(r"a(?:(b)|(c))?d", true);
        let m = r.find_all("ABD acd ad");
        assert_eq!(m[0].groups, vec![Some("B".into()), None]);
        assert_eq!(m[1].groups, vec![None, Some("c".into())]);
        assert_eq!(m[2].groups, vec![None, None]);
    }

    #[test]
    fn backtracks_into_runs() {
        let r = Regex::new(r"[^x]+, (\d+) to", false);
        let m = r.find_all("a, 1 to b, 22 to");
        assert_eq!(m[0].groups, vec![Some("22".into())]);
    }
}
