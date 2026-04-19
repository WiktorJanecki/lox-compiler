pub type Id = String;
pub enum Primary{
    // literals
    Number(f64),
    String(String),
    Bool(bool),
    Nil,
    This,

    Identifier(Id),
    Super(Id),
    Grouping(Box<Primary>) // TODO: change to expr

}
pub enum Expr{
    Primary(Primary),
}