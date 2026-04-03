pub enum SymbolKind {
    Attribute,
    BuiltinAttr,
    Const,
    ConstParam,
    CrateRoot,
    Derive,
    DeriveHelper,
    Enum,
    Field,
    Function,
    Method,
    Impl,
    InlineAsmRegOrRegClass,
    Label,
    LifetimeParam,
    Local,
    Macro,
    ProcMacro,
    Module,
    SelfParam,
    SelfType,
    Static,
    Struct,
    ToolModule,
    Trait,
    TypeAlias,
    TypeParam,
    Union,
    ValueParam,
    Variant,
}

pub enum HlTag {
    Symbol(SymbolKind),

    AttributeBracket,
    BoolLiteral,
    BuiltinType,
    ByteLiteral,
    CharLiteral,
    Comment,
    EscapeSequence,
    FormatSpecifier,
    InvalidEscapeSequence,
    Keyword,
    NumericLiteral,
    Operator(HlOperator),
    Punctuation(HlPunct),
    StringLiteral,
    UnresolvedReference,

    // For things which don't have a specific highlight.
    None,
}

enum Option {
    Some(T),
    None,
}
use Option::*;

enum Result<T, E> {
    Ok(T),
    Err(E),
}
use Result::*;
