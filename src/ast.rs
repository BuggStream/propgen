use ra_ap_syntax::ast::edit::{AstNodeEdit, IndentLevel};
use ra_ap_syntax::ast::make;
use ra_ap_syntax::{ast, syntax_editor, AstToken, NodeOrToken, WalkEvent};
use ra_ap_syntax::syntax_editor::SyntaxEditor;

pub trait IndentAllLines: AstNodeEdit {
    #[must_use]
    fn indent_all_lines(&self, level: IndentLevel) -> Self {
        let node = self.syntax().clone_subtree();
        let Some(first_token) = node.first_token() else {
            return Self::cast(node).unwrap();
        };

        let mut editor = SyntaxEditor::new(node.clone());

        let start_whitespace = make::tokens::whitespace(&format!("{level}"));
        editor.insert(syntax_editor::Position::before(first_token), start_whitespace);

        let tokens = node
            .preorder_with_tokens()
            .filter_map(|event| match event {
                WalkEvent::Leave(NodeOrToken::Token(it)) => Some(it),
                _ => None,
            })
            .filter_map(ast::Whitespace::cast)
            .filter(|ws| ws.text().contains('\n'));
        for ws in tokens {
            let new_ws = make::tokens::whitespace(&format!("{}{level}", ws.syntax()));
            editor.replace(ws.syntax(), &new_ws);
        }
        let new_root = editor.finish().new_root().clone();

        Self::cast(new_root).unwrap()
    }
}

impl<N: AstNodeEdit> IndentAllLines for N {}
