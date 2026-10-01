use crate::span::Span;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Int(i64),
    Float(f64),
    Char(char),
    String(String),
    Ident(String),

    // Keywords
    Return,
    If,
    Else,
    While,
    For,
    Do,
    Break,
    Continue,
    Switch,
    Case,
    Default,
    Goto,
    Sizeof,
    Alignof,
    Typedef,
    Struct,
    Union,
    Enum,
    Static,
    Extern,
    Const,
    Volatile,
    Auto,
    Register,
    Inline,
    Restrict,
    Atomic,
    Attribute,
    Asm,
    Extension,
    Void,
    Bool,
    CharKw,
    Short,
    IntKw,
    Long,
    Signed,
    Unsigned,
    FloatKw,
    Double,

    // Operators & Punctuation
    Plus,          // +
    Minus,         // -
    Star,          // *
    Slash,         // /
    Percent,       // %
    Amp,           // &
    Pipe,          // |
    Caret,         // ^
    Tilde,         // ~
    Exclaim,       // !
    AmpAmp,        // &&
    PipePipe,      // ||
    Shl,           // <<
    Shr,           // >>
    Eq,            // ==
    Ne,            // !=
    Lt,            // <
    Le,            // <=
    Gt,            // >
    Ge,            // >=
    Assign,        // =
    PlusAssign,    // +=
    MinusAssign,   // -=
    StarAssign,    // *=
    SlashAssign,   // /=
    PercentAssign, // %=
    AmpAssign,     // &=
    PipeAssign,    // |=
    CaretAssign,   // ^=
    ShlAssign,     // <<=
    ShrAssign,     // >>=
    PlusPlus,      // ++
    MinusMinus,    // --
    Arrow,         // ->
    Dot,           // .
    Question,      // ?
    Colon,         // :
    Comma,         // ,
    Semicolon,     // ;
    LParen,        // (
    RParen,        // )
    LBracket,      // [
    RBracket,      // ]
    LBrace,        // {
    RBrace,        // }
    Ellipsis,      // ...
    Hash,          // #
    HashHash,      // ##

    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub fn eof(span: Span) -> Self {
        Self {
            kind: TokenKind::Eof,
            span,
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Int(v) => write!(f, "{}", v),
            TokenKind::Float(v) => write!(f, "{}", v),
            TokenKind::Char(c) => write!(f, "{:?}", c),
            TokenKind::String(s) => write!(f, "{:?}", s),
            TokenKind::Ident(s) => write!(f, "{}", s),
            TokenKind::Return => write!(f, "return"),
            TokenKind::If => write!(f, "if"),
            TokenKind::Else => write!(f, "else"),
            TokenKind::While => write!(f, "while"),
            TokenKind::For => write!(f, "for"),
            TokenKind::Do => write!(f, "do"),
            TokenKind::Break => write!(f, "break"),
            TokenKind::Continue => write!(f, "continue"),
            TokenKind::Switch => write!(f, "switch"),
            TokenKind::Case => write!(f, "case"),
            TokenKind::Default => write!(f, "default"),
            TokenKind::Goto => write!(f, "goto"),
            TokenKind::Sizeof => write!(f, "sizeof"),
            TokenKind::Alignof => write!(f, "_Alignof"),
            TokenKind::Typedef => write!(f, "typedef"),
            TokenKind::Struct => write!(f, "struct"),
            TokenKind::Union => write!(f, "union"),
            TokenKind::Enum => write!(f, "enum"),
            TokenKind::Static => write!(f, "static"),
            TokenKind::Extern => write!(f, "extern"),
            TokenKind::Const => write!(f, "const"),
            TokenKind::Volatile => write!(f, "volatile"),
            TokenKind::Auto => write!(f, "auto"),
            TokenKind::Register => write!(f, "register"),
            TokenKind::Inline => write!(f, "inline"),
            TokenKind::Restrict => write!(f, "restrict"),
            TokenKind::Atomic => write!(f, "_Atomic"),
            TokenKind::Attribute => write!(f, "__attribute__"),
            TokenKind::Asm => write!(f, "__asm__"),
            TokenKind::Extension => write!(f, "__extension__"),
            TokenKind::Void => write!(f, "void"),
            TokenKind::Bool => write!(f, "_Bool"),
            TokenKind::CharKw => write!(f, "char"),
            TokenKind::Short => write!(f, "short"),
            TokenKind::IntKw => write!(f, "int"),
            TokenKind::Long => write!(f, "long"),
            TokenKind::Signed => write!(f, "signed"),
            TokenKind::Unsigned => write!(f, "unsigned"),
            TokenKind::FloatKw => write!(f, "float"),
            TokenKind::Double => write!(f, "double"),
            TokenKind::Plus => write!(f, "+"),
            TokenKind::Minus => write!(f, "-"),
            TokenKind::Star => write!(f, "*"),
            TokenKind::Slash => write!(f, "/"),
            TokenKind::Percent => write!(f, "%"),
            TokenKind::Amp => write!(f, "&"),
            TokenKind::Pipe => write!(f, "|"),
            TokenKind::Caret => write!(f, "^"),
            TokenKind::Tilde => write!(f, "~"),
            TokenKind::Exclaim => write!(f, "!"),
            TokenKind::AmpAmp => write!(f, "&&"),
            TokenKind::PipePipe => write!(f, "||"),
            TokenKind::Shl => write!(f, "<<"),
            TokenKind::Shr => write!(f, ">>"),
            TokenKind::Eq => write!(f, "=="),
            TokenKind::Ne => write!(f, "!="),
            TokenKind::Lt => write!(f, "<"),
            TokenKind::Le => write!(f, "<="),
            TokenKind::Gt => write!(f, ">"),
            TokenKind::Ge => write!(f, ">="),
            TokenKind::Assign => write!(f, "="),
            TokenKind::PlusAssign => write!(f, "+="),
            TokenKind::MinusAssign => write!(f, "-="),
            TokenKind::StarAssign => write!(f, "*="),
            TokenKind::SlashAssign => write!(f, "/="),
            TokenKind::PercentAssign => write!(f, "%="),
            TokenKind::AmpAssign => write!(f, "&="),
            TokenKind::PipeAssign => write!(f, "|="),
            TokenKind::CaretAssign => write!(f, "^="),
            TokenKind::ShlAssign => write!(f, "<<="),
            TokenKind::ShrAssign => write!(f, ">>="),
            TokenKind::PlusPlus => write!(f, "++"),
            TokenKind::MinusMinus => write!(f, "--"),
            TokenKind::Arrow => write!(f, "->"),
            TokenKind::Dot => write!(f, "."),
            TokenKind::Question => write!(f, "?"),
            TokenKind::Colon => write!(f, ":"),
            TokenKind::Comma => write!(f, ","),
            TokenKind::Semicolon => write!(f, ";"),
            TokenKind::LParen => write!(f, "("),
            TokenKind::RParen => write!(f, ")"),
            TokenKind::LBracket => write!(f, "["),
            TokenKind::RBracket => write!(f, "]"),
            TokenKind::LBrace => write!(f, "{{"),
            TokenKind::RBrace => write!(f, "}}"),
            TokenKind::Ellipsis => write!(f, "..."),
            TokenKind::Hash => write!(f, "#"),
            TokenKind::HashHash => write!(f, "##"),
            TokenKind::Eof => write!(f, "<EOF>"),
        }
    }
}
