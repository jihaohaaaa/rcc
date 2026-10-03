use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    Void,
    Bool,
    Char {
        is_signed: bool,
    },
    Short {
        is_signed: bool,
    },
    Int {
        is_signed: bool,
    },
    Long {
        is_signed: bool,
    },
    LongLong {
        is_signed: bool,
    },
    Float,
    Double,
    Pointer(Box<Type>),
    Array {
        elem: Box<Type>,
        len: Option<usize>,
    },
    Struct(Rc<RefCell<StructType>>),
    Union(Rc<RefCell<UnionType>>),
    Enum(Rc<EnumType>),
    Function {
        ret: Box<Type>,
        params: Vec<Type>,
        is_variadic: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type {
    pub kind: TypeKind,
    pub is_const: bool,
    pub is_volatile: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructMember {
    pub name: String,
    pub ty: Type,
    pub offset: usize,
    pub bit_width: Option<usize>,
    pub bit_offset: Option<usize>,
}

#[derive(Clone)]
pub struct StructType {
    pub tag: Option<String>,
    pub members: Vec<StructMember>,
    pub size: usize,
    pub align: usize,
    pub is_complete: bool,
}

impl PartialEq for StructType {
    fn eq(&self, other: &Self) -> bool {
        if self.tag.is_some() && self.tag == other.tag {
            return true;
        }
        self.size == other.size
            && self.align == other.align
            && self.members.len() == other.members.len()
    }
}
impl Eq for StructType {}

impl std::fmt::Debug for StructType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StructType")
            .field("tag", &self.tag)
            .field("size", &self.size)
            .field("align", &self.align)
            .field("is_complete", &self.is_complete)
            .field("member_count", &self.members.len())
            .finish()
    }
}

#[derive(Clone)]
pub struct UnionType {
    pub tag: Option<String>,
    pub members: Vec<StructMember>,
    pub size: usize,
    pub align: usize,
    pub is_complete: bool,
}

impl PartialEq for UnionType {
    fn eq(&self, other: &Self) -> bool {
        if self.tag.is_some() && self.tag == other.tag {
            return true;
        }
        self.size == other.size
            && self.align == other.align
            && self.members.len() == other.members.len()
    }
}
impl Eq for UnionType {}

impl std::fmt::Debug for UnionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnionType")
            .field("tag", &self.tag)
            .field("size", &self.size)
            .field("align", &self.align)
            .field("is_complete", &self.is_complete)
            .field("member_count", &self.members.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    pub name: String,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumType {
    pub tag: Option<String>,
    pub variants: Vec<EnumVariant>,
}

impl Type {
    pub fn new(kind: TypeKind) -> Self {
        Self {
            kind,
            is_const: false,
            is_volatile: false,
        }
    }

    pub fn void() -> Self {
        Self::new(TypeKind::Void)
    }

    pub fn bool_ty() -> Self {
        Self::new(TypeKind::Bool)
    }

    pub fn char_ty() -> Self {
        Self::new(TypeKind::Char { is_signed: true })
    }

    pub fn uchar_ty() -> Self {
        Self::new(TypeKind::Char { is_signed: false })
    }

    pub fn short_ty() -> Self {
        Self::new(TypeKind::Short { is_signed: true })
    }

    pub fn ushort_ty() -> Self {
        Self::new(TypeKind::Short { is_signed: false })
    }

    pub fn int_ty() -> Self {
        Self::new(TypeKind::Int { is_signed: true })
    }

    pub fn uint_ty() -> Self {
        Self::new(TypeKind::Int { is_signed: false })
    }

    pub fn long_ty() -> Self {
        Self::new(TypeKind::Long { is_signed: true })
    }

    pub fn ulong_ty() -> Self {
        Self::new(TypeKind::Long { is_signed: false })
    }

    pub fn longlong_ty() -> Self {
        Self::new(TypeKind::LongLong { is_signed: true })
    }

    pub fn ulonglong_ty() -> Self {
        Self::new(TypeKind::LongLong { is_signed: false })
    }

    pub fn float_ty() -> Self {
        Self::new(TypeKind::Float)
    }

    pub fn double_ty() -> Self {
        Self::new(TypeKind::Double)
    }

    pub fn void_ptr_ty() -> Self {
        Self::void().pointer_to()
    }

    pub fn pointer_to(self) -> Self {
        Self::new(TypeKind::Pointer(Box::new(self)))
    }

    pub fn array_of(self, len: Option<usize>) -> Self {
        Self::new(TypeKind::Array {
            elem: Box::new(self),
            len,
        })
    }

    pub fn size(&self) -> usize {
        match &self.kind {
            TypeKind::Void => 1, // GNU C extension: sizeof(void) == 1
            TypeKind::Bool | TypeKind::Char { .. } => 1,
            TypeKind::Short { .. } => 2,
            TypeKind::Int { .. } | TypeKind::Enum { .. } | TypeKind::Float => 4,
            TypeKind::Long { .. }
            | TypeKind::LongLong { .. }
            | TypeKind::Double
            | TypeKind::Pointer(_) => 8,
            TypeKind::Array { elem, len } => elem.size() * len.unwrap_or(0),
            TypeKind::Struct(s) => s.borrow().size,
            TypeKind::Union(u) => u.borrow().size,
            TypeKind::Function { .. } => 1,
        }
    }

    pub fn align(&self) -> usize {
        match &self.kind {
            TypeKind::Void | TypeKind::Bool | TypeKind::Char { .. } => 1,
            TypeKind::Short { .. } => 2,
            TypeKind::Int { .. } | TypeKind::Enum { .. } | TypeKind::Float => 4,
            TypeKind::Long { .. }
            | TypeKind::LongLong { .. }
            | TypeKind::Double
            | TypeKind::Pointer(_) => 8,
            TypeKind::Array { elem, .. } => elem.align(),
            TypeKind::Struct(s) => s.borrow().align,
            TypeKind::Union(u) => u.borrow().align,
            TypeKind::Function { .. } => 8,
        }
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self.kind,
            TypeKind::Bool
                | TypeKind::Char { .. }
                | TypeKind::Short { .. }
                | TypeKind::Int { .. }
                | TypeKind::Long { .. }
                | TypeKind::LongLong { .. }
                | TypeKind::Enum { .. }
        )
    }

    pub fn is_signed_integer(&self) -> bool {
        match &self.kind {
            TypeKind::Char { is_signed }
            | TypeKind::Short { is_signed }
            | TypeKind::Int { is_signed }
            | TypeKind::Long { is_signed }
            | TypeKind::LongLong { is_signed } => *is_signed,
            TypeKind::Enum { .. } => true,
            _ => false,
        }
    }

    pub fn is_pointer(&self) -> bool {
        matches!(self.kind, TypeKind::Pointer(_))
    }

    pub fn is_char(&self) -> bool {
        matches!(self.kind, TypeKind::Char { .. })
    }

    pub fn is_array(&self) -> bool {
        matches!(self.kind, TypeKind::Array { .. })
    }

    pub fn is_function(&self) -> bool {
        matches!(self.kind, TypeKind::Function { .. })
    }

    pub fn is_void(&self) -> bool {
        matches!(self.kind, TypeKind::Void)
    }

    pub fn is_struct(&self) -> bool {
        matches!(self.kind, TypeKind::Struct(_))
    }

    pub fn is_union(&self) -> bool {
        matches!(self.kind, TypeKind::Union(_))
    }

    pub fn is_scalar(&self) -> bool {
        self.is_integer()
            || self.is_pointer()
            || matches!(self.kind, TypeKind::Float | TypeKind::Double)
    }

    pub fn get_pointer_base(&self) -> Option<&Type> {
        match &self.kind {
            TypeKind::Pointer(base) => Some(base),
            TypeKind::Array { elem, .. } => Some(elem),
            _ => None,
        }
    }
}
