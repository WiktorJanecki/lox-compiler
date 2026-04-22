// by making id indices to ID hashmap we can reduce memory footprint
// TODO: benchmark after implementing working compiler
pub type Id = String;

// https://blog.trailofbits.com/2024/05/02/the-life-and-times-of-an-abstract-syntax-tree/
// in this article it is stated that it might be possible to setup node array to be in post-visit order
// then instead of indexing something like stack based evaluation may be faster
// TODO: investigate
pub struct Ast {
    nodes: Vec<Node>,
    pub program: Vec<NodeID>, // Vec<decl>
}
impl Ast {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            program: Vec::new(),
        }
    }

    pub fn push(&mut self, node: Node) -> NodeID {
        let id = self.nodes.len();
        self.nodes.push(node);
        id
    }
}

pub enum Operator {
    // Equality
    Eq,  // ==
    Neq, // !=
    Geq, // >=
    Leq, // <=
    Less,
    Greater,

    // Math
    Plus,
    Minus,
    Mul,
    Div,

    // Logic
    Not,
}

pub type NodeID = usize;

#[allow(unused)]
pub enum Node {
    // declarations
    VarDecl(Id, NodeID), // expr
    Stmt(NodeID),

    // statements
    ExprStmt(NodeID),          // expr
    PrintStmt(NodeID),         // expr
    ReturnStmt(NodeID),        // expr
    WhileStmt(NodeID, NodeID), // expr, stmt
    Block(Vec<NodeID>),        // decls

    // expressions
    Assignment(Id, NodeID), // stores other assignment // TODO: store calling
    LogicOr(NodeID, NodeID),
    LogicAnd(NodeID, NodeID),
    Equality(NodeID, Operator, NodeID),
    Comparison(NodeID, Operator, NodeID),
    Term(NodeID, Operator, NodeID),
    Factor(NodeID, Operator, NodeID),
    Unary(NodeID, Operator),
    Call, // TODO:
    Identifier(Id),
    Super(Id),
    Grouping(NodeID), //expr

    // literals
    Number(f64),
    String(String),
    Bool(bool),
    Nil,
    This,
}
