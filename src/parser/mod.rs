use crate::ast::*;
use crate::diag::Diagnostics;
use crate::lexer::token::{Token, TokenKind};
use crate::types::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Parser<'a> {
    tokens: &'a [Token],
    cursor: usize,
    typedefs: Vec<HashMap<String, Type>>,
    struct_tags: Vec<HashMap<String, Rc<RefCell<StructType>>>>,
    union_tags: Vec<HashMap<String, Rc<RefCell<UnionType>>>>,
    enum_tags: Vec<HashMap<String, Rc<EnumType>>>,
    enum_constants: Vec<HashMap<String, i64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precedence {
    None,
    Comma,          // ,
    Assignment,     // = += -= etc
    Ternary,        // ?:
    LogicalOr,      // ||
    LogicalAnd,     // &&
    BitOr,          // |
    BitXor,         // ^
    BitAnd,         // &
    Equality,       // == !=
    Relational,     // < <= > >=
    Shift,          // << >>
    Additive,       // + -
    Multiplicative, // * / %
    Cast,           // (type)
    Unary,          // ++ -- + - ~ ! * & sizeof _Alignof
    Postfix,        // () [] . -> ++ --
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            cursor: 0,
            typedefs: vec![HashMap::new()],
            struct_tags: vec![HashMap::new()],
            union_tags: vec![HashMap::new()],
            enum_tags: vec![HashMap::new()],
            enum_constants: vec![HashMap::new()],
        }
    }

    fn peek(&self) -> &Token {
        if self.cursor < self.tokens.len() {
            &self.tokens[self.cursor]
        } else {
            &self.tokens[self.tokens.len() - 1]
        }
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    fn peek_next(&self) -> &Token {
        if self.cursor + 1 < self.tokens.len() {
            &self.tokens[self.cursor + 1]
        } else {
            &self.tokens[self.tokens.len() - 1]
        }
    }

    fn advance(&mut self) -> &Token {
        let idx = self.cursor;
        if self.cursor < self.tokens.len() {
            self.cursor += 1;
        }
        if idx < self.tokens.len() {
            &self.tokens[idx]
        } else {
            &self.tokens[self.tokens.len() - 1]
        }
    }

    fn match_token(&mut self, kind: &TokenKind) -> bool {
        if std::mem::discriminant(self.peek_kind()) == std::mem::discriminant(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: &TokenKind, diag: &mut Diagnostics) -> Option<Token> {
        let tok = self.peek().clone();
        if std::mem::discriminant(&tok.kind) == std::mem::discriminant(kind) {
            self.advance();
            Some(tok)
        } else {
            diag.error(
                tok.span,
                format!("Expected '{}', found '{}'", kind, tok.kind),
            );
            None
        }
    }

    fn enter_scope(&mut self) {
        self.typedefs.push(HashMap::new());
        self.struct_tags.push(HashMap::new());
        self.union_tags.push(HashMap::new());
        self.enum_tags.push(HashMap::new());
        self.enum_constants.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        self.typedefs.pop();
        self.struct_tags.pop();
        self.union_tags.pop();
        self.enum_tags.pop();
        self.enum_constants.pop();
    }

    fn is_typedef_name(&self, name: &str) -> bool {
        self.find_typedef(name).is_some()
    }

    fn find_typedef(&self, name: &str) -> Option<Type> {
        for scope in self.typedefs.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        None
    }

    fn add_typedef(&mut self, name: String, ty: Type) {
        if let Some(scope) = self.typedefs.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn find_enum_constant(&self, name: &str) -> Option<i64> {
        for scope in self.enum_constants.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(*val);
            }
        }
        None
    }

    fn skip_extension(&mut self) {
        while self.match_token(&TokenKind::Extension) {}
    }

    fn skip_attribute(&mut self) {
        while self.match_token(&TokenKind::Attribute) {
            if self.match_token(&TokenKind::LParen) {
                let mut depth = 1;
                while depth > 0 && self.peek_kind() != &TokenKind::Eof {
                    if self.match_token(&TokenKind::LParen) {
                        depth += 1;
                    } else if self.match_token(&TokenKind::RParen) {
                        depth -= 1;
                    } else {
                        self.advance();
                    }
                }
            }
        }
    }

    fn skip_asm(&mut self) {
        while self.match_token(&TokenKind::Asm) {
            while self.peek_kind() == &TokenKind::Volatile
                || self.peek_kind() == &TokenKind::Const
                || self.peek_kind() == &TokenKind::Goto
                || self.peek_kind() == &TokenKind::Inline
            {
                self.advance();
            }
            if self.match_token(&TokenKind::LParen) {
                let mut depth = 1;
                while depth > 0 && self.peek_kind() != &TokenKind::Eof {
                    if self.match_token(&TokenKind::LParen) {
                        depth += 1;
                    } else if self.match_token(&TokenKind::RParen) {
                        depth -= 1;
                    } else {
                        self.advance();
                    }
                }
            }
        }
    }

    pub fn parse_program(&mut self, diag: &mut Diagnostics) -> Program {
        let mut prog = Program::default();

        while self.peek_kind() != &TokenKind::Eof {
            if diag.messages.len() >= 100 {
                break;
            }
            self.skip_extension();
            self.skip_attribute();

            if self.match_token(&TokenKind::Semicolon) {
                continue;
            }

            let prev_cursor = self.cursor;
            self.parse_top_level_item(&mut prog, diag);
            if self.cursor == prev_cursor {
                // If parse_top_level_item didn't advance, skip to next semicolon/brace to recover
                self.advance();
                while self.peek_kind() != &TokenKind::Semicolon
                    && self.peek_kind() != &TokenKind::RBrace
                    && self.peek_kind() != &TokenKind::Eof
                {
                    self.advance();
                }
                if self.peek_kind() == &TokenKind::Semicolon
                    || self.peek_kind() == &TokenKind::RBrace
                {
                    self.advance();
                }
            }
        }

        prog
    }

    fn is_type_specifier_start(&self) -> bool {
        match self.peek_kind() {
            TokenKind::Void
            | TokenKind::Bool
            | TokenKind::CharKw
            | TokenKind::Short
            | TokenKind::IntKw
            | TokenKind::Long
            | TokenKind::Signed
            | TokenKind::Unsigned
            | TokenKind::FloatKw
            | TokenKind::Double
            | TokenKind::Struct
            | TokenKind::Union
            | TokenKind::Enum
            | TokenKind::Typedef
            | TokenKind::Static
            | TokenKind::Extern
            | TokenKind::Const
            | TokenKind::Volatile
            | TokenKind::Auto
            | TokenKind::Register
            | TokenKind::Inline
            | TokenKind::Restrict
            | TokenKind::Atomic
            | TokenKind::Extension
            | TokenKind::Attribute
            | TokenKind::Asm => true,
            TokenKind::Ident(name) => self.is_typedef_name(name),
            _ => false,
        }
    }

    fn parse_top_level_item(&mut self, prog: &mut Program, diag: &mut Diagnostics) {
        let start_span = self.peek().span;
        let (base_ty, is_typedef, is_static, is_extern) = self.parse_decl_specifiers(diag);

        if self.match_token(&TokenKind::Semicolon) {
            // e.g. struct Point { int x; int y; };
            return;
        }

        loop {
            let (ty, name, param_names) = self.parse_declarator(base_ty.clone(), diag);

            if is_typedef {
                self.add_typedef(name, ty);
            } else if let TypeKind::Function {
                ret,
                params,
                is_variadic,
            } = ty.kind
            {
                // Function declaration or definition
                if self.peek_kind() == &TokenKind::LBrace {
                    // Function definition
                    let mut full_params = Vec::new();
                    for (i, pty) in params.into_iter().enumerate() {
                        let pname = param_names.get(i).cloned().unwrap_or_default();
                        full_params.push((pty, pname));
                    }

                    self.enter_scope();
                    for (pty, pname) in &full_params {
                        if !pname.is_empty() {
                            // register local param
                        }
                        let _ = pty;
                    }

                    let body = self.parse_block(diag);
                    self.exit_scope();

                    let func = Function {
                        name: name.clone(),
                        ret_type: *ret,
                        params: full_params,
                        is_variadic,
                        body: Some(body),
                        is_static,
                        span: start_span.merge(self.peek().span),
                    };
                    if let Some(existing) = prog.functions.iter_mut().find(|f| f.name == name) {
                        if existing.body.is_none() {
                            *existing = func;
                        }
                    } else {
                        prog.functions.push(func);
                    }
                    return;
                } else {
                    // Function prototype declaration
                    let mut full_params = Vec::new();
                    for (i, pty) in params.into_iter().enumerate() {
                        let pname = param_names.get(i).cloned().unwrap_or_default();
                        full_params.push((pty, pname));
                    }
                    let func = Function {
                        name: name.clone(),
                        ret_type: *ret,
                        params: full_params,
                        is_variadic,
                        body: None,
                        is_static,
                        span: start_span.merge(self.peek().span),
                    };
                    if !prog.functions.iter().any(|f| f.name == name) {
                        prog.functions.push(func);
                    }
                }
            } else {
                // Global variable
                let init = if self.match_token(&TokenKind::Assign) {
                    Some(self.parse_initializer(diag))
                } else {
                    None
                };

                let gvar = GlobalVar {
                    name,
                    ty,
                    init,
                    is_static,
                    is_extern,
                    span: start_span.merge(self.peek().span),
                };
                prog.globals.push(gvar);
            }

            if self.match_token(&TokenKind::Comma) {
                continue;
            } else {
                self.expect(&TokenKind::Semicolon, diag);
                break;
            }
        }
    }

    fn parse_decl_specifiers(&mut self, diag: &mut Diagnostics) -> (Type, bool, bool, bool) {
        let mut is_typedef = false;
        let mut is_static = false;
        let mut is_extern = false;
        let mut is_const = false;
        let mut is_volatile = false;

        let mut has_void = false;
        let mut has_bool = false;
        let mut has_char = false;
        let mut has_short = false;
        let mut has_int = false;
        let mut long_count = 0;
        let mut has_signed = false;
        let mut has_unsigned = false;
        let mut has_float = false;
        let mut has_double = false;

        let mut custom_type: Option<Type> = None;

        while self.is_type_specifier_start() {
            match self.peek_kind() {
                TokenKind::Typedef => {
                    self.advance();
                    is_typedef = true;
                }
                TokenKind::Static => {
                    self.advance();
                    is_static = true;
                }
                TokenKind::Extern => {
                    self.advance();
                    is_extern = true;
                }
                TokenKind::Const => {
                    self.advance();
                    is_const = true;
                }
                TokenKind::Volatile => {
                    self.advance();
                    is_volatile = true;
                }
                TokenKind::Auto | TokenKind::Register | TokenKind::Inline | TokenKind::Restrict => {
                    self.advance();
                }
                TokenKind::Atomic => {
                    self.advance();
                    if self.match_token(&TokenKind::LParen) {
                        let (inner_base, _, _, _) = self.parse_decl_specifiers(diag);
                        let (inner_ty, _, _) = self.parse_declarator(inner_base, diag);
                        self.expect(&TokenKind::RParen, diag);
                        custom_type = Some(inner_ty);
                    }
                }
                TokenKind::Extension => {
                    self.advance();
                }
                TokenKind::Attribute => {
                    self.skip_attribute();
                }
                TokenKind::Asm => {
                    self.skip_asm();
                }
                TokenKind::Void => {
                    self.advance();
                    has_void = true;
                }
                TokenKind::Bool => {
                    self.advance();
                    has_bool = true;
                }
                TokenKind::CharKw => {
                    self.advance();
                    has_char = true;
                }
                TokenKind::Short => {
                    self.advance();
                    has_short = true;
                }
                TokenKind::IntKw => {
                    self.advance();
                    has_int = true;
                }
                TokenKind::Long => {
                    self.advance();
                    long_count += 1;
                }
                TokenKind::Signed => {
                    self.advance();
                    has_signed = true;
                }
                TokenKind::Unsigned => {
                    self.advance();
                    has_unsigned = true;
                }
                TokenKind::FloatKw => {
                    self.advance();
                    has_float = true;
                }
                TokenKind::Double => {
                    self.advance();
                    has_double = true;
                }
                TokenKind::Struct => {
                    if custom_type.is_some()
                        || has_void
                        || has_bool
                        || has_char
                        || has_short
                        || has_int
                        || long_count > 0
                        || has_float
                        || has_double
                    {
                        break;
                    }
                    custom_type = Some(self.parse_struct_or_union_specifier(true, diag));
                }
                TokenKind::Union => {
                    if custom_type.is_some()
                        || has_void
                        || has_bool
                        || has_char
                        || has_short
                        || has_int
                        || long_count > 0
                        || has_float
                        || has_double
                    {
                        break;
                    }
                    custom_type = Some(self.parse_struct_or_union_specifier(false, diag));
                }
                TokenKind::Enum => {
                    if custom_type.is_some()
                        || has_void
                        || has_bool
                        || has_char
                        || has_short
                        || has_int
                        || long_count > 0
                        || has_float
                        || has_double
                    {
                        break;
                    }
                    custom_type = Some(self.parse_enum_specifier(diag));
                }
                TokenKind::Ident(name) => {
                    if custom_type.is_some()
                        || has_void
                        || has_bool
                        || has_char
                        || has_short
                        || has_int
                        || long_count > 0
                        || has_float
                        || has_double
                    {
                        break;
                    }
                    if let Some(ty) = self.find_typedef(name) {
                        self.advance();
                        custom_type = Some(ty);
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }

        let mut ty = if let Some(t) = custom_type {
            t
        } else if has_void {
            Type::void()
        } else if has_bool {
            Type::bool_ty()
        } else if has_float {
            Type::new(TypeKind::Float)
        } else if has_double {
            Type::new(TypeKind::Double)
        } else if has_char {
            if has_unsigned {
                Type::uchar_ty()
            } else {
                Type::char_ty()
            }
        } else if has_short {
            if has_unsigned {
                Type::ushort_ty()
            } else {
                Type::short_ty()
            }
        } else if long_count == 1 {
            if has_unsigned {
                Type::ulong_ty()
            } else {
                Type::long_ty()
            }
        } else if long_count >= 2 {
            if has_unsigned {
                Type::ulonglong_ty()
            } else {
                Type::longlong_ty()
            }
        } else if has_int || has_signed || has_unsigned {
            if has_unsigned {
                Type::uint_ty()
            } else {
                Type::int_ty()
            }
        } else {
            Type::int_ty() // default int
        };

        ty.is_const = is_const;
        ty.is_volatile = is_volatile;

        (ty, is_typedef, is_static, is_extern)
    }

    fn parse_struct_or_union_specifier(&mut self, is_struct: bool, diag: &mut Diagnostics) -> Type {
        self.advance(); // struct or union
        self.skip_extension();
        self.skip_attribute();

        let tag = if let TokenKind::Ident(name) = self.peek_kind() {
            let name_str = name.clone();
            self.advance();
            Some(name_str)
        } else {
            None
        };

        self.skip_extension();
        self.skip_attribute();

        if self.match_token(&TokenKind::LBrace) {
            let mut members = Vec::new();
            let mut current_offset: usize = 0;
            let mut current_storage_offset: usize = 0;
            let mut current_storage_size: usize = 0;
            let mut current_bit_offset: usize = 0;
            let mut max_align: usize = 1;

            while self.peek_kind() != &TokenKind::RBrace && self.peek_kind() != &TokenKind::Eof {
                if diag.messages.len() >= 100 {
                    break;
                }
                let loop_start_cursor = self.cursor;

                self.skip_extension();
                self.skip_attribute();
                if self.match_token(&TokenKind::Semicolon) {
                    continue;
                }
                let (member_base_ty, _, _, _) = self.parse_decl_specifiers(diag);

                loop {
                    let (member_ty, member_name, _) =
                        self.parse_declarator(member_base_ty.clone(), diag);
                    let member_align = member_ty.align();
                    let member_size = member_ty.size();

                    max_align = max_align.max(member_align);

                    let (member_final_ty, bit_width, bit_offset, offset) = if self
                        .match_token(&TokenKind::Colon)
                    {
                        let bit_expr = self.parse_conditional_expr(diag);
                        let bw = self.eval_const_expr(&bit_expr).max(0) as usize;
                        if bw == 0 {
                            // Unnamed 0-width bitfield: align to next boundary
                            current_bit_offset = 0;
                            current_storage_size = 0;
                            let aligned = current_offset.div_ceil(member_align) * member_align;
                            current_offset = aligned;
                            (member_ty.clone(), Some(0), Some(0), aligned)
                        } else if is_struct {
                            let unit_size = member_size.max(current_storage_size);
                            let can_fit = current_storage_size > 0
                                && (current_bit_offset + bw <= unit_size * 8)
                                && current_storage_offset.is_multiple_of(member_align);

                            if can_fit {
                                current_storage_size = unit_size;
                                let off = current_storage_offset;
                                let boff = current_bit_offset;
                                current_bit_offset += bw;
                                current_offset =
                                    current_storage_offset + current_bit_offset.div_ceil(8);
                                let storage_ty = match current_storage_size {
                                    1 => Type::uchar_ty(),
                                    2 => Type::ushort_ty(),
                                    4 => Type::uint_ty(),
                                    _ => Type::ulong_ty(),
                                };
                                (storage_ty, Some(bw), Some(boff), off)
                            } else {
                                let aligned = current_offset.div_ceil(member_align) * member_align;
                                current_storage_offset = aligned;
                                current_storage_size = member_size;
                                let boff = 0;
                                current_bit_offset = bw;
                                current_offset =
                                    current_storage_offset + current_bit_offset.div_ceil(8);
                                let storage_ty = match current_storage_size {
                                    1 => Type::uchar_ty(),
                                    2 => Type::ushort_ty(),
                                    4 => Type::uint_ty(),
                                    _ => Type::ulong_ty(),
                                };
                                (storage_ty, Some(bw), Some(boff), aligned)
                            }
                        } else {
                            // Union bitfield
                            current_offset = current_offset.max(member_size);
                            (member_ty.clone(), Some(bw), Some(0), 0)
                        }
                    } else {
                        current_bit_offset = 0;
                        current_storage_size = 0;
                        let off = if is_struct {
                            let aligned = current_offset.div_ceil(member_align) * member_align;
                            current_offset = aligned + member_size;
                            aligned
                        } else {
                            current_offset = current_offset.max(member_size);
                            0
                        };
                        (member_ty.clone(), None, None, off)
                    };

                    if member_name.is_empty() {
                        if let TypeKind::Struct(sub_st) = &member_final_ty.kind {
                            for sub_m in &sub_st.borrow().members {
                                members.push(StructMember {
                                    name: sub_m.name.clone(),
                                    ty: sub_m.ty.clone(),
                                    offset: offset + sub_m.offset,
                                    bit_width: sub_m.bit_width,
                                    bit_offset: sub_m.bit_offset,
                                });
                            }
                        } else if let TypeKind::Union(sub_ut) = &member_final_ty.kind {
                            for sub_m in &sub_ut.borrow().members {
                                members.push(StructMember {
                                    name: sub_m.name.clone(),
                                    ty: sub_m.ty.clone(),
                                    offset: offset + sub_m.offset,
                                    bit_width: sub_m.bit_width,
                                    bit_offset: sub_m.bit_offset,
                                });
                            }
                        } else {
                            members.push(StructMember {
                                name: member_name,
                                ty: member_final_ty,
                                offset,
                                bit_width,
                                bit_offset,
                            });
                        }
                    } else {
                        members.push(StructMember {
                            name: member_name,
                            ty: member_final_ty,
                            offset,
                            bit_width,
                            bit_offset,
                        });
                    }

                    if self.match_token(&TokenKind::Comma) {
                        continue;
                    } else {
                        if !self.match_token(&TokenKind::Semicolon) {
                            self.expect(&TokenKind::Semicolon, diag);
                        }
                        break;
                    }
                }

                if self.cursor == loop_start_cursor {
                    self.advance();
                }
            }

            self.expect(&TokenKind::RBrace, diag);

            let total_size = current_offset.div_ceil(max_align) * max_align;

            if is_struct {
                let st = if let Some(ref t) = tag {
                    let existing = self
                        .struct_tags
                        .iter()
                        .rev()
                        .find_map(|s| s.get(t))
                        .cloned();
                    if let Some(existing_st) = existing {
                        *existing_st.borrow_mut() = StructType {
                            tag: tag.clone(),
                            members,
                            size: total_size.max(1),
                            align: max_align,
                            is_complete: true,
                        };
                        existing_st
                    } else {
                        let st = Rc::new(RefCell::new(StructType {
                            tag: tag.clone(),
                            members,
                            size: total_size.max(1),
                            align: max_align,
                            is_complete: true,
                        }));
                        if let Some(scope) = self.struct_tags.last_mut() {
                            scope.insert(t.clone(), st.clone());
                        }
                        st
                    }
                } else {
                    Rc::new(RefCell::new(StructType {
                        tag: None,
                        members,
                        size: total_size.max(1),
                        align: max_align,
                        is_complete: true,
                    }))
                };
                Type::new(TypeKind::Struct(st))
            } else {
                let ut = if let Some(ref t) = tag {
                    let existing = self.union_tags.iter().rev().find_map(|s| s.get(t)).cloned();
                    if let Some(existing_ut) = existing {
                        *existing_ut.borrow_mut() = UnionType {
                            tag: tag.clone(),
                            members,
                            size: total_size.max(1),
                            align: max_align,
                            is_complete: true,
                        };
                        existing_ut
                    } else {
                        let ut = Rc::new(RefCell::new(UnionType {
                            tag: tag.clone(),
                            members,
                            size: total_size.max(1),
                            align: max_align,
                            is_complete: true,
                        }));
                        if let Some(scope) = self.union_tags.last_mut() {
                            scope.insert(t.clone(), ut.clone());
                        }
                        ut
                    }
                } else {
                    Rc::new(RefCell::new(UnionType {
                        tag: None,
                        members,
                        size: total_size.max(1),
                        align: max_align,
                        is_complete: true,
                    }))
                };
                Type::new(TypeKind::Union(ut))
            }
        } else if let Some(t) = tag {
            if is_struct {
                for scope in self.struct_tags.iter().rev() {
                    if let Some(st) = scope.get(&t) {
                        return Type::new(TypeKind::Struct(st.clone()));
                    }
                }
                let st = Rc::new(RefCell::new(StructType {
                    tag: Some(t.clone()),
                    members: Vec::new(),
                    size: 0,
                    align: 1,
                    is_complete: false,
                }));
                if let Some(scope) = self.struct_tags.last_mut() {
                    scope.insert(t, st.clone());
                }
                Type::new(TypeKind::Struct(st))
            } else {
                for scope in self.union_tags.iter().rev() {
                    if let Some(ut) = scope.get(&t) {
                        return Type::new(TypeKind::Union(ut.clone()));
                    }
                }
                let ut = Rc::new(RefCell::new(UnionType {
                    tag: Some(t.clone()),
                    members: Vec::new(),
                    size: 0,
                    align: 1,
                    is_complete: false,
                }));
                if let Some(scope) = self.union_tags.last_mut() {
                    scope.insert(t, ut.clone());
                }
                Type::new(TypeKind::Union(ut))
            }
        } else {
            diag.error(self.peek().span, "Unnamed struct/union must have a body");
            Type::int_ty()
        }
    }

    fn parse_enum_specifier(&mut self, diag: &mut Diagnostics) -> Type {
        self.advance(); // enum

        let tag = if let TokenKind::Ident(name) = self.peek_kind() {
            let name_str = name.clone();
            self.advance();
            Some(name_str)
        } else {
            None
        };

        if self.match_token(&TokenKind::LBrace) {
            let mut variants = Vec::new();
            let mut next_val = 0i64;

            while self.peek_kind() != &TokenKind::RBrace && self.peek_kind() != &TokenKind::Eof {
                if let TokenKind::Ident(vname) = self.peek_kind() {
                    let vname_str = vname.clone();
                    self.advance();

                    let val = if self.match_token(&TokenKind::Assign) {
                        let expr = self.parse_conditional_expr(diag);
                        let v = self.eval_const_expr(&expr);
                        next_val = v + 1;
                        v
                    } else {
                        let v = next_val;
                        next_val += 1;
                        v
                    };

                    variants.push(EnumVariant {
                        name: vname_str.clone(),
                        value: val,
                    });

                    if let Some(scope) = self.enum_constants.last_mut() {
                        scope.insert(vname_str, val);
                    }

                    if self.match_token(&TokenKind::Comma) {
                        continue;
                    } else {
                        break;
                    }
                } else {
                    diag.error(self.peek().span, "Expected identifier in enum declaration");
                    break;
                }
            }

            self.expect(&TokenKind::RBrace, diag);

            let et = Rc::new(EnumType {
                tag: tag.clone(),
                variants,
            });

            if let (Some(t), Some(scope)) = (tag, self.enum_tags.last_mut()) {
                scope.insert(t, et.clone());
            }

            Type::new(TypeKind::Enum(et))
        } else if let Some(t) = tag {
            for scope in self.enum_tags.iter().rev() {
                if let Some(et) = scope.get(&t) {
                    return Type::new(TypeKind::Enum(et.clone()));
                }
            }
            Type::int_ty()
        } else {
            diag.error(self.peek().span, "Unnamed enum must have a body");
            Type::int_ty()
        }
    }

    fn eval_const_expr(&self, expr: &Expr) -> i64 {
        match expr {
            Expr::Int(v, _) => *v,
            Expr::Char(c, _) => *c as i64,
            Expr::Binary { op, lhs, rhs, .. } => {
                let l = self.eval_const_expr(lhs);
                let r = self.eval_const_expr(rhs);
                match op {
                    BinaryOp::Add => l.wrapping_add(r),
                    BinaryOp::Sub => l.wrapping_sub(r),
                    BinaryOp::Mul => l.wrapping_mul(r),
                    BinaryOp::Div => {
                        if r != 0 {
                            l / r
                        } else {
                            0
                        }
                    }
                    BinaryOp::Rem => {
                        if r != 0 {
                            l % r
                        } else {
                            0
                        }
                    }
                    BinaryOp::BitAnd => l & r,
                    BinaryOp::BitOr => l | r,
                    BinaryOp::BitXor => l ^ r,
                    BinaryOp::Shl => l << (r as u32),
                    BinaryOp::Shr => l >> (r as u32),
                    _ => 0,
                }
            }
            Expr::Unary { op, expr, .. } => {
                let v = self.eval_const_expr(expr);
                match op {
                    UnaryOp::Pos => v,
                    UnaryOp::Neg => -v,
                    UnaryOp::BitNot => !v,
                    UnaryOp::LogNot => i64::from(v == 0),
                    _ => 0,
                }
            }
            Expr::SizeofType { target_type, .. } => target_type.size() as i64,
            Expr::Var(name, _) => self.find_enum_constant(name).unwrap_or(0),
            Expr::Cast { expr, .. } => self.eval_const_expr(expr),
            Expr::Ternary {
                cond,
                then_expr,
                else_expr,
                ..
            } => {
                if self.eval_const_expr(cond) != 0 {
                    self.eval_const_expr(then_expr)
                } else {
                    self.eval_const_expr(else_expr)
                }
            }
            _ => 0,
        }
    }

    fn is_nested_declarator_start(&self) -> bool {
        if self.peek_kind() != &TokenKind::LParen {
            return false;
        }
        let next = self.peek_next();
        match &next.kind {
            TokenKind::Star | TokenKind::LParen => true,
            TokenKind::Ident(name) => !self.is_typedef_name(name),
            _ => false,
        }
    }

    fn parse_declarator(
        &mut self,
        mut base_ty: Type,
        diag: &mut Diagnostics,
    ) -> (Type, String, Vec<String>) {
        self.skip_extension();
        self.skip_attribute();

        // Pointers
        while self.match_token(&TokenKind::Star) {
            while self.peek_kind() == &TokenKind::Const
                || self.peek_kind() == &TokenKind::Volatile
                || self.peek_kind() == &TokenKind::Restrict
                || self.peek_kind() == &TokenKind::Atomic
            {
                self.advance();
            }
            self.skip_attribute();
            base_ty = base_ty.pointer_to();
        }

        let mut nested_span = None;
        let mut name = String::new();

        if self.is_nested_declarator_start() {
            self.advance(); // consume '('
            let inner_start = self.cursor;
            let mut depth = 1;
            while self.cursor < self.tokens.len() && depth > 0 {
                if self.peek_kind() == &TokenKind::LParen {
                    depth += 1;
                } else if self.peek_kind() == &TokenKind::RParen {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                self.cursor += 1;
            }
            let inner_end = self.cursor;
            if self.peek_kind() == &TokenKind::RParen {
                self.advance(); // consume matching ')'
            }
            nested_span = Some((inner_start, inner_end));
        } else if let TokenKind::Ident(s) = self.peek_kind() {
            name = s.clone();
            self.advance();
        }

        self.skip_attribute();
        self.skip_asm();

        // Postfix declarator modifiers: array brackets or function parameters
        let mut param_names = Vec::new();
        if self.match_token(&TokenKind::LParen) {
            let mut params = Vec::new();
            let mut is_variadic = false;

            if self.peek_kind() == &TokenKind::Void && self.peek_next().kind == TokenKind::RParen {
                self.advance(); // void
            } else {
                while self.peek_kind() != &TokenKind::RParen && self.peek_kind() != &TokenKind::Eof
                {
                    if self.match_token(&TokenKind::Ellipsis) {
                        is_variadic = true;
                        break;
                    }

                    let (param_base, _, _, _) = self.parse_decl_specifiers(diag);
                    let (param_ty, pname, _) = self.parse_declarator(param_base, diag);

                    // Array decay to pointer in parameter lists
                    let final_param_ty = if let TypeKind::Array { elem, .. } = param_ty.kind {
                        elem.pointer_to()
                    } else {
                        param_ty
                    };

                    params.push(final_param_ty);
                    param_names.push(pname);

                    if self.match_token(&TokenKind::Comma) {
                        continue;
                    } else {
                        break;
                    }
                }
            }

            self.expect(&TokenKind::RParen, diag);

            base_ty = Type::new(TypeKind::Function {
                ret: Box::new(base_ty),
                params,
                is_variadic,
            });
        } else {
            // Array dimensions
            let mut dimensions = Vec::new();
            while self.match_token(&TokenKind::LBracket) {
                while self.peek_kind() == &TokenKind::Static
                    || self.peek_kind() == &TokenKind::Const
                    || self.peek_kind() == &TokenKind::Volatile
                    || self.peek_kind() == &TokenKind::Restrict
                {
                    self.advance();
                }
                if self.match_token(&TokenKind::RBracket) {
                    dimensions.push(None);
                } else if self.peek_kind() == &TokenKind::Star
                    && self.peek_next().kind == TokenKind::RBracket
                {
                    self.advance(); // *
                    self.advance(); // ]
                    dimensions.push(None);
                } else {
                    let len_expr = self.parse_expr(diag);
                    let len = self.eval_const_expr(&len_expr) as usize;
                    self.expect(&TokenKind::RBracket, diag);
                    dimensions.push(Some(len));
                }
            }

            for dim in dimensions.into_iter().rev() {
                base_ty = base_ty.array_of(dim);
            }
        }

        self.skip_attribute();
        self.skip_asm();

        if let Some((inner_start, _inner_end)) = nested_span {
            let saved_cursor = self.cursor;
            self.cursor = inner_start;
            let (inner_ty, inner_name, inner_param_names) = self.parse_declarator(base_ty, diag);
            self.cursor = saved_cursor;
            (
                inner_ty,
                inner_name,
                if inner_param_names.is_empty() {
                    param_names
                } else {
                    inner_param_names
                },
            )
        } else {
            (base_ty, name, param_names)
        }
    }

    fn parse_initializer(&mut self, diag: &mut Diagnostics) -> Initializer {
        if self.match_token(&TokenKind::LBrace) {
            let mut list = Vec::new();
            while self.peek_kind() != &TokenKind::RBrace && self.peek_kind() != &TokenKind::Eof {
                let mut designators = Vec::new();
                while self.peek_kind() == &TokenKind::Dot
                    || self.peek_kind() == &TokenKind::LBracket
                {
                    if self.match_token(&TokenKind::Dot) {
                        if let TokenKind::Ident(name) = self.peek_kind() {
                            let n = name.clone();
                            self.advance();
                            designators.push(Designator::Field(n));
                        } else {
                            diag.error(
                                self.peek().span,
                                "Expected identifier after '.' in designated initializer",
                            );
                        }
                    } else if self.match_token(&TokenKind::LBracket) {
                        let idx_expr = self.parse_conditional_expr(diag);
                        let start_idx = self.eval_const_expr(&idx_expr);
                        if self.match_token(&TokenKind::Ellipsis) {
                            let end_expr = self.parse_conditional_expr(diag);
                            let end_idx = self.eval_const_expr(&end_expr);
                            self.expect(&TokenKind::RBracket, diag);
                            for idx in start_idx..=end_idx {
                                designators.push(Designator::Index(idx));
                            }
                        } else {
                            self.expect(&TokenKind::RBracket, diag);
                            designators.push(Designator::Index(start_idx));
                        }
                    }
                }
                if !designators.is_empty() {
                    self.match_token(&TokenKind::Assign);
                }
                let init = self.parse_initializer(diag);
                if designators.len() > 1
                    && designators
                        .iter()
                        .all(|d| matches!(d, Designator::Index(_)))
                {
                    for d in designators {
                        list.push(InitItem {
                            designators: vec![d],
                            init: init.clone(),
                        });
                    }
                } else {
                    list.push(InitItem { designators, init });
                }
                if self.match_token(&TokenKind::Comma) {
                    if self.peek_kind() == &TokenKind::RBrace {
                        break;
                    }
                    continue;
                } else {
                    break;
                }
            }
            self.expect(&TokenKind::RBrace, diag);
            Initializer::List(list)
        } else {
            Initializer::Single(Box::new(self.parse_assignment_expr(diag)))
        }
    }

    pub fn parse_block(&mut self, diag: &mut Diagnostics) -> Vec<Stmt> {
        self.expect(&TokenKind::LBrace, diag);
        let mut stmts = Vec::new();

        while self.peek_kind() != &TokenKind::RBrace && self.peek_kind() != &TokenKind::Eof {
            stmts.push(self.parse_stmt(diag));
        }

        self.expect(&TokenKind::RBrace, diag);
        stmts
    }

    pub fn parse_stmt(&mut self, diag: &mut Diagnostics) -> Stmt {
        let span = self.peek().span;

        match self.peek_kind() {
            TokenKind::Return => {
                self.advance();
                let expr = if self.peek_kind() == &TokenKind::Semicolon {
                    None
                } else {
                    Some(self.parse_expr(diag))
                };
                self.expect(&TokenKind::Semicolon, diag);
                Stmt::Return(expr, span.merge(self.peek().span))
            }
            TokenKind::If => {
                self.advance();
                self.expect(&TokenKind::LParen, diag);
                let cond = self.parse_expr(diag);
                self.expect(&TokenKind::RParen, diag);
                let then_stmt = Box::new(self.parse_stmt(diag));
                let else_stmt = if self.match_token(&TokenKind::Else) {
                    Some(Box::new(self.parse_stmt(diag)))
                } else {
                    None
                };
                Stmt::If {
                    cond,
                    then_stmt,
                    else_stmt,
                    span: span.merge(self.peek().span),
                }
            }
            TokenKind::While => {
                self.advance();
                self.expect(&TokenKind::LParen, diag);
                let cond = self.parse_expr(diag);
                self.expect(&TokenKind::RParen, diag);
                let body = Box::new(self.parse_stmt(diag));
                Stmt::While {
                    cond,
                    body,
                    span: span.merge(self.peek().span),
                }
            }
            TokenKind::Do => {
                self.advance();
                let body = Box::new(self.parse_stmt(diag));
                self.expect(&TokenKind::While, diag);
                self.expect(&TokenKind::LParen, diag);
                let cond = self.parse_expr(diag);
                self.expect(&TokenKind::RParen, diag);
                self.expect(&TokenKind::Semicolon, diag);
                Stmt::DoWhile {
                    body,
                    cond,
                    span: span.merge(self.peek().span),
                }
            }
            TokenKind::For => {
                self.advance();
                self.expect(&TokenKind::LParen, diag);
                self.enter_scope();

                let init = if self.match_token(&TokenKind::Semicolon) {
                    None
                } else if self.is_type_specifier_start() {
                    let decls = self.parse_var_decl_list(diag);
                    Some(ForInit::Decl(decls))
                } else {
                    let expr = self.parse_expr(diag);
                    self.expect(&TokenKind::Semicolon, diag);
                    Some(ForInit::Expr(expr))
                };

                let cond = if self.peek_kind() == &TokenKind::Semicolon {
                    None
                } else {
                    Some(self.parse_expr(diag))
                };
                self.expect(&TokenKind::Semicolon, diag);

                let step = if self.peek_kind() == &TokenKind::RParen {
                    None
                } else {
                    Some(self.parse_expr(diag))
                };
                self.expect(&TokenKind::RParen, diag);

                let body = Box::new(self.parse_stmt(diag));
                self.exit_scope();

                Stmt::For {
                    init,
                    cond,
                    step,
                    body,
                    span: span.merge(self.peek().span),
                }
            }
            TokenKind::Switch => {
                self.advance();
                self.expect(&TokenKind::LParen, diag);
                let expr = self.parse_expr(diag);
                self.expect(&TokenKind::RParen, diag);
                let body = Box::new(self.parse_stmt(diag));
                Stmt::Switch {
                    expr,
                    body,
                    span: span.merge(self.peek().span),
                }
            }
            TokenKind::Case => {
                self.advance();
                let val_expr = self.parse_expr(diag);
                let val = self.eval_const_expr(&val_expr);
                self.expect(&TokenKind::Colon, diag);
                let body = Box::new(self.parse_stmt(diag));
                Stmt::Case {
                    val,
                    body,
                    span: span.merge(self.peek().span),
                }
            }
            TokenKind::Default => {
                self.advance();
                self.expect(&TokenKind::Colon, diag);
                let body = Box::new(self.parse_stmt(diag));
                Stmt::Default {
                    body,
                    span: span.merge(self.peek().span),
                }
            }
            TokenKind::Break => {
                self.advance();
                self.expect(&TokenKind::Semicolon, diag);
                Stmt::Break(span)
            }
            TokenKind::Continue => {
                self.advance();
                self.expect(&TokenKind::Semicolon, diag);
                Stmt::Continue(span)
            }
            TokenKind::Goto => {
                self.advance();
                if self.match_token(&TokenKind::Star) {
                    let expr = self.parse_expr(diag);
                    self.expect(&TokenKind::Semicolon, diag);
                    Stmt::GotoExpr(expr, span.merge(self.peek().span))
                } else {
                    let label = if let TokenKind::Ident(lbl) = self.peek_kind() {
                        let l = lbl.clone();
                        self.advance();
                        l
                    } else {
                        diag.error(self.peek().span, "Expected label after goto");
                        String::new()
                    };
                    self.expect(&TokenKind::Semicolon, diag);
                    Stmt::Goto(label, span.merge(self.peek().span))
                }
            }
            TokenKind::LBrace => {
                self.enter_scope();
                let block = self.parse_block(diag);
                self.exit_scope();
                Stmt::Block(block, span)
            }
            TokenKind::Semicolon => {
                self.advance();
                Stmt::Empty(span)
            }
            TokenKind::Asm => {
                self.skip_asm();
                self.match_token(&TokenKind::Semicolon);
                Stmt::Empty(span)
            }
            _ => {
                // Check if it is a label: `ident:`
                if let TokenKind::Ident(name) = self.peek_kind()
                    && self.peek_next().kind == TokenKind::Colon
                    && !self.is_typedef_name(name)
                {
                    let name_str = name.clone();
                    self.advance(); // ident
                    self.advance(); // colon
                    let inner = Box::new(self.parse_stmt(diag));
                    return Stmt::Label(name_str, inner, span);
                }

                // Check if it is a declaration
                if self.is_type_specifier_start() {
                    let decls = self.parse_var_decl_list(diag);
                    return Stmt::Decl(decls);
                }

                // Expression statement
                let expr = self.parse_expr(diag);
                self.expect(&TokenKind::Semicolon, diag);
                Stmt::Expr(expr)
            }
        }
    }

    fn parse_var_decl_list(&mut self, diag: &mut Diagnostics) -> Vec<VarDecl> {
        let (base_ty, is_typedef, is_static, is_extern) = self.parse_decl_specifiers(diag);
        let mut decls = Vec::new();

        loop {
            let start = self.peek().span;
            let (ty, name, _) = self.parse_declarator(base_ty.clone(), diag);

            if is_typedef {
                self.add_typedef(name, ty);
            } else {
                let init = if self.match_token(&TokenKind::Assign) {
                    Some(self.parse_initializer(diag))
                } else {
                    None
                };

                decls.push(VarDecl {
                    name,
                    ty,
                    init,
                    is_static,
                    is_extern,
                    span: start.merge(self.peek().span),
                });
            }

            if self.match_token(&TokenKind::Comma) {
                continue;
            } else {
                self.expect(&TokenKind::Semicolon, diag);
                break;
            }
        }

        decls
    }

    pub fn parse_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        self.parse_comma_expr(diag)
    }

    fn parse_comma_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let mut expr = self.parse_assignment_expr(diag);

        while self.match_token(&TokenKind::Comma) {
            let rhs = self.parse_assignment_expr(diag);
            let span = expr.span().merge(rhs.span());
            expr = Expr::Binary {
                op: BinaryOp::Comma,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
                span,
            };
        }

        expr
    }

    fn parse_assignment_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let lhs = self.parse_conditional_expr(diag);

        let op = match self.peek_kind() {
            TokenKind::Assign => Some(BinaryOp::Assign),
            TokenKind::PlusAssign => Some(BinaryOp::PlusAssign),
            TokenKind::MinusAssign => Some(BinaryOp::MinusAssign),
            TokenKind::StarAssign => Some(BinaryOp::StarAssign),
            TokenKind::SlashAssign => Some(BinaryOp::SlashAssign),
            TokenKind::PercentAssign => Some(BinaryOp::PercentAssign),
            TokenKind::AmpAssign => Some(BinaryOp::AmpAssign),
            TokenKind::PipeAssign => Some(BinaryOp::PipeAssign),
            TokenKind::CaretAssign => Some(BinaryOp::CaretAssign),
            TokenKind::ShlAssign => Some(BinaryOp::ShlAssign),
            TokenKind::ShrAssign => Some(BinaryOp::ShrAssign),
            _ => None,
        };

        if let Some(binary_op) = op {
            self.advance();
            let rhs = self.parse_assignment_expr(diag);
            let span = lhs.span().merge(rhs.span());
            Expr::Binary {
                op: binary_op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            }
        } else {
            lhs
        }
    }

    fn parse_conditional_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let cond = self.parse_logical_or_expr(diag);

        if self.match_token(&TokenKind::Question) {
            let then_expr = self.parse_expr(diag);
            self.expect(&TokenKind::Colon, diag);
            let else_expr = self.parse_conditional_expr(diag);
            let span = cond.span().merge(else_expr.span());
            Expr::Ternary {
                cond: Box::new(cond),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
                span,
            }
        } else {
            cond
        }
    }

    fn parse_logical_or_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let mut lhs = self.parse_logical_and_expr(diag);

        while self.match_token(&TokenKind::PipePipe) {
            let rhs = self.parse_logical_and_expr(diag);
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op: BinaryOp::LogicalOr,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }

        lhs
    }

    fn parse_logical_and_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let mut lhs = self.parse_bitwise_or_expr(diag);

        while self.match_token(&TokenKind::AmpAmp) {
            let rhs = self.parse_bitwise_or_expr(diag);
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op: BinaryOp::LogicalAnd,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }

        lhs
    }

    fn parse_bitwise_or_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let mut lhs = self.parse_bitwise_xor_expr(diag);

        while self.match_token(&TokenKind::Pipe) {
            let rhs = self.parse_bitwise_xor_expr(diag);
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op: BinaryOp::BitOr,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }

        lhs
    }

    fn parse_bitwise_xor_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let mut lhs = self.parse_bitwise_and_expr(diag);

        while self.match_token(&TokenKind::Caret) {
            let rhs = self.parse_bitwise_and_expr(diag);
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op: BinaryOp::BitXor,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }

        lhs
    }

    fn parse_bitwise_and_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let mut lhs = self.parse_equality_expr(diag);

        while self.match_token(&TokenKind::Amp) {
            let rhs = self.parse_equality_expr(diag);
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op: BinaryOp::BitAnd,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }

        lhs
    }

    fn parse_equality_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let mut lhs = self.parse_relational_expr(diag);

        loop {
            let op = if self.match_token(&TokenKind::Eq) {
                BinaryOp::Eq
            } else if self.match_token(&TokenKind::Ne) {
                BinaryOp::Ne
            } else {
                break;
            };

            let rhs = self.parse_relational_expr(diag);
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }

        lhs
    }

    fn parse_relational_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let mut lhs = self.parse_shift_expr(diag);

        loop {
            let op = if self.match_token(&TokenKind::Lt) {
                BinaryOp::Lt
            } else if self.match_token(&TokenKind::Le) {
                BinaryOp::Le
            } else if self.match_token(&TokenKind::Gt) {
                BinaryOp::Gt
            } else if self.match_token(&TokenKind::Ge) {
                BinaryOp::Ge
            } else {
                break;
            };

            let rhs = self.parse_shift_expr(diag);
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }

        lhs
    }

    fn parse_shift_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let mut lhs = self.parse_additive_expr(diag);

        loop {
            let op = if self.match_token(&TokenKind::Shl) {
                BinaryOp::Shl
            } else if self.match_token(&TokenKind::Shr) {
                BinaryOp::Shr
            } else {
                break;
            };

            let rhs = self.parse_additive_expr(diag);
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }

        lhs
    }

    fn parse_additive_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let mut lhs = self.parse_multiplicative_expr(diag);

        loop {
            let op = if self.match_token(&TokenKind::Plus) {
                BinaryOp::Add
            } else if self.match_token(&TokenKind::Minus) {
                BinaryOp::Sub
            } else {
                break;
            };

            let rhs = self.parse_multiplicative_expr(diag);
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }

        lhs
    }

    fn parse_multiplicative_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let mut lhs = self.parse_cast_expr(diag);

        loop {
            let op = if self.match_token(&TokenKind::Star) {
                BinaryOp::Mul
            } else if self.match_token(&TokenKind::Slash) {
                BinaryOp::Div
            } else if self.match_token(&TokenKind::Percent) {
                BinaryOp::Rem
            } else {
                break;
            };

            let rhs = self.parse_cast_expr(diag);
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }

        lhs
    }

    fn parse_cast_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        if self.peek_kind() == &TokenKind::LParen {
            // Check if this LParen is a cast `(type)` or compound literal `(type){...}` or expression `(expr)`
            let saved_cursor = self.cursor;
            self.advance(); // '('
            if self.is_type_specifier_start() {
                let (base_ty, _, _, _) = self.parse_decl_specifiers(diag);
                let (ty, _, _) = self.parse_declarator(base_ty, diag);
                if self.peek_kind() == &TokenKind::RParen {
                    self.advance(); // ')'
                    if self.peek_kind() == &TokenKind::LBrace {
                        // Compound literal! `(Type){ initializer }`
                        let init = self.parse_initializer(diag);
                        let span = self.peek().span;
                        let expr = Expr::CompoundLiteral {
                            target_type: ty,
                            init,
                            span,
                        };
                        return self.parse_postfix_expr_tail(expr, diag);
                    }
                    let expr = self.parse_cast_expr(diag);
                    return Expr::Cast {
                        target_type: ty,
                        span: expr.span(),
                        expr: Box::new(expr),
                    };
                }
            }
            // Not a cast, backtrack
            self.cursor = saved_cursor;
        }

        self.parse_unary_expr(diag)
    }

    fn parse_unary_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let span = self.peek().span;

        match self.peek_kind() {
            TokenKind::Plus => {
                self.advance();
                let expr = self.parse_cast_expr(diag);
                Expr::Unary {
                    op: UnaryOp::Pos,
                    span: span.merge(expr.span()),
                    expr: Box::new(expr),
                }
            }
            TokenKind::Minus => {
                self.advance();
                let expr = self.parse_cast_expr(diag);
                Expr::Unary {
                    op: UnaryOp::Neg,
                    span: span.merge(expr.span()),
                    expr: Box::new(expr),
                }
            }
            TokenKind::Tilde => {
                self.advance();
                let expr = self.parse_cast_expr(diag);
                Expr::Unary {
                    op: UnaryOp::BitNot,
                    span: span.merge(expr.span()),
                    expr: Box::new(expr),
                }
            }
            TokenKind::Exclaim => {
                self.advance();
                let expr = self.parse_cast_expr(diag);
                Expr::Unary {
                    op: UnaryOp::LogNot,
                    span: span.merge(expr.span()),
                    expr: Box::new(expr),
                }
            }
            TokenKind::Star => {
                self.advance();
                let expr = self.parse_cast_expr(diag);
                Expr::Unary {
                    op: UnaryOp::Deref,
                    span: span.merge(expr.span()),
                    expr: Box::new(expr),
                }
            }
            TokenKind::Amp => {
                self.advance();
                let expr = self.parse_cast_expr(diag);
                Expr::Unary {
                    op: UnaryOp::AddrOf,
                    span: span.merge(expr.span()),
                    expr: Box::new(expr),
                }
            }
            TokenKind::AmpAmp => {
                self.advance();
                if let TokenKind::Ident(label) = self.peek_kind() {
                    let l = label.clone();
                    self.advance();
                    Expr::Unary {
                        op: UnaryOp::AddrOfLabel,
                        expr: Box::new(Expr::Var(l, span.merge(self.peek().span))),
                        span: span.merge(self.peek().span),
                    }
                } else {
                    diag.error(span, "Expected label name after '&&'");
                    Expr::Int(0, span)
                }
            }
            TokenKind::PlusPlus => {
                self.advance();
                let expr = self.parse_unary_expr(diag);
                Expr::Unary {
                    op: UnaryOp::PreInc,
                    span: span.merge(expr.span()),
                    expr: Box::new(expr),
                }
            }
            TokenKind::MinusMinus => {
                self.advance();
                let expr = self.parse_unary_expr(diag);
                Expr::Unary {
                    op: UnaryOp::PreDec,
                    span: span.merge(expr.span()),
                    expr: Box::new(expr),
                }
            }
            TokenKind::Sizeof => {
                self.advance();
                if self.peek_kind() == &TokenKind::LParen {
                    let saved_cursor = self.cursor;
                    self.advance(); // '('
                    if self.is_type_specifier_start() {
                        let (base_ty, _, _, _) = self.parse_decl_specifiers(diag);
                        let (ty, _, _) = self.parse_declarator(base_ty, diag);
                        if self.peek_kind() == &TokenKind::RParen {
                            self.advance();
                            return Expr::SizeofType {
                                target_type: ty,
                                span: span.merge(self.peek().span),
                            };
                        }
                    }
                    self.cursor = saved_cursor;
                }
                let expr = self.parse_unary_expr(diag);
                Expr::Unary {
                    op: UnaryOp::Sizeof,
                    span: span.merge(expr.span()),
                    expr: Box::new(expr),
                }
            }
            _ => self.parse_postfix_expr(diag),
        }
    }

    fn parse_postfix_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let expr = self.parse_primary_expr(diag);
        self.parse_postfix_expr_tail(expr, diag)
    }

    fn parse_postfix_expr_tail(&mut self, mut expr: Expr, diag: &mut Diagnostics) -> Expr {
        loop {
            if self.match_token(&TokenKind::LParen) {
                // Function call
                let mut args = Vec::new();
                if self.peek_kind() != &TokenKind::RParen {
                    loop {
                        args.push(self.parse_assignment_expr(diag));
                        if self.match_token(&TokenKind::Comma) {
                            continue;
                        } else {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RParen, diag);
                let span = expr.span().merge(self.peek().span);
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                    span,
                };
            } else if self.match_token(&TokenKind::LBracket) {
                // Array indexing `expr[index]`
                let index = self.parse_expr(diag);
                self.expect(&TokenKind::RBracket, diag);
                let span = expr.span().merge(self.peek().span);
                expr = Expr::Index {
                    expr: Box::new(expr),
                    index: Box::new(index),
                    span,
                };
            } else if self.match_token(&TokenKind::Dot) {
                // Member access `.`
                if let TokenKind::Ident(m) = self.peek_kind() {
                    let member = m.clone();
                    self.advance();
                    let span = expr.span().merge(self.peek().span);
                    expr = Expr::Member {
                        expr: Box::new(expr),
                        member,
                        is_arrow: false,
                        span,
                    };
                } else {
                    diag.error(self.peek().span, "Expected member name after '.'");
                    break;
                }
            } else if self.match_token(&TokenKind::Arrow) {
                // Member access `->`
                if let TokenKind::Ident(m) = self.peek_kind() {
                    let member = m.clone();
                    self.advance();
                    let span = expr.span().merge(self.peek().span);
                    expr = Expr::Member {
                        expr: Box::new(expr),
                        member,
                        is_arrow: true,
                        span,
                    };
                } else {
                    diag.error(self.peek().span, "Expected member name after '->'");
                    break;
                }
            } else if self.match_token(&TokenKind::PlusPlus) {
                let span = expr.span().merge(self.peek().span);
                expr = Expr::Unary {
                    op: UnaryOp::PostInc,
                    expr: Box::new(expr),
                    span,
                };
            } else if self.match_token(&TokenKind::MinusMinus) {
                let span = expr.span().merge(self.peek().span);
                expr = Expr::Unary {
                    op: UnaryOp::PostDec,
                    expr: Box::new(expr),
                    span,
                };
            } else {
                break;
            }
        }

        expr
    }

    fn parse_primary_expr(&mut self, diag: &mut Diagnostics) -> Expr {
        let tok = self.peek().clone();

        match &tok.kind {
            TokenKind::Int(v) => {
                self.advance();
                Expr::Int(*v, tok.span)
            }
            TokenKind::Float(v) => {
                self.advance();
                Expr::Float(*v, tok.span)
            }
            TokenKind::Char(c) => {
                self.advance();
                Expr::Char(*c, tok.span)
            }
            TokenKind::String(s) => {
                let mut full_str = s.clone();
                self.advance();
                while let TokenKind::String(next_s) = self.peek_kind() {
                    full_str.push_str(next_s);
                    self.advance();
                }
                Expr::String(full_str, tok.span.merge(self.peek().span))
            }
            TokenKind::Ident(name) => {
                let name_str = name.clone();
                self.advance();
                if let Some(val) = self.find_enum_constant(&name_str) {
                    Expr::Int(val, tok.span)
                } else {
                    Expr::Var(name_str, tok.span)
                }
            }
            TokenKind::LParen => {
                self.advance();
                // Check GNU statement expression `({ ... })`
                if self.peek_kind() == &TokenKind::LBrace {
                    self.enter_scope();
                    let block = self.parse_block(diag);
                    self.exit_scope();
                    self.expect(&TokenKind::RParen, diag);
                    Expr::StmtExpr {
                        body: block,
                        span: tok.span.merge(self.peek().span),
                    }
                } else {
                    let expr = self.parse_expr(diag);
                    self.expect(&TokenKind::RParen, diag);
                    expr
                }
            }
            _ => {
                diag.error(
                    tok.span,
                    format!("Unexpected token in expression: '{}'", tok.kind),
                );
                self.advance();
                Expr::Int(0, tok.span)
            }
        }
    }
}
