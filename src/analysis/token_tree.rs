use itertools::Itertools;
use ra_ap_syntax::{NodeOrToken, SyntaxToken, ast};

type Not = NodeOrToken<ast::TokenTree, SyntaxToken>;

pub struct TtFlatten {
    stack: Vec<Not>,
}

impl TtFlatten {
    pub fn new(iter: impl Iterator<Item = Not>) -> TtFlatten {
        let mut stack = Vec::from_iter(iter);
        stack.reverse();
        TtFlatten { stack }
    }
}

impl Iterator for TtFlatten {
    type Item = SyntaxToken;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.stack.pop()? {
                Not::Node(tt) => {
                    let mut tt_vec = tt.token_trees_and_tokens().collect_vec();
                    tt_vec.reverse();
                    self.stack.extend(tt_vec);
                }
                Not::Token(token) => return Some(token),
            };
        }
    }
}

pub trait TtIterator {
    fn flatten_tt(self) -> TtFlatten
    where
        Self: Sized,
        Self: Iterator<Item = Not>,
    {
        TtFlatten::new(self)
    }
}

impl<T: Iterator<Item = Not>> TtIterator for T {}
