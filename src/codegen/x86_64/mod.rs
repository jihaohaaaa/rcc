use crate::ast::*;
use crate::codegen::target::TargetEmitter;
use crate::sema::*;
use crate::types::*;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

pub struct X86_64Emitter {
    is_macos: bool,
    output: String,
    label_counter: usize,
    current_func: String,
    current_func_scratch_offset: usize,
    current_func_va_offset: usize,
    current_func_named_count: usize,
    loop_labels: Vec<(String, String)>, // (continue_label, break_label)
    depth: Cell<usize>,
    defined_globals: HashSet<String>,
}

impl Default for X86_64Emitter {
    fn default() -> Self {
        Self::new()
    }
}

impl X86_64Emitter {
    pub fn new() -> Self {
        let is_macos = cfg!(target_os = "macos");
        Self {
            is_macos,
            output: String::new(),
            label_counter: 0,
            current_func: String::new(),
            current_func_scratch_offset: 0,
            current_func_va_offset: 0,
            current_func_named_count: 0,
            loop_labels: Vec::new(),
            depth: Cell::new(0),
            defined_globals: HashSet::new(),
        }
    }

    pub fn with_macos(mut self, is_macos: bool) -> Self {
        self.is_macos = is_macos;
        self
    }

    fn new_label(&mut self, prefix: &str) -> String {
        self.label_counter += 1;
        format!(".L.{}.{}", prefix, self.label_counter)
    }

    fn symbol_name(&self, name: &str) -> String {
        if name.starts_with('.') {
            name.to_string()
        } else if self.is_macos {
            format!("_{}", name)
        } else {
            name.to_string()
        }
    }

    fn emit(&mut self, line: &str) {
        writeln!(self.output, "    {}", line).unwrap();
    }

    fn emit_label(&mut self, label: &str) {
        writeln!(self.output, "{}:", label).unwrap();
    }

    fn push(&mut self) {
        self.emit("push rax");
    }

    fn pop(&mut self, reg: &str) {
        self.emit(&format!("pop {}", reg));
    }

    fn emit_load_imm(&mut self, reg: &str, val: i64) {
        if val == 0 {
            if reg == "rax" {
                self.emit("xor eax, eax");
            } else if reg == "rcx" {
                self.emit("xor ecx, ecx");
            } else if reg == "rdx" {
                self.emit("xor edx, edx");
            } else if reg == "rsi" {
                self.emit("xor esi, esi");
            } else if reg == "rdi" {
                self.emit("xor edi, edi");
            } else if reg == "r8" {
                self.emit("xor r8d, r8d");
            } else if reg == "r9" {
                self.emit("xor r9d, r9d");
            } else if reg == "r10" {
                self.emit("xor r10d, r10d");
            } else if reg == "r11" {
                self.emit("xor r11d, r11d");
            } else {
                self.emit(&format!("mov {}, 0", reg));
            }
        } else if val >= i32::MIN as i64 && val <= i32::MAX as i64 {
            self.emit(&format!("mov {}, {}", reg, val));
        } else {
            self.emit(&format!("movabs {}, {}", reg, val));
        }
    }

    fn gen_lval(&mut self, expr: &TypedExpr) {
        let d = self.depth.get() + 1;
        self.depth.set(d);
        if d > 200 {
            panic!(
                "Infinite recursion in gen_lval! func: {}, expr: {:?}",
                self.current_func, expr
            );
        }
        self.gen_lval_inner(expr);
        self.depth.set(d - 1);
    }

    fn gen_lval_inner(&mut self, expr: &TypedExpr) {
        match &expr.kind {
            TypedExprKind::LocalVar(offset) => {
                self.emit(&format!("lea rax, [rbp - {}]", offset));
            }
            TypedExprKind::GlobalVar(name) => {
                let sym = self.symbol_name(name);
                if self.defined_globals.contains(name) {
                    self.emit(&format!("lea rax, [rip + {}]", sym));
                } else {
                    self.emit(&format!("mov rax, [rip + {}@GOTPCREL]", sym));
                }
            }
            TypedExprKind::Deref(inner) => {
                self.gen_expr(inner);
            }
            TypedExprKind::Member { expr, offset, .. } => {
                self.gen_lval(expr);
                if *offset > 0 {
                    self.emit(&format!("add rax, {}", offset));
                }
            }
            TypedExprKind::StmtExpr(stmts) => {
                if let Some((last, head)) = stmts.split_last() {
                    for s in head {
                        self.gen_stmt(s);
                    }
                    if let TypedStmt::Expr(last_expr) = last {
                        self.gen_lval(last_expr);
                    }
                }
            }
            TypedExprKind::Call { .. } => {
                self.gen_expr(expr);
            }
            _ => {
                panic!("Not an lvalue: {:?}", expr);
            }
        }
    }

    fn load(&mut self, ty: &Type) {
        if ty.is_array() || ty.is_struct() || ty.is_union() || ty.is_function() {
            // Decays to pointer address in rax
            return;
        }
        match ty.size() {
            1 => {
                if ty.is_signed_integer() {
                    self.emit("movsx rax, byte ptr [rax]");
                } else {
                    self.emit("movzx eax, byte ptr [rax]");
                }
            }
            2 => {
                if ty.is_signed_integer() {
                    self.emit("movsx rax, word ptr [rax]");
                } else {
                    self.emit("movzx eax, word ptr [rax]");
                }
            }
            4 => {
                if ty.is_signed_integer() {
                    self.emit("movsxd rax, dword ptr [rax]");
                } else {
                    self.emit("mov eax, dword ptr [rax]");
                }
            }
            _ => {
                self.emit("mov rax, [rax]");
            }
        }
    }

    fn store(&mut self, ty: &Type) {
        if ty.is_struct() || ty.is_union() {
            let size = ty.size();
            if size <= 128 {
                let mut offset = 0;
                while offset + 8 <= size {
                    self.emit(&format!("mov rdx, [rax + {}]", offset));
                    self.emit(&format!("mov [rdi + {}], rdx", offset));
                    offset += 8;
                }
                if offset + 4 <= size {
                    self.emit(&format!("mov edx, [rax + {}]", offset));
                    self.emit(&format!("mov [rdi + {}], edx", offset));
                    offset += 4;
                }
                if offset + 2 <= size {
                    self.emit(&format!("mov dx, [rax + {}]", offset));
                    self.emit(&format!("mov [rdi + {}], dx", offset));
                    offset += 2;
                }
                if offset < size {
                    self.emit(&format!("mov dl, [rax + {}]", offset));
                    self.emit(&format!("mov [rdi + {}], dl", offset));
                }
            } else {
                let loop_lbl = self.new_label("struct_copy");
                self.emit_load_imm("rcx", size as i64);
                self.emit("mov rsi, rax");
                self.emit_label(&loop_lbl);
                self.emit("mov dl, [rsi]");
                self.emit("mov [rdi], dl");
                self.emit("inc rsi");
                self.emit("inc rdi");
                self.emit("dec rcx");
                self.emit(&format!("jnz {}", loop_lbl));
            }
            return;
        }
        match ty.size() {
            1 => self.emit("mov byte ptr [rdi], al"),
            2 => self.emit("mov word ptr [rdi], ax"),
            4 => self.emit("mov dword ptr [rdi], eax"),
            _ => self.emit("mov [rdi], rax"),
        }
    }

    fn emit_store_local(&mut self, reg: &str, offset: i32, size: usize) {
        let (r8, r16, r32, r64) = match reg {
            "rdi" => ("dil", "di", "edi", "rdi"),
            "rsi" => ("sil", "si", "esi", "rsi"),
            "rdx" => ("dl", "dx", "edx", "rdx"),
            "rcx" => ("cl", "cx", "ecx", "rcx"),
            "r8" => ("r8b", "r8w", "r8d", "r8"),
            "r9" => ("r9b", "r9w", "r9d", "r9"),
            "rax" => ("al", "ax", "eax", "rax"),
            "r10" => ("r10b", "r10w", "r10d", "r10"),
            "r11" => ("r11b", "r11w", "r11d", "r11"),
            _ => (reg, reg, reg, reg),
        };

        match size {
            1 => self.emit(&format!("mov byte ptr [rbp - {}], {}", offset, r8)),
            2 => self.emit(&format!("mov word ptr [rbp - {}], {}", offset, r16)),
            4 => self.emit(&format!("mov dword ptr [rbp - {}], {}", offset, r32)),
            _ => self.emit(&format!("mov qword ptr [rbp - {}], {}", offset, r64)),
        }
    }

    fn store_bitfield(&mut self, ty: &Type, bw: usize, boff: usize) {
        if bw == 0 {
            return;
        }
        // Load existing word into rdx
        match ty.size() {
            1 => self.emit("movzx edx, byte ptr [rdi]"),
            2 => self.emit("movzx edx, word ptr [rdi]"),
            4 => self.emit("mov edx, dword ptr [rdi]"),
            _ => self.emit("mov rdx, [rdi]"),
        }

        let mask = (((1u128 << bw) - 1) << boff) as u64;
        let val_mask = ((1u128 << bw) - 1) as u64;

        self.emit(&format!("movabs rcx, {:#x}", !mask));
        self.emit("and rdx, rcx");
        self.emit(&format!("movabs rcx, {:#x}", val_mask));
        self.emit("and rax, rcx");
        if boff > 0 {
            self.emit(&format!("shl rax, {}", boff));
        }
        self.emit("or rdx, rax");

        // Store back
        match ty.size() {
            1 => self.emit("mov byte ptr [rdi], dl"),
            2 => self.emit("mov word ptr [rdi], dx"),
            4 => self.emit("mov dword ptr [rdi], edx"),
            _ => self.emit("mov [rdi], rdx"),
        }

        // Return bitfield value in rax
        if boff > 0 {
            self.emit(&format!("shr rax, {}", boff));
        }
        if ty.is_signed_integer() && bw < 64 {
            let shift = 64 - bw;
            self.emit(&format!("shl rax, {}", shift));
            self.emit(&format!("sar rax, {}", shift));
        }
    }

    pub fn gen_expr(&mut self, expr: &TypedExpr) {
        let d = self.depth.get() + 1;
        self.depth.set(d);
        if d > 200 {
            panic!(
                "Infinite recursion in gen_expr! func: {}, expr: {:?}",
                self.current_func, expr
            );
        }
        self.gen_expr_inner(expr);
        self.depth.set(d - 1);
    }

    fn gen_expr_inner(&mut self, expr: &TypedExpr) {
        match &expr.kind {
            TypedExprKind::Int(v) => {
                self.emit_load_imm("rax", *v);
            }
            TypedExprKind::Float(v) => {
                self.emit_load_imm("rax", v.to_bits() as i64);
            }
            TypedExprKind::Char(c) => {
                self.emit(&format!("mov eax, {}", *c as u32));
            }
            TypedExprKind::StringLiteral(label) => {
                self.emit(&format!("lea rax, [rip + {}]", label));
            }
            TypedExprKind::AddrOfLabel(lbl) => {
                let sym = format!(".L.user.{}", lbl);
                self.emit(&format!("lea rax, [rip + {}]", sym));
            }
            TypedExprKind::LocalVar(_) | TypedExprKind::GlobalVar(_) | TypedExprKind::Deref(_) => {
                self.gen_lval(expr);
                self.load(&expr.ty);
            }
            TypedExprKind::Member {
                bit_width,
                bit_offset,
                ..
            } => {
                self.gen_lval(expr);
                self.load(&expr.ty);
                if let (Some(bw), Some(boff)) = (*bit_width, *bit_offset)
                    && bw > 0
                {
                    if boff > 0 {
                        self.emit(&format!("shr rax, {}", boff));
                    }
                    let val_mask = ((1u128 << bw) - 1) as u64;
                    self.emit(&format!("movabs rcx, {:#x}", val_mask));
                    self.emit("and rax, rcx");
                    if expr.ty.is_signed_integer() && bw < 64 {
                        let shift = 64 - bw;
                        self.emit(&format!("shl rax, {}", shift));
                        self.emit(&format!("sar rax, {}", shift));
                    }
                }
            }
            TypedExprKind::AddrOf(inner) => {
                self.gen_lval(inner);
            }
            TypedExprKind::Cast { expr: inner } => {
                self.gen_expr(inner);
                if matches!(expr.ty.kind, TypeKind::Bool) {
                    if matches!(inner.ty.kind, TypeKind::Double) {
                        self.emit("movq xmm0, rax");
                        self.emit("xorpd xmm1, xmm1");
                        self.emit("ucomisd xmm0, xmm1");
                        self.emit("setne al");
                        self.emit("setp cl");
                        self.emit("or al, cl");
                        self.emit("movzx eax, al");
                    } else if matches!(inner.ty.kind, TypeKind::Float) {
                        self.emit("movd xmm0, eax");
                        self.emit("xorps xmm1, xmm1");
                        self.emit("ucomiss xmm0, xmm1");
                        self.emit("setne al");
                        self.emit("setp cl");
                        self.emit("or al, cl");
                        self.emit("movzx eax, al");
                    } else {
                        self.emit("test rax, rax");
                        self.emit("setne al");
                        self.emit("movzx eax, al");
                    }
                } else if matches!(expr.ty.kind, TypeKind::Double) {
                    if matches!(inner.ty.kind, TypeKind::Double) {
                        // no-op
                    } else if matches!(inner.ty.kind, TypeKind::Float) {
                        self.emit("movd xmm0, eax");
                        self.emit("cvtss2sd xmm0, xmm0");
                        self.emit("movq rax, xmm0");
                    } else if inner.ty.is_integer() {
                        if inner.ty.size() == 8 {
                            if inner.ty.is_signed_integer() {
                                self.emit("cvtsi2sd xmm0, rax");
                            } else {
                                let lbl_neg = self.new_label("u2d_neg");
                                let lbl_end = self.new_label("u2d_end");
                                self.emit("test rax, rax");
                                self.emit(&format!("js {}", lbl_neg));
                                self.emit("cvtsi2sd xmm0, rax");
                                self.emit(&format!("jmp {}", lbl_end));
                                self.emit_label(&lbl_neg);
                                self.emit("mov rdx, rax");
                                self.emit("shr rdx, 1");
                                self.emit("and rax, 1");
                                self.emit("or rdx, rax");
                                self.emit("cvtsi2sd xmm0, rdx");
                                self.emit("addsd xmm0, xmm0");
                                self.emit_label(&lbl_end);
                            }
                        } else if inner.ty.is_signed_integer() {
                            self.emit("movsxd rax, eax");
                            self.emit("cvtsi2sd xmm0, rax");
                        } else {
                            self.emit("mov eax, eax");
                            self.emit("cvtsi2sd xmm0, rax");
                        }
                        self.emit("movq rax, xmm0");
                    }
                } else if matches!(expr.ty.kind, TypeKind::Float) {
                    if matches!(inner.ty.kind, TypeKind::Double) {
                        self.emit("movq xmm0, rax");
                        self.emit("cvtsd2ss xmm0, xmm0");
                        self.emit("movd eax, xmm0");
                    } else if matches!(inner.ty.kind, TypeKind::Float) {
                        // no-op
                    } else if inner.ty.is_integer() {
                        if inner.ty.size() == 8 {
                            if inner.ty.is_signed_integer() {
                                self.emit("cvtsi2ss xmm0, rax");
                            } else {
                                let lbl_neg = self.new_label("u2f_neg");
                                let lbl_end = self.new_label("u2f_end");
                                self.emit("test rax, rax");
                                self.emit(&format!("js {}", lbl_neg));
                                self.emit("cvtsi2ss xmm0, rax");
                                self.emit(&format!("jmp {}", lbl_end));
                                self.emit_label(&lbl_neg);
                                self.emit("mov rdx, rax");
                                self.emit("shr rdx, 1");
                                self.emit("and rax, 1");
                                self.emit("or rdx, rax");
                                self.emit("cvtsi2ss xmm0, rdx");
                                self.emit("addss xmm0, xmm0");
                                self.emit_label(&lbl_end);
                            }
                        } else if inner.ty.is_signed_integer() {
                            self.emit("movsxd rax, eax");
                            self.emit("cvtsi2ss xmm0, rax");
                        } else {
                            self.emit("mov eax, eax");
                            self.emit("cvtsi2ss xmm0, rax");
                        }
                        self.emit("movd eax, xmm0");
                    }
                } else if matches!(inner.ty.kind, TypeKind::Double) {
                    self.emit("movq xmm0, rax");
                    self.emit("cvttsd2si rax, xmm0");
                    match expr.ty.size() {
                        1 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("movsx rax, al");
                            } else {
                                self.emit("movzx eax, al");
                            }
                        }
                        2 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("movsx rax, ax");
                            } else {
                                self.emit("movzx eax, ax");
                            }
                        }
                        4 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("movsxd rax, eax");
                            } else {
                                self.emit("mov eax, eax");
                            }
                        }
                        _ => {}
                    }
                } else if matches!(inner.ty.kind, TypeKind::Float) {
                    self.emit("movd xmm0, eax");
                    self.emit("cvttss2si rax, xmm0");
                    match expr.ty.size() {
                        1 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("movsx rax, al");
                            } else {
                                self.emit("movzx eax, al");
                            }
                        }
                        2 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("movsx rax, ax");
                            } else {
                                self.emit("movzx eax, ax");
                            }
                        }
                        4 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("movsxd rax, eax");
                            } else {
                                self.emit("mov eax, eax");
                            }
                        }
                        _ => {}
                    }
                } else {
                    // Integer to Integer cast
                    match expr.ty.size() {
                        1 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("movsx rax, al");
                            } else {
                                self.emit("movzx eax, al");
                            }
                        }
                        2 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("movsx rax, ax");
                            } else {
                                self.emit("movzx eax, ax");
                            }
                        }
                        4 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("movsxd rax, eax");
                            } else {
                                self.emit("mov eax, eax");
                            }
                        }
                        8 if inner.ty.is_integer() && inner.ty.size() < 8 => {
                            if inner.ty.is_signed_integer() {
                                match inner.ty.size() {
                                    1 => self.emit("movsx rax, al"),
                                    2 => self.emit("movsx rax, ax"),
                                    4 => self.emit("movsxd rax, eax"),
                                    _ => {}
                                }
                            } else {
                                match inner.ty.size() {
                                    1 => self.emit("movzx eax, al"),
                                    2 => self.emit("movzx eax, ax"),
                                    4 => self.emit("mov eax, eax"),
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            TypedExprKind::Binary { op, lhs, rhs } => match op {
                BinaryOp::Assign => {
                    self.gen_expr(rhs);
                    self.push();
                    self.gen_lval(lhs);
                    self.pop("rdi");
                    self.emit("xchg rax, rdi"); // rax = value, rdi = address
                    if let TypedExprKind::Member {
                        bit_width: Some(bw),
                        bit_offset: Some(boff),
                        ..
                    } = &lhs.kind
                    {
                        self.store_bitfield(&lhs.ty, *bw, *boff);
                    } else {
                        self.store(&lhs.ty);
                    }
                }
                BinaryOp::PlusAssign
                | BinaryOp::MinusAssign
                | BinaryOp::StarAssign
                | BinaryOp::SlashAssign
                | BinaryOp::PercentAssign
                | BinaryOp::AmpAssign
                | BinaryOp::PipeAssign
                | BinaryOp::CaretAssign
                | BinaryOp::ShlAssign
                | BinaryOp::ShrAssign => {
                    self.gen_lval(lhs);
                    self.push(); // [dest_addr]
                    self.gen_expr(lhs);
                    self.push(); // [dest_addr, lhs_val]
                    self.gen_expr(rhs); // rax = rhs_val
                    self.pop("rdi"); // rdi = lhs_val, rax = rhs_val
                    match op {
                        BinaryOp::PlusAssign => {
                            if matches!(lhs.ty.kind, TypeKind::Double) {
                                self.emit("movq xmm1, rdi");
                                self.emit("movq xmm0, rax");
                                self.emit("addsd xmm0, xmm1");
                                self.emit("movq rax, xmm0");
                            } else if matches!(lhs.ty.kind, TypeKind::Float) {
                                self.emit("movd xmm1, edi");
                                self.emit("movd xmm0, eax");
                                self.emit("addss xmm0, xmm1");
                                self.emit("movd eax, xmm0");
                            } else {
                                self.emit("add rax, rdi");
                            }
                        }
                        BinaryOp::MinusAssign => {
                            if matches!(lhs.ty.kind, TypeKind::Double) {
                                self.emit("movq xmm1, rdi");
                                self.emit("movq xmm0, rax");
                                self.emit("subsd xmm1, xmm0");
                                self.emit("movq rax, xmm1");
                            } else if matches!(lhs.ty.kind, TypeKind::Float) {
                                self.emit("movd xmm1, edi");
                                self.emit("movd xmm0, eax");
                                self.emit("subss xmm1, xmm0");
                                self.emit("movd eax, xmm1");
                            } else {
                                self.emit("sub rdi, rax");
                                self.emit("mov rax, rdi");
                            }
                        }
                        BinaryOp::StarAssign => {
                            if matches!(lhs.ty.kind, TypeKind::Double) {
                                self.emit("movq xmm1, rdi");
                                self.emit("movq xmm0, rax");
                                self.emit("mulsd xmm0, xmm1");
                                self.emit("movq rax, xmm0");
                            } else if matches!(lhs.ty.kind, TypeKind::Float) {
                                self.emit("movd xmm1, edi");
                                self.emit("movd xmm0, eax");
                                self.emit("mulss xmm0, xmm1");
                                self.emit("movd eax, xmm0");
                            } else {
                                self.emit("imul rax, rdi");
                            }
                        }
                        BinaryOp::SlashAssign => {
                            if matches!(lhs.ty.kind, TypeKind::Double) {
                                self.emit("movq xmm1, rdi");
                                self.emit("movq xmm0, rax");
                                self.emit("divsd xmm1, xmm0");
                                self.emit("movq rax, xmm1");
                            } else if matches!(lhs.ty.kind, TypeKind::Float) {
                                self.emit("movd xmm1, edi");
                                self.emit("movd xmm0, eax");
                                self.emit("divss xmm1, xmm0");
                                self.emit("movd eax, xmm1");
                            } else if lhs.ty.is_signed_integer() {
                                self.emit("mov rcx, rax");
                                self.emit("mov rax, rdi");
                                self.emit("cqo");
                                self.emit("idiv rcx");
                            } else {
                                self.emit("mov rcx, rax");
                                self.emit("mov rax, rdi");
                                self.emit("xor edx, edx");
                                self.emit("div rcx");
                            }
                        }
                        BinaryOp::PercentAssign => {
                            if lhs.ty.is_signed_integer() {
                                self.emit("mov rcx, rax");
                                self.emit("mov rax, rdi");
                                self.emit("cqo");
                                self.emit("idiv rcx");
                                self.emit("mov rax, rdx");
                            } else {
                                self.emit("mov rcx, rax");
                                self.emit("mov rax, rdi");
                                self.emit("xor edx, edx");
                                self.emit("div rcx");
                                self.emit("mov rax, rdx");
                            }
                        }
                        BinaryOp::AmpAssign => self.emit("and rax, rdi"),
                        BinaryOp::PipeAssign => self.emit("or rax, rdi"),
                        BinaryOp::CaretAssign => self.emit("xor rax, rdi"),
                        BinaryOp::ShlAssign => {
                            self.emit("mov rcx, rax");
                            self.emit("mov rax, rdi");
                            self.emit("shl rax, cl");
                        }
                        BinaryOp::ShrAssign => {
                            self.emit("mov rcx, rax");
                            self.emit("mov rax, rdi");
                            if lhs.ty.is_signed_integer() {
                                self.emit("sar rax, cl");
                            } else {
                                self.emit("shr rax, cl");
                            }
                        }
                        _ => unreachable!(),
                    }
                    self.pop("rdi"); // rdi = dest_addr, rax = result_val
                    if let TypedExprKind::Member {
                        bit_width: Some(bw),
                        bit_offset: Some(boff),
                        ..
                    } = &lhs.kind
                    {
                        self.store_bitfield(&lhs.ty, *bw, *boff);
                    } else {
                        self.store(&lhs.ty);
                    }
                }
                BinaryOp::LogicalAnd => {
                    let false_label = self.new_label("land_false");
                    let end_label = self.new_label("land_end");

                    self.gen_expr(lhs);
                    self.emit("test rax, rax");
                    self.emit(&format!("jz {}", false_label));

                    self.gen_expr(rhs);
                    self.emit("test rax, rax");
                    self.emit(&format!("jz {}", false_label));

                    self.emit("mov eax, 1");
                    self.emit(&format!("jmp {}", end_label));

                    self.emit_label(&false_label);
                    self.emit("xor eax, eax");

                    self.emit_label(&end_label);
                }
                BinaryOp::LogicalOr => {
                    let true_label = self.new_label("lor_true");
                    let end_label = self.new_label("lor_end");

                    self.gen_expr(lhs);
                    self.emit("test rax, rax");
                    self.emit(&format!("jnz {}", true_label));

                    self.gen_expr(rhs);
                    self.emit("test rax, rax");
                    self.emit(&format!("jnz {}", true_label));

                    self.emit("xor eax, eax");
                    self.emit(&format!("jmp {}", end_label));

                    self.emit_label(&true_label);
                    self.emit("mov eax, 1");

                    self.emit_label(&end_label);
                }
                BinaryOp::Comma => {
                    self.gen_expr(lhs);
                    self.gen_expr(rhs);
                }
                _ => {
                    self.gen_expr(lhs);
                    self.push();
                    self.gen_expr(rhs);
                    self.pop("rdi"); // rdi = lhs, rax = rhs

                    match op {
                        BinaryOp::Add => {
                            if matches!(expr.ty.kind, TypeKind::Double) {
                                self.emit("movq xmm1, rdi");
                                self.emit("movq xmm0, rax");
                                self.emit("addsd xmm0, xmm1");
                                self.emit("movq rax, xmm0");
                            } else if matches!(expr.ty.kind, TypeKind::Float) {
                                self.emit("movd xmm1, edi");
                                self.emit("movd xmm0, eax");
                                self.emit("addss xmm0, xmm1");
                                self.emit("movd eax, xmm0");
                            } else {
                                self.emit("add rax, rdi");
                            }
                        }
                        BinaryOp::Sub => {
                            if matches!(expr.ty.kind, TypeKind::Double) {
                                self.emit("movq xmm1, rdi");
                                self.emit("movq xmm0, rax");
                                self.emit("subsd xmm1, xmm0");
                                self.emit("movq rax, xmm1");
                            } else if matches!(expr.ty.kind, TypeKind::Float) {
                                self.emit("movd xmm1, edi");
                                self.emit("movd xmm0, eax");
                                self.emit("subss xmm1, xmm0");
                                self.emit("movd eax, xmm1");
                            } else {
                                self.emit("sub rdi, rax");
                                self.emit("mov rax, rdi");
                            }
                        }
                        BinaryOp::Mul => {
                            if matches!(expr.ty.kind, TypeKind::Double) {
                                self.emit("movq xmm1, rdi");
                                self.emit("movq xmm0, rax");
                                self.emit("mulsd xmm0, xmm1");
                                self.emit("movq rax, xmm0");
                            } else if matches!(expr.ty.kind, TypeKind::Float) {
                                self.emit("movd xmm1, edi");
                                self.emit("movd xmm0, eax");
                                self.emit("mulss xmm0, xmm1");
                                self.emit("movd eax, xmm0");
                            } else {
                                let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                                if is_32bit {
                                    self.emit("imul eax, edi");
                                    if expr.ty.is_signed_integer() {
                                        self.emit("movsxd rax, eax");
                                    } else {
                                        self.emit("mov eax, eax");
                                    }
                                } else {
                                    self.emit("imul rax, rdi");
                                }
                            }
                        }
                        BinaryOp::Div => {
                            if matches!(expr.ty.kind, TypeKind::Double) {
                                self.emit("movq xmm1, rdi");
                                self.emit("movq xmm0, rax");
                                self.emit("divsd xmm1, xmm0");
                                self.emit("movq rax, xmm1");
                            } else if matches!(expr.ty.kind, TypeKind::Float) {
                                self.emit("movd xmm1, edi");
                                self.emit("movd xmm0, eax");
                                self.emit("divss xmm1, xmm0");
                                self.emit("movd eax, xmm1");
                            } else {
                                let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                                if is_32bit {
                                    if expr.ty.is_signed_integer() {
                                        self.emit("mov ecx, eax");
                                        self.emit("mov eax, edi");
                                        self.emit("cdq");
                                        self.emit("idiv ecx");
                                        self.emit("movsxd rax, eax");
                                    } else {
                                        self.emit("mov ecx, eax");
                                        self.emit("mov eax, edi");
                                        self.emit("xor edx, edx");
                                        self.emit("div ecx");
                                        self.emit("mov eax, eax");
                                    }
                                } else if expr.ty.is_signed_integer() {
                                    self.emit("mov rcx, rax");
                                    self.emit("mov rax, rdi");
                                    self.emit("cqo");
                                    self.emit("idiv rcx");
                                } else {
                                    self.emit("mov rcx, rax");
                                    self.emit("mov rax, rdi");
                                    self.emit("xor edx, edx");
                                    self.emit("div rcx");
                                }
                            }
                        }
                        BinaryOp::Rem => {
                            let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                            if is_32bit {
                                if expr.ty.is_signed_integer() {
                                    self.emit("mov ecx, eax");
                                    self.emit("mov eax, edi");
                                    self.emit("cdq");
                                    self.emit("idiv ecx");
                                    self.emit("movsxd rax, edx");
                                } else {
                                    self.emit("mov ecx, eax");
                                    self.emit("mov eax, edi");
                                    self.emit("xor edx, edx");
                                    self.emit("div ecx");
                                    self.emit("mov eax, edx");
                                }
                            } else if expr.ty.is_signed_integer() {
                                self.emit("mov rcx, rax");
                                self.emit("mov rax, rdi");
                                self.emit("cqo");
                                self.emit("idiv rcx");
                                self.emit("mov rax, rdx");
                            } else {
                                self.emit("mov rcx, rax");
                                self.emit("mov rax, rdi");
                                self.emit("xor edx, edx");
                                self.emit("div rcx");
                                self.emit("mov rax, rdx");
                            }
                        }
                        BinaryOp::BitAnd => {
                            let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                            if is_32bit {
                                self.emit("and eax, edi");
                                self.emit("mov eax, eax");
                            } else {
                                self.emit("and rax, rdi");
                            }
                        }
                        BinaryOp::BitOr => {
                            let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                            if is_32bit {
                                self.emit("or eax, edi");
                                self.emit("mov eax, eax");
                            } else {
                                self.emit("or rax, rdi");
                            }
                        }
                        BinaryOp::BitXor => {
                            let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                            if is_32bit {
                                self.emit("xor eax, edi");
                                self.emit("mov eax, eax");
                            } else {
                                self.emit("xor rax, rdi");
                            }
                        }
                        BinaryOp::Shl => {
                            let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                            self.emit("mov rcx, rax");
                            self.emit("mov rax, rdi");
                            if is_32bit {
                                self.emit("shl eax, cl");
                                if expr.ty.is_signed_integer() {
                                    self.emit("movsxd rax, eax");
                                } else {
                                    self.emit("mov eax, eax");
                                }
                            } else {
                                self.emit("shl rax, cl");
                            }
                        }
                        BinaryOp::Shr => {
                            let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                            self.emit("mov rcx, rax");
                            self.emit("mov rax, rdi");
                            if is_32bit {
                                if expr.ty.is_signed_integer() {
                                    self.emit("sar eax, cl");
                                    self.emit("movsxd rax, eax");
                                } else {
                                    self.emit("shr eax, cl");
                                    self.emit("mov eax, eax");
                                }
                            } else if expr.ty.is_signed_integer() {
                                self.emit("sar rax, cl");
                            } else {
                                self.emit("shr rax, cl");
                            }
                        }
                        BinaryOp::Eq => {
                            if matches!(lhs.ty.kind, TypeKind::Double)
                                || matches!(rhs.ty.kind, TypeKind::Double)
                            {
                                self.emit("movq xmm1, rdi");
                                self.emit("movq xmm0, rax");
                                self.emit("ucomisd xmm1, xmm0");
                                self.emit("sete al");
                                self.emit("setnp cl");
                                self.emit("and al, cl");
                            } else if matches!(lhs.ty.kind, TypeKind::Float)
                                || matches!(rhs.ty.kind, TypeKind::Float)
                            {
                                self.emit("movd xmm1, edi");
                                self.emit("movd xmm0, eax");
                                self.emit("ucomiss xmm1, xmm0");
                                self.emit("sete al");
                                self.emit("setnp cl");
                                self.emit("and al, cl");
                            } else {
                                let is_32bit = lhs.ty.size() <= 4
                                    && rhs.ty.size() <= 4
                                    && !lhs.ty.is_pointer()
                                    && !rhs.ty.is_pointer();
                                if is_32bit {
                                    self.emit("cmp edi, eax");
                                } else {
                                    self.emit("cmp rdi, rax");
                                }
                                self.emit("sete al");
                            }
                            self.emit("movzx eax, al");
                        }
                        BinaryOp::Ne => {
                            if matches!(lhs.ty.kind, TypeKind::Double)
                                || matches!(rhs.ty.kind, TypeKind::Double)
                            {
                                self.emit("movq xmm1, rdi");
                                self.emit("movq xmm0, rax");
                                self.emit("ucomisd xmm1, xmm0");
                                self.emit("setne al");
                                self.emit("setp cl");
                                self.emit("or al, cl");
                            } else if matches!(lhs.ty.kind, TypeKind::Float)
                                || matches!(rhs.ty.kind, TypeKind::Float)
                            {
                                self.emit("movd xmm1, edi");
                                self.emit("movd xmm0, eax");
                                self.emit("ucomiss xmm1, xmm0");
                                self.emit("setne al");
                                self.emit("setp cl");
                                self.emit("or al, cl");
                            } else {
                                let is_32bit = lhs.ty.size() <= 4
                                    && rhs.ty.size() <= 4
                                    && !lhs.ty.is_pointer()
                                    && !rhs.ty.is_pointer();
                                if is_32bit {
                                    self.emit("cmp edi, eax");
                                } else {
                                    self.emit("cmp rdi, rax");
                                }
                                self.emit("setne al");
                            }
                            self.emit("movzx eax, al");
                        }
                        BinaryOp::Lt => {
                            if matches!(lhs.ty.kind, TypeKind::Double)
                                || matches!(rhs.ty.kind, TypeKind::Double)
                            {
                                self.emit("movq xmm1, rdi");
                                self.emit("movq xmm0, rax");
                                self.emit("ucomisd xmm1, xmm0");
                                self.emit("setb al");
                            } else if matches!(lhs.ty.kind, TypeKind::Float)
                                || matches!(rhs.ty.kind, TypeKind::Float)
                            {
                                self.emit("movd xmm1, edi");
                                self.emit("movd xmm0, eax");
                                self.emit("ucomiss xmm1, xmm0");
                                self.emit("setb al");
                            } else {
                                let is_signed = lhs.ty.is_signed_integer()
                                    && rhs.ty.is_signed_integer()
                                    && !lhs.ty.is_pointer()
                                    && !rhs.ty.is_pointer();
                                let is_32bit = lhs.ty.size() <= 4
                                    && rhs.ty.size() <= 4
                                    && !lhs.ty.is_pointer()
                                    && !rhs.ty.is_pointer();
                                if is_32bit {
                                    self.emit("cmp edi, eax");
                                } else {
                                    self.emit("cmp rdi, rax");
                                }
                                if is_signed {
                                    self.emit("setl al");
                                } else {
                                    self.emit("setb al");
                                }
                            }
                            self.emit("movzx eax, al");
                        }
                        BinaryOp::Le => {
                            if matches!(lhs.ty.kind, TypeKind::Double)
                                || matches!(rhs.ty.kind, TypeKind::Double)
                            {
                                self.emit("movq xmm1, rdi");
                                self.emit("movq xmm0, rax");
                                self.emit("ucomisd xmm1, xmm0");
                                self.emit("setbe al");
                            } else if matches!(lhs.ty.kind, TypeKind::Float)
                                || matches!(rhs.ty.kind, TypeKind::Float)
                            {
                                self.emit("movd xmm1, edi");
                                self.emit("movd xmm0, eax");
                                self.emit("ucomiss xmm1, xmm0");
                                self.emit("setbe al");
                            } else {
                                let is_signed = lhs.ty.is_signed_integer()
                                    && rhs.ty.is_signed_integer()
                                    && !lhs.ty.is_pointer()
                                    && !rhs.ty.is_pointer();
                                let is_32bit = lhs.ty.size() <= 4
                                    && rhs.ty.size() <= 4
                                    && !lhs.ty.is_pointer()
                                    && !rhs.ty.is_pointer();
                                if is_32bit {
                                    self.emit("cmp edi, eax");
                                } else {
                                    self.emit("cmp rdi, rax");
                                }
                                if is_signed {
                                    self.emit("setle al");
                                } else {
                                    self.emit("setbe al");
                                }
                            }
                            self.emit("movzx eax, al");
                        }
                        BinaryOp::Gt => {
                            if matches!(lhs.ty.kind, TypeKind::Double)
                                || matches!(rhs.ty.kind, TypeKind::Double)
                            {
                                self.emit("movq xmm1, rdi");
                                self.emit("movq xmm0, rax");
                                self.emit("ucomisd xmm1, xmm0");
                                self.emit("seta al");
                            } else if matches!(lhs.ty.kind, TypeKind::Float)
                                || matches!(rhs.ty.kind, TypeKind::Float)
                            {
                                self.emit("movd xmm1, edi");
                                self.emit("movd xmm0, eax");
                                self.emit("ucomiss xmm1, xmm0");
                                self.emit("seta al");
                            } else {
                                let is_signed = lhs.ty.is_signed_integer()
                                    && rhs.ty.is_signed_integer()
                                    && !lhs.ty.is_pointer()
                                    && !rhs.ty.is_pointer();
                                let is_32bit = lhs.ty.size() <= 4
                                    && rhs.ty.size() <= 4
                                    && !lhs.ty.is_pointer()
                                    && !rhs.ty.is_pointer();
                                if is_32bit {
                                    self.emit("cmp edi, eax");
                                } else {
                                    self.emit("cmp rdi, rax");
                                }
                                if is_signed {
                                    self.emit("setg al");
                                } else {
                                    self.emit("seta al");
                                }
                            }
                            self.emit("movzx eax, al");
                        }
                        BinaryOp::Ge => {
                            if matches!(lhs.ty.kind, TypeKind::Double)
                                || matches!(rhs.ty.kind, TypeKind::Double)
                            {
                                self.emit("movq xmm1, rdi");
                                self.emit("movq xmm0, rax");
                                self.emit("ucomisd xmm1, xmm0");
                                self.emit("setae al");
                            } else if matches!(lhs.ty.kind, TypeKind::Float)
                                || matches!(rhs.ty.kind, TypeKind::Float)
                            {
                                self.emit("movd xmm1, edi");
                                self.emit("movd xmm0, eax");
                                self.emit("ucomiss xmm1, xmm0");
                                self.emit("setae al");
                            } else {
                                let is_signed = lhs.ty.is_signed_integer()
                                    && rhs.ty.is_signed_integer()
                                    && !lhs.ty.is_pointer()
                                    && !rhs.ty.is_pointer();
                                let is_32bit = lhs.ty.size() <= 4
                                    && rhs.ty.size() <= 4
                                    && !lhs.ty.is_pointer()
                                    && !rhs.ty.is_pointer();
                                if is_32bit {
                                    self.emit("cmp edi, eax");
                                } else {
                                    self.emit("cmp rdi, rax");
                                }
                                if is_signed {
                                    self.emit("setge al");
                                } else {
                                    self.emit("setae al");
                                }
                            }
                            self.emit("movzx eax, al");
                        }
                        _ => {}
                    }
                }
            },
            TypedExprKind::Unary { op, expr: inner } => match op {
                UnaryOp::Pos => {
                    self.gen_expr(inner);
                }
                UnaryOp::Neg => {
                    self.gen_expr(inner);
                    if matches!(inner.ty.kind, TypeKind::Double) {
                        self.emit("movq xmm0, rax");
                        self.emit("movabs rcx, -9223372036854775808"); // 0x8000000000000000
                        self.emit("movq xmm1, rcx");
                        self.emit("xorpd xmm0, xmm1");
                        self.emit("movq rax, xmm0");
                    } else if matches!(inner.ty.kind, TypeKind::Float) {
                        self.emit("movd xmm0, eax");
                        self.emit("mov ecx, -2147483648"); // 0x80000000
                        self.emit("movd xmm1, ecx");
                        self.emit("xorps xmm0, xmm1");
                        self.emit("movd eax, xmm0");
                    } else {
                        self.emit("neg rax");
                    }
                }
                UnaryOp::BitNot => {
                    self.gen_expr(inner);
                    self.emit("not rax");
                }
                UnaryOp::LogNot => {
                    self.gen_expr(inner);
                    self.emit("test rax, rax");
                    self.emit("sete al");
                    self.emit("movzx eax, al");
                }
                UnaryOp::PreInc => {
                    let step = if inner.ty.is_pointer() {
                        inner.ty.get_pointer_base().map_or(1, |b| b.size())
                    } else {
                        1
                    };
                    self.gen_lval(inner);
                    self.push();
                    self.gen_expr(inner);
                    self.emit(&format!("add rax, {}", step));
                    self.pop("rdi"); // rdi = dest_addr, rax = val
                    if let TypedExprKind::Member {
                        bit_width: Some(bw),
                        bit_offset: Some(boff),
                        ..
                    } = &inner.kind
                    {
                        self.store_bitfield(&inner.ty, *bw, *boff);
                    } else {
                        self.store(&inner.ty);
                    }
                }
                UnaryOp::PreDec => {
                    let step = if inner.ty.is_pointer() {
                        inner.ty.get_pointer_base().map_or(1, |b| b.size())
                    } else {
                        1
                    };
                    self.gen_lval(inner);
                    self.push();
                    self.gen_expr(inner);
                    self.emit(&format!("sub rax, {}", step));
                    self.pop("rdi");
                    if let TypedExprKind::Member {
                        bit_width: Some(bw),
                        bit_offset: Some(boff),
                        ..
                    } = &inner.kind
                    {
                        self.store_bitfield(&inner.ty, *bw, *boff);
                    } else {
                        self.store(&inner.ty);
                    }
                }
                UnaryOp::PostInc => {
                    let step = if inner.ty.is_pointer() {
                        inner.ty.get_pointer_base().map_or(1, |b| b.size())
                    } else {
                        1
                    };
                    self.gen_lval(inner);
                    self.push(); // [addr]
                    self.gen_expr(inner);
                    self.push(); // [addr, old_val]
                    self.emit(&format!("add rax, {}", step)); // new_val in rax
                    self.emit("mov rdx, rax"); // new_val in rdx
                    self.pop("rcx"); // rcx = old_val
                    self.pop("rdi"); // rdi = addr
                    self.emit("mov rax, rdx"); // rax = new_val
                    if let TypedExprKind::Member {
                        bit_width: Some(bw),
                        bit_offset: Some(boff),
                        ..
                    } = &inner.kind
                    {
                        self.store_bitfield(&inner.ty, *bw, *boff);
                    } else {
                        self.store(&inner.ty);
                    }
                    self.emit("mov rax, rcx"); // return old_val
                }
                UnaryOp::PostDec => {
                    let step = if inner.ty.is_pointer() {
                        inner.ty.get_pointer_base().map_or(1, |b| b.size())
                    } else {
                        1
                    };
                    self.gen_lval(inner);
                    self.push(); // [addr]
                    self.gen_expr(inner);
                    self.push(); // [addr, old_val]
                    self.emit(&format!("sub rax, {}", step)); // new_val in rax
                    self.emit("mov rdx, rax"); // new_val in rdx
                    self.pop("rcx"); // rcx = old_val
                    self.pop("rdi"); // rdi = addr
                    self.emit("mov rax, rdx"); // rax = new_val
                    if let TypedExprKind::Member {
                        bit_width: Some(bw),
                        bit_offset: Some(boff),
                        ..
                    } = &inner.kind
                    {
                        self.store_bitfield(&inner.ty, *bw, *boff);
                    } else {
                        self.store(&inner.ty);
                    }
                    self.emit("mov rax, rcx"); // return old_val
                }
                UnaryOp::Deref
                | UnaryOp::AddrOf
                | UnaryOp::Sizeof
                | UnaryOp::Alignof
                | UnaryOp::AddrOfLabel => {}
            },
            TypedExprKind::Ternary {
                cond,
                then_expr,
                else_expr,
            } => {
                let else_label = self.new_label("ternary_else");
                let end_label = self.new_label("ternary_end");

                self.gen_expr(cond);
                self.emit("test rax, rax");
                self.emit(&format!("jz {}", else_label));

                self.gen_expr(then_expr);
                self.emit(&format!("jmp {}", end_label));

                self.emit_label(&else_label);
                self.gen_expr(else_expr);

                self.emit_label(&end_label);
            }
            TypedExprKind::Call { callee, args } => {
                if let TypedExprKind::GlobalVar(name) = &callee.kind {
                    match name.as_str() {
                        "__builtin_expect" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            } else {
                                self.emit("xor eax, eax");
                            }
                            return;
                        }
                        "__builtin_clz" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            let nz_lbl = self.new_label("clz_nz");
                            let end_lbl = self.new_label("clz_end");
                            self.emit("test eax, eax");
                            self.emit(&format!("jnz {}", nz_lbl));
                            self.emit("mov eax, 32");
                            self.emit(&format!("jmp {}", end_lbl));
                            self.emit_label(&nz_lbl);
                            self.emit("bsr eax, eax");
                            self.emit("xor eax, 31");
                            self.emit_label(&end_lbl);
                            self.emit("mov eax, eax");
                            return;
                        }
                        "__builtin_clzll" | "__builtin_clzl" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            let nz_lbl = self.new_label("clzll_nz");
                            let end_lbl = self.new_label("clzll_end");
                            self.emit("test rax, rax");
                            self.emit(&format!("jnz {}", nz_lbl));
                            self.emit("mov eax, 64");
                            self.emit(&format!("jmp {}", end_lbl));
                            self.emit_label(&nz_lbl);
                            self.emit("bsr rax, rax");
                            self.emit("xor rax, 63");
                            self.emit_label(&end_lbl);
                            return;
                        }
                        "__builtin_ctz" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            let nz_lbl = self.new_label("ctz_nz");
                            let end_lbl = self.new_label("ctz_end");
                            self.emit("test eax, eax");
                            self.emit(&format!("jnz {}", nz_lbl));
                            self.emit("mov eax, 32");
                            self.emit(&format!("jmp {}", end_lbl));
                            self.emit_label(&nz_lbl);
                            self.emit("bsf eax, eax");
                            self.emit_label(&end_lbl);
                            self.emit("mov eax, eax");
                            return;
                        }
                        "__builtin_ctzll" | "__builtin_ctzl" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            let nz_lbl = self.new_label("ctzll_nz");
                            let end_lbl = self.new_label("ctzll_end");
                            self.emit("test rax, rax");
                            self.emit(&format!("jnz {}", nz_lbl));
                            self.emit("mov eax, 64");
                            self.emit(&format!("jmp {}", end_lbl));
                            self.emit_label(&nz_lbl);
                            self.emit("bsf rax, rax");
                            self.emit_label(&end_lbl);
                            return;
                        }
                        "__builtin_bswap16" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            self.emit("rol ax, 8");
                            self.emit("movzx eax, ax");
                            return;
                        }
                        "__builtin_bswap32" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            self.emit("bswap eax");
                            self.emit("mov eax, eax");
                            return;
                        }
                        "__builtin_bswap64" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            self.emit("bswap rax");
                            return;
                        }
                        "__builtin_unreachable" => {
                            self.emit("ud2");
                            return;
                        }
                        "__builtin_constant_p" => {
                            self.emit("xor eax, eax");
                            return;
                        }
                        "__builtin_frame_address" => {
                            self.emit("mov rax, rbp");
                            return;
                        }
                        "__builtin_inff" | "__builtin_inf" | "__builtin_huge_val" => {
                            self.emit_load_imm("rax", 0x7ff0000000000000_u64 as i64);
                            return;
                        }
                        "__builtin_nanf" | "__builtin_nan" => {
                            self.emit_load_imm("rax", 0x7ff8000000000000_u64 as i64);
                            return;
                        }
                        "__builtin_va_start" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                                let va_offset = self.current_func_va_offset;
                                let named_count = self.current_func_named_count;
                                let gp_offset = if named_count < 6 { named_count * 8 } else { 48 };
                                let overflow_offset = if named_count > 6 {
                                    16 + (named_count - 6) * 8
                                } else {
                                    16
                                };
                                self.emit(&format!("mov dword ptr [rax], {}", gp_offset));
                                self.emit("mov dword ptr [rax + 4], 48");
                                self.emit(&format!("lea rdx, [rbp + {}]", overflow_offset));
                                self.emit("mov [rax + 8], rdx");
                                self.emit(&format!("lea rdx, [rbp - {}]", va_offset));
                                self.emit("mov [rax + 16], rdx");
                            }
                            return;
                        }
                        "__builtin_va_end" => {
                            return;
                        }
                        "alloca" | "__builtin_alloca" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            self.emit("add rax, 15");
                            self.emit("and rax, -16");
                            self.emit("sub rsp, rax");
                            self.emit("mov rax, rsp");
                            return;
                        }
                        _ => {}
                    }
                }

                enum ArgLocation {
                    Reg1,
                    Reg2,
                    Stack(usize, usize),
                }

                let mut arg_locs = Vec::new();
                let mut reg_idx = 0;
                let mut stack_byte_offset = 0;

                for arg in args {
                    if arg.ty.is_struct() || arg.ty.is_union() {
                        let sz = arg.ty.size();
                        if sz <= 8 {
                            if reg_idx < 6 {
                                arg_locs.push(ArgLocation::Reg1);
                                reg_idx += 1;
                            } else {
                                arg_locs.push(ArgLocation::Stack(stack_byte_offset, 8));
                                stack_byte_offset += 8;
                            }
                        } else if sz <= 16 {
                            if reg_idx <= 4 {
                                arg_locs.push(ArgLocation::Reg2);
                                reg_idx += 2;
                            } else {
                                reg_idx = 6;
                                arg_locs.push(ArgLocation::Stack(stack_byte_offset, 16));
                                stack_byte_offset += 16;
                            }
                        } else {
                            arg_locs.push(ArgLocation::Stack(stack_byte_offset, sz));
                            stack_byte_offset += sz.div_ceil(8) * 8;
                        }
                    } else if reg_idx < 6 {
                        arg_locs.push(ArgLocation::Reg1);
                        reg_idx += 1;
                    } else {
                        arg_locs.push(ArgLocation::Stack(stack_byte_offset, 8));
                        stack_byte_offset += 8;
                    }
                }

                let total_stack_bytes = stack_byte_offset.div_ceil(16) * 16;
                if total_stack_bytes > 0 {
                    self.emit(&format!("sub rsp, {}", total_stack_bytes));
                    for (arg, loc) in args.iter().zip(arg_locs.iter()) {
                        if let ArgLocation::Stack(off, sz) = loc {
                            self.gen_expr(arg);
                            if arg.ty.is_struct() || arg.ty.is_union() {
                                if *sz <= 8 {
                                    self.emit("mov rax, [rax]");
                                    self.emit(&format!("mov [rsp + {}], rax", off));
                                } else if *sz <= 16 {
                                    self.emit("mov r10, [rax]");
                                    self.emit(&format!("mov [rsp + {}], r10", off));
                                    self.emit("mov r10, [rax + 8]");
                                    self.emit(&format!("mov [rsp + {}], r10", off + 8));
                                } else {
                                    for o in (0..*sz).step_by(8) {
                                        if o + 8 <= *sz {
                                            self.emit(&format!("mov r10, [rax + {}]", o));
                                            self.emit(&format!("mov [rsp + {}], r10", off + o));
                                        } else {
                                            let rem = *sz - o;
                                            for b in 0..rem {
                                                self.emit(&format!(
                                                    "mov r10b, byte ptr [rax + {}]",
                                                    o + b
                                                ));
                                                self.emit(&format!(
                                                    "mov byte ptr [rsp + {}], r10b",
                                                    off + o + b
                                                ));
                                            }
                                        }
                                    }
                                }
                            } else {
                                self.emit(&format!("mov [rsp + {}], rax", off));
                            }
                        }
                    }
                }

                let is_direct =
                    matches!(&callee.kind, TypedExprKind::GlobalVar(_)) && callee.ty.is_function();
                if !is_direct {
                    self.gen_expr(callee);
                    self.push();
                }

                let mut pushed_regs = 0;
                for (arg, loc) in args.iter().zip(arg_locs.iter()) {
                    match loc {
                        ArgLocation::Reg1 => {
                            self.gen_expr(arg);
                            if arg.ty.is_struct() || arg.ty.is_union() {
                                self.emit("mov rax, [rax]");
                            }
                            self.push();
                            pushed_regs += 1;
                        }
                        ArgLocation::Reg2 => {
                            self.gen_expr(arg);
                            self.emit("mov r10, [rax]"); // low 8
                            self.emit("mov r11, [rax + 8]"); // high 8
                            self.emit("push r10");
                            self.emit("push r11");
                            pushed_regs += 2;
                        }
                        ArgLocation::Stack(_, _) => {}
                    }
                }

                let arg_regs = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"];
                for i in (0..pushed_regs).rev() {
                    self.pop(arg_regs[i]);
                }

                self.emit("xor eax, eax"); // AL = 0 (no vector registers for variadic calls)
                if is_direct {
                    if let TypedExprKind::GlobalVar(name) = &callee.kind {
                        let sym = self.symbol_name(name);
                        self.emit(&format!("call {}", sym));
                    }
                } else {
                    self.pop("r10");
                    self.emit("call r10");
                }

                if expr.ty.is_struct() || expr.ty.is_union() {
                    let sz = expr.ty.size();
                    let scratch_offset = self.current_func_scratch_offset;
                    if sz <= 8 {
                        self.emit_store_local("rax", scratch_offset as i32, 8);
                        self.emit(&format!("lea rax, [rbp - {}]", scratch_offset));
                    } else if sz <= 16 {
                        self.emit_store_local("rax", scratch_offset as i32, 8);
                        self.emit_store_local("rdx", (scratch_offset - 8) as i32, 8);
                        self.emit(&format!("lea rax, [rbp - {}]", scratch_offset));
                    }
                }

                if total_stack_bytes > 0 {
                    self.emit(&format!("add rsp, {}", total_stack_bytes));
                }
            }
            TypedExprKind::StmtExpr(stmts) => {
                for stmt in stmts {
                    self.gen_stmt(stmt);
                }
            }
        }
    }

    pub fn gen_stmt(&mut self, stmt: &TypedStmt) {
        match stmt {
            TypedStmt::Expr(expr) => {
                self.gen_expr(expr);
            }
            TypedStmt::Return(expr_opt, _) => {
                if let Some(expr) = expr_opt {
                    self.gen_expr(expr);
                    if expr.ty.is_struct() || expr.ty.is_union() {
                        let sz = expr.ty.size();
                        if sz <= 8 {
                            self.emit("mov rax, [rax]");
                        } else if sz <= 16 {
                            self.emit("mov rdx, [rax + 8]");
                            self.emit("mov rax, [rax]");
                        }
                    }
                }
                self.emit(&format!("jmp .L.return.{}", self.current_func));
            }
            TypedStmt::Block(stmts, _) => {
                for s in stmts {
                    self.gen_stmt(s);
                }
            }
            TypedStmt::If {
                cond,
                then_stmt,
                else_stmt,
                ..
            } => {
                let else_label = self.new_label("if_else");
                let end_label = self.new_label("if_end");

                self.gen_expr(cond);
                self.emit("test rax, rax");

                if let Some(else_s) = else_stmt {
                    self.emit(&format!("jz {}", else_label));
                    self.gen_stmt(then_stmt);
                    self.emit(&format!("jmp {}", end_label));
                    self.emit_label(&else_label);
                    self.gen_stmt(else_s);
                } else {
                    self.emit(&format!("jz {}", end_label));
                    self.gen_stmt(then_stmt);
                }

                self.emit_label(&end_label);
            }
            TypedStmt::While { cond, body, .. } => {
                let loop_label = self.new_label("while_loop");
                let break_label = self.new_label("while_break");

                self.loop_labels
                    .push((loop_label.clone(), break_label.clone()));

                self.emit_label(&loop_label);
                self.gen_expr(cond);
                self.emit("test rax, rax");
                self.emit(&format!("jz {}", break_label));

                self.gen_stmt(body);
                self.emit(&format!("jmp {}", loop_label));

                self.emit_label(&break_label);
                self.loop_labels.pop();
            }
            TypedStmt::DoWhile { body, cond, .. } => {
                let loop_label = self.new_label("dowhile_loop");
                let cond_label = self.new_label("dowhile_cond");
                let break_label = self.new_label("dowhile_break");

                self.loop_labels
                    .push((cond_label.clone(), break_label.clone()));

                self.emit_label(&loop_label);
                self.gen_stmt(body);

                self.emit_label(&cond_label);
                self.gen_expr(cond);
                self.emit("test rax, rax");
                self.emit(&format!("jnz {}", loop_label));

                self.emit_label(&break_label);
                self.loop_labels.pop();
            }
            TypedStmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => {
                let loop_label = self.new_label("for_loop");
                let step_label = self.new_label("for_step");
                let break_label = self.new_label("for_break");

                if let Some(init_s) = init {
                    self.gen_stmt(init_s);
                }

                self.loop_labels
                    .push((step_label.clone(), break_label.clone()));

                self.emit_label(&loop_label);
                if let Some(c) = cond {
                    self.gen_expr(c);
                    self.emit("test rax, rax");
                    self.emit(&format!("jz {}", break_label));
                }

                self.gen_stmt(body);

                self.emit_label(&step_label);
                if let Some(s) = step {
                    self.gen_expr(s);
                }
                self.emit(&format!("jmp {}", loop_label));

                self.emit_label(&break_label);
                self.loop_labels.pop();
            }
            TypedStmt::Switch {
                expr,
                body,
                cases,
                default_label,
                break_label,
                ..
            } => {
                self.gen_expr(expr);
                let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();

                for (val, label) in cases {
                    if is_32bit {
                        let imm = *val as i32;
                        self.emit(&format!("cmp eax, {}", imm));
                    } else if *val >= i32::MIN as i64 && *val <= i32::MAX as i64 {
                        self.emit(&format!("cmp rax, {}", val));
                    } else {
                        self.emit_load_imm("rcx", *val);
                        self.emit("cmp rax, rcx");
                    }
                    self.emit(&format!("je {}", label));
                }

                if let Some(def_lbl) = default_label {
                    self.emit(&format!("jmp {}", def_lbl));
                } else {
                    self.emit(&format!("jmp {}", break_label));
                }

                self.loop_labels.push((String::new(), break_label.clone()));
                self.gen_stmt(body);
                self.loop_labels.pop();

                self.emit_label(break_label);
            }
            TypedStmt::Case { label, body, .. } => {
                self.emit_label(label);
                self.gen_stmt(body);
            }
            TypedStmt::Default { label, body, .. } => {
                self.emit_label(label);
                self.gen_stmt(body);
            }
            TypedStmt::Break(_) => {
                if let Some((_, break_lbl)) = self.loop_labels.last() {
                    self.emit(&format!("jmp {}", break_lbl));
                }
            }
            TypedStmt::Continue(_) => {
                if let Some((cont_lbl, _)) =
                    self.loop_labels.iter().rev().find(|(c, _)| !c.is_empty())
                {
                    self.emit(&format!("jmp {}", cont_lbl));
                }
            }
            TypedStmt::Goto(lbl, _) => {
                self.emit(&format!("jmp .L.user.{}", lbl));
            }
            TypedStmt::GotoExpr(expr, _) => {
                self.gen_expr(expr);
                self.emit("jmp rax");
            }
            TypedStmt::Label(lbl, body, _) => {
                self.emit_label(&format!(".L.user.{}", lbl));
                self.gen_stmt(body);
            }
            TypedStmt::Empty(_) => {}
        }
    }

    fn emit_function(&mut self, func: &TypedFunction) {
        if func.body.is_none() {
            return;
        }

        self.current_func = func.name.clone();
        self.current_func_named_count = func.params.len();
        let sym = self.symbol_name(&func.name);

        if !func.is_static {
            writeln!(self.output, ".globl {}", sym).unwrap();
        }
        writeln!(self.output, ".p2align 4").unwrap();
        self.emit_label(&sym);

        // Prologue
        self.emit("push rbp");
        self.emit("mov rbp, rsp");

        let raw_stack_size = func.stack_size.max(16);
        let scratch_offset = raw_stack_size.div_ceil(16) * 16 + 16;
        let va_offset = scratch_offset + 192;
        let stack_size = (va_offset + 32).div_ceil(16) * 16;
        self.current_func_scratch_offset = scratch_offset;
        self.current_func_va_offset = va_offset;

        self.emit(&format!("sub rsp, {}", stack_size));

        if func.is_variadic {
            self.emit(&format!("mov [rbp - {}], rdi", va_offset));
            self.emit(&format!("mov [rbp - {}], rsi", va_offset - 8));
            self.emit(&format!("mov [rbp - {}], rdx", va_offset - 16));
            self.emit(&format!("mov [rbp - {}], rcx", va_offset - 24));
            self.emit(&format!("mov [rbp - {}], r8", va_offset - 32));
            self.emit(&format!("mov [rbp - {}], r9", va_offset - 40));
            self.emit(&format!("movups [rbp - {}], xmm0", va_offset - 48));
            self.emit(&format!("movups [rbp - {}], xmm1", va_offset - 64));
            self.emit(&format!("movups [rbp - {}], xmm2", va_offset - 80));
            self.emit(&format!("movups [rbp - {}], xmm3", va_offset - 96));
            self.emit(&format!("movups [rbp - {}], xmm4", va_offset - 112));
            self.emit(&format!("movups [rbp - {}], xmm5", va_offset - 128));
            self.emit(&format!("movups [rbp - {}], xmm6", va_offset - 144));
            self.emit(&format!("movups [rbp - {}], xmm7", va_offset - 160));
        }

        // Save incoming argument registers into their stack slots
        let arg_regs = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"];
        let mut reg_idx = 0;
        let mut stack_arg_offset = 16;

        for param in &func.params {
            let offset = param.offset;
            if param.ty.is_struct() || param.ty.is_union() {
                let sz = param.ty.size();
                if sz <= 8 {
                    if reg_idx < 6 {
                        self.emit_store_local(arg_regs[reg_idx], offset, sz);
                        reg_idx += 1;
                    } else {
                        self.emit(&format!("mov r10, [rbp + {}]", stack_arg_offset));
                        self.emit_store_local("r10", offset, sz);
                        stack_arg_offset += 8;
                    }
                } else if sz <= 16 {
                    if reg_idx <= 4 {
                        self.emit_store_local(arg_regs[reg_idx], offset, 8);
                        self.emit_store_local(arg_regs[reg_idx + 1], offset - 8, 8);
                        reg_idx += 2;
                    } else {
                        reg_idx = 6;
                        self.emit(&format!("mov r10, [rbp + {}]", stack_arg_offset));
                        self.emit_store_local("r10", offset, 8);
                        self.emit(&format!("mov r10, [rbp + {}]", stack_arg_offset + 8));
                        self.emit_store_local("r10", offset - 8, 8);
                        stack_arg_offset += 16;
                    }
                } else {
                    for o in (0..sz).step_by(8) {
                        if o + 8 <= sz {
                            self.emit(&format!("mov r10, [rbp + {}]", stack_arg_offset + o));
                            self.emit(&format!("mov [rbp - {}], r10", offset as usize - o));
                        } else {
                            let rem = sz - o;
                            for b in 0..rem {
                                self.emit(&format!(
                                    "mov r10b, byte ptr [rbp + {}]",
                                    stack_arg_offset + o + b
                                ));
                                self.emit(&format!(
                                    "mov byte ptr [rbp - {}], r10b",
                                    offset as usize - o - b
                                ));
                            }
                        }
                    }
                    stack_arg_offset += sz.div_ceil(8) * 8;
                }
            } else {
                let sz = param.ty.size();
                if reg_idx < 6 {
                    self.emit_store_local(arg_regs[reg_idx], offset, sz);
                    reg_idx += 1;
                } else {
                    self.emit(&format!("mov r10, [rbp + {}]", stack_arg_offset));
                    self.emit_store_local("r10", offset, sz);
                    stack_arg_offset += 8;
                }
            }
        }

        // Body
        if let Some(body) = &func.body {
            for stmt in body {
                self.gen_stmt(stmt);
            }
        }

        // Epilogue
        self.emit_label(&format!(".L.return.{}", func.name));
        self.emit("mov rsp, rbp");
        self.emit("pop rbp");
        self.emit("ret");
        writeln!(self.output).unwrap();
    }

    fn emit_typed_initializer(
        &mut self,
        ty: &Type,
        init: &Initializer,
        emitted: &mut usize,
        extra_strings: &mut Vec<StringLiteral>,
        globals: &HashMap<String, &Type>,
    ) {
        match init {
            Initializer::Single(expr) => {
                if let TypeKind::Array { .. } = &ty.kind
                    && let Expr::String(s, _) = &**expr
                {
                    let bytes = s.as_bytes();
                    let max_len = ty.size();
                    let copy_len = bytes.len().min(max_len);
                    let has_null = copy_len < max_len;

                    let mut str_bytes = Vec::new();
                    str_bytes.extend_from_slice(&bytes[..copy_len]);
                    if has_null {
                        str_bytes.push(0);
                    }
                    for b in &str_bytes {
                        self.emit(&format!(".byte {}", b));
                    }
                    *emitted += str_bytes.len();
                    if *emitted < max_len {
                        let pad = max_len - *emitted;
                        self.emit(&format!(".zero {}", pad));
                        *emitted += pad;
                    }
                    return;
                }

                if ty.is_pointer() {
                    if let Expr::String(s, _) = &**expr {
                        let label = self.new_label("str_init");
                        extra_strings.push(StringLiteral {
                            label: label.clone(),
                            content: s.clone(),
                        });
                        self.emit(&format!(".quad {}", label));
                        *emitted += 8;
                        return;
                    }
                    if let Some((sym, off)) = eval_symbol_addr(expr, globals) {
                        let s_name = self.symbol_name(&sym);
                        if off == 0 {
                            self.emit(&format!(".quad {}", s_name));
                        } else if off > 0 {
                            self.emit(&format!(".quad {} + {}", s_name, off));
                        } else {
                            self.emit(&format!(".quad {} - {}", s_name, -off));
                        }
                        *emitted += 8;
                        return;
                    }
                }

                let val = eval_const_expr(expr, globals).unwrap_or(0);
                match ty.size() {
                    1 => self.emit(&format!(".byte {}", val as u8)),
                    2 => self.emit(&format!(".short {}", val as u16)),
                    4 => self.emit(&format!(".long {}", val as u32)),
                    8 => self.emit(&format!(".quad {}", val)),
                    sz => self.emit(&format!(".zero {}", sz)),
                }
                *emitted += ty.size();
            }
            Initializer::List(list) => match &ty.kind {
                TypeKind::Struct(s_cell) => {
                    let s = s_cell.borrow();
                    let mut member_idx = 0;
                    let start_emitted = *emitted;
                    for item in list {
                        if let Some(Designator::Field(designator)) = item.designators.first() {
                            if let Some(m) = s.members.iter().find(|m| &m.name == designator) {
                                let target_offset = start_emitted + m.offset;
                                if *emitted < target_offset {
                                    self.emit(&format!(".zero {}", target_offset - *emitted));
                                    *emitted = target_offset;
                                }
                                self.emit_typed_initializer(
                                    &m.ty,
                                    &item.init,
                                    emitted,
                                    extra_strings,
                                    globals,
                                );
                            }
                        } else if member_idx < s.members.len() {
                            let m = &s.members[member_idx];
                            let target_offset = start_emitted + m.offset;
                            if *emitted < target_offset {
                                self.emit(&format!(".zero {}", target_offset - *emitted));
                                *emitted = target_offset;
                            }
                            self.emit_typed_initializer(
                                &m.ty,
                                &item.init,
                                emitted,
                                extra_strings,
                                globals,
                            );
                            member_idx += 1;
                        }
                    }
                    let end_target = start_emitted + s.size;
                    if *emitted < end_target {
                        self.emit(&format!(".zero {}", end_target - *emitted));
                        *emitted = end_target;
                    }
                }
                TypeKind::Union(u_cell) => {
                    let u = u_cell.borrow();
                    let start_emitted = *emitted;
                    if let Some(first_item) = list.first() {
                        let mut target_member = None;
                        for d in &first_item.designators {
                            if let Designator::Field(fname) = d
                                && let Some(m) = u.members.iter().find(|mem| &mem.name == fname)
                            {
                                target_member = Some(m);
                            }
                        }
                        if target_member.is_none() {
                            target_member = u.members.first();
                        }
                        if let Some(member) = target_member {
                            self.emit_typed_initializer(
                                &member.ty,
                                &first_item.init,
                                emitted,
                                extra_strings,
                                globals,
                            );
                        }
                    }
                    let end_target = start_emitted + u.size;
                    if *emitted < end_target {
                        self.emit(&format!(".zero {}", end_target - *emitted));
                        *emitted = end_target;
                    }
                }
                TypeKind::Array { elem, len } => {
                    let start_emitted = *emitted;
                    for item in list {
                        self.emit_typed_initializer(
                            elem,
                            &item.init,
                            emitted,
                            extra_strings,
                            globals,
                        );
                    }
                    if let Some(arr_len) = len {
                        let total_size = elem.size() * arr_len;
                        let end_target = start_emitted + total_size;
                        if *emitted < end_target {
                            self.emit(&format!(".zero {}", end_target - *emitted));
                            *emitted = end_target;
                        }
                    }
                }
                _ => {
                    if let Some(first) = list.first() {
                        self.emit_typed_initializer(
                            ty,
                            &first.init,
                            emitted,
                            extra_strings,
                            globals,
                        );
                    }
                }
            },
        }
    }
}

impl TargetEmitter for X86_64Emitter {
    fn emit_program(&mut self, prog: &TypedProgram) -> String {
        self.output.clear();
        writeln!(self.output, ".intel_syntax noprefix").unwrap();

        self.defined_globals = prog
            .globals
            .iter()
            .filter(|g| !g.is_extern)
            .map(|g| g.name.clone())
            .collect();
        let mut extra_strings = Vec::new();

        // Data segment for string literals
        if !prog.string_literals.is_empty() {
            if self.is_macos {
                writeln!(self.output, ".section __TEXT,__cstring,cstring_literals").unwrap();
            } else {
                writeln!(self.output, ".section .rodata").unwrap();
            }

            for s in &prog.string_literals {
                self.emit_label(&s.label);
                let escaped = escape_string_for_asm(&s.content);
                self.emit(&format!(".asciz \"{}\"", escaped));
            }
            writeln!(self.output).unwrap();
        }

        // Global variables
        if !prog.globals.is_empty() {
            let mut unique_globals: HashMap<&str, &GlobalVarInfo> = HashMap::new();
            for g in &prog.globals {
                if g.is_extern {
                    continue;
                }
                if let Some(existing) = unique_globals.get_mut(g.name.as_str()) {
                    if existing.init.is_none() && g.init.is_some() {
                        *existing = g;
                    }
                } else {
                    unique_globals.insert(&g.name, g);
                }
            }

            if !unique_globals.is_empty() {
                writeln!(self.output, ".data").unwrap();
                let mut emitted_names = HashSet::new();

                for orig_g in &prog.globals {
                    if let Some(g) = unique_globals.get(orig_g.name.as_str()) {
                        if !emitted_names.insert(&g.name) {
                            continue;
                        }

                        let sym = self.symbol_name(&g.name);
                        if !g.is_static {
                            writeln!(self.output, ".globl {}", sym).unwrap();
                        }
                        writeln!(self.output, ".p2align {}", g.ty.align().trailing_zeros())
                            .unwrap();
                        self.emit_label(&sym);

                        let size = g.ty.size();
                        if let Some(init) = &g.init {
                            let mut emitted = 0;
                            let global_types: HashMap<String, &Type> = prog
                                .globals
                                .iter()
                                .map(|g| (g.name.clone(), &g.ty))
                                .collect();
                            self.emit_typed_initializer(
                                &g.ty,
                                init,
                                &mut emitted,
                                &mut extra_strings,
                                &global_types,
                            );
                            if emitted < size {
                                self.emit(&format!(".zero {}", size - emitted));
                            }
                        } else {
                            self.emit(&format!(".zero {}", size));
                        }
                    }
                }
                writeln!(self.output).unwrap();
            }
        }

        // Emit any extra string literals generated by initializers
        if !extra_strings.is_empty() {
            if self.is_macos {
                writeln!(self.output, ".section __TEXT,__cstring,cstring_literals").unwrap();
            } else {
                writeln!(self.output, ".section .rodata").unwrap();
            }
            for s in &extra_strings {
                self.emit_label(&s.label);
                let escaped = escape_string_for_asm(&s.content);
                self.emit(&format!(".asciz \"{}\"", escaped));
            }
            writeln!(self.output).unwrap();
        }

        // Text segment
        writeln!(self.output, ".text").unwrap();
        for func in &prog.functions {
            self.emit_function(func);
        }

        self.output.clone()
    }
}

fn eval_sizeof_expr(expr: &Expr, globals: &HashMap<String, &Type>) -> Option<i64> {
    match expr {
        Expr::Var(name, _) => globals.get(name).map(|ty| ty.size() as i64),
        Expr::Index { expr: arr_expr, .. } => {
            if let Expr::Var(name, _) = &**arr_expr
                && let Some(ty) = globals.get(name)
                && let Some(elem) = ty.get_pointer_base()
            {
                return Some(elem.size() as i64);
            }
            None
        }
        Expr::Unary {
            op: UnaryOp::Deref,
            expr: ptr_expr,
            ..
        } => {
            if let Expr::Var(name, _) = &**ptr_expr
                && let Some(ty) = globals.get(name)
                && let Some(elem) = ty.get_pointer_base()
            {
                return Some(elem.size() as i64);
            }
            None
        }
        Expr::Member {
            expr: struct_expr,
            member,
            is_arrow,
            ..
        } => {
            if let Expr::Var(name, _) = &**struct_expr
                && let Some(ty) = globals.get(name)
            {
                let st_ty = if *is_arrow {
                    ty.get_pointer_base()
                } else {
                    Some(*ty)
                };
                if let Some(st) = st_ty {
                    if let TypeKind::Struct(s_cell) = &st.kind {
                        if let Some(m) = s_cell.borrow().members.iter().find(|m| &m.name == member)
                        {
                            return Some(m.ty.size() as i64);
                        }
                    } else if let TypeKind::Union(u_cell) = &st.kind
                        && let Some(m) = u_cell.borrow().members.iter().find(|m| &m.name == member)
                    {
                        return Some(m.ty.size() as i64);
                    }
                }
            }
            None
        }
        Expr::SizeofType { target_type, .. } => Some(target_type.size() as i64),
        _ => None,
    }
}

fn eval_const_expr(expr: &Expr, globals: &HashMap<String, &Type>) -> Option<i64> {
    match expr {
        Expr::Int(v, _) => Some(*v),
        Expr::Float(f, _) => Some(f.to_bits() as i64),
        Expr::Char(c, _) => Some(*c as i64),
        Expr::Unary { op, expr, .. } => match op {
            UnaryOp::Pos => eval_const_expr(expr, globals),
            UnaryOp::Neg => eval_const_expr(expr, globals).map(|v| -v),
            UnaryOp::BitNot => eval_const_expr(expr, globals).map(|v| !v),
            UnaryOp::LogNot => eval_const_expr(expr, globals).map(|v| if v == 0 { 1 } else { 0 }),
            UnaryOp::Sizeof => eval_sizeof_expr(expr, globals),
            UnaryOp::Alignof => match &**expr {
                Expr::Var(name, _) => globals.get(name).map(|ty| ty.align() as i64),
                Expr::SizeofType { target_type, .. } => Some(target_type.align() as i64),
                _ => None,
            },
            _ => None,
        },
        Expr::Binary { op, lhs, rhs, .. } => {
            let l = eval_const_expr(lhs, globals)?;
            let r = eval_const_expr(rhs, globals)?;
            match op {
                BinaryOp::Add => Some(l.wrapping_add(r)),
                BinaryOp::Sub => Some(l.wrapping_sub(r)),
                BinaryOp::Mul => Some(l.wrapping_mul(r)),
                BinaryOp::Div => {
                    if r != 0 {
                        Some(l / r)
                    } else {
                        None
                    }
                }
                BinaryOp::Rem => {
                    if r != 0 {
                        Some(l % r)
                    } else {
                        None
                    }
                }
                BinaryOp::BitAnd => Some(l & r),
                BinaryOp::BitOr => Some(l | r),
                BinaryOp::BitXor => Some(l ^ r),
                BinaryOp::Shl => Some(l << (r as u32)),
                BinaryOp::Shr => Some(l >> (r as u32)),
                BinaryOp::Eq => Some(if l == r { 1 } else { 0 }),
                BinaryOp::Ne => Some(if l != r { 1 } else { 0 }),
                BinaryOp::Lt => Some(if l < r { 1 } else { 0 }),
                BinaryOp::Le => Some(if l <= r { 1 } else { 0 }),
                BinaryOp::Gt => Some(if l > r { 1 } else { 0 }),
                BinaryOp::Ge => Some(if l >= r { 1 } else { 0 }),
                BinaryOp::LogicalAnd => Some(if l != 0 && r != 0 { 1 } else { 0 }),
                BinaryOp::LogicalOr => Some(if l != 0 || r != 0 { 1 } else { 0 }),
                _ => None,
            }
        }
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            let c = eval_const_expr(cond, globals)?;
            if c != 0 {
                eval_const_expr(then_expr, globals)
            } else {
                eval_const_expr(else_expr, globals)
            }
        }
        Expr::Cast { expr, .. } => eval_const_expr(expr, globals),
        Expr::SizeofType { target_type, .. } => Some(target_type.size() as i64),
        _ => None,
    }
}

fn eval_symbol_addr(expr: &Expr, globals: &HashMap<String, &Type>) -> Option<(String, i64)> {
    match expr {
        Expr::Var(name, _) => Some((name.clone(), 0)),
        Expr::Unary {
            op: UnaryOp::AddrOf,
            expr,
            ..
        } => eval_symbol_addr(expr, globals),
        Expr::Unary {
            op: UnaryOp::AddrOfLabel,
            expr,
            ..
        } => {
            if let Expr::Var(lbl, _) = &**expr {
                if lbl.starts_with('.') {
                    Some((lbl.clone(), 0))
                } else {
                    Some((format!(".L.user.{}", lbl), 0))
                }
            } else {
                None
            }
        }
        Expr::Unary {
            op: UnaryOp::Deref,
            expr,
            ..
        } => eval_symbol_addr(expr, globals),
        Expr::Cast { expr, .. } => eval_symbol_addr(expr, globals),
        Expr::Index {
            expr: base_expr,
            index,
            ..
        } => {
            if let Some((sym, base_off)) = eval_symbol_addr(base_expr, globals) {
                let idx_val = eval_const_expr(index, globals).unwrap_or(0);
                let elem_size = if let Some(ty) = globals.get(&sym) {
                    match &ty.kind {
                        TypeKind::Array { elem, .. } => elem.size(),
                        TypeKind::Pointer(elem) => elem.size(),
                        _ => 1,
                    }
                } else {
                    1
                };
                Some((sym, base_off + idx_val * (elem_size as i64)))
            } else {
                None
            }
        }
        Expr::Member {
            expr: base_expr,
            member,
            ..
        } => {
            if let Some((sym, base_off)) = eval_symbol_addr(base_expr, globals) {
                let member_off = if let Some(ty) = globals.get(&sym) {
                    match &ty.kind {
                        TypeKind::Struct(s_cell) => {
                            let s = s_cell.borrow();
                            s.members
                                .iter()
                                .find(|m| &m.name == member)
                                .map(|m| m.offset)
                                .unwrap_or(0)
                        }
                        _ => 0,
                    }
                } else {
                    0
                };
                Some((sym, base_off + (member_off as i64)))
            } else {
                None
            }
        }
        Expr::Binary {
            op: BinaryOp::Add,
            lhs,
            rhs,
            ..
        } => {
            if let Some((sym, off)) = eval_symbol_addr(lhs, globals) {
                let add_off = eval_const_expr(rhs, globals).unwrap_or(0);
                let elem_size = if let Some(ty) = globals.get(&sym) {
                    match &ty.kind {
                        TypeKind::Array { elem, .. } => elem.size(),
                        TypeKind::Pointer(elem) => elem.size(),
                        _ => 1,
                    }
                } else {
                    1
                };
                Some((sym, off + add_off * (elem_size as i64)))
            } else if let Some((sym, off)) = eval_symbol_addr(rhs, globals) {
                let add_off = eval_const_expr(lhs, globals).unwrap_or(0);
                let elem_size = if let Some(ty) = globals.get(&sym) {
                    match &ty.kind {
                        TypeKind::Array { elem, .. } => elem.size(),
                        TypeKind::Pointer(elem) => elem.size(),
                        _ => 1,
                    }
                } else {
                    1
                };
                Some((sym, off + add_off * (elem_size as i64)))
            } else {
                None
            }
        }
        Expr::Binary {
            op: BinaryOp::Sub,
            lhs,
            rhs,
            ..
        } => {
            if let Some((sym, off)) = eval_symbol_addr(lhs, globals) {
                let sub_off = eval_const_expr(rhs, globals).unwrap_or(0);
                let elem_size = if let Some(ty) = globals.get(&sym) {
                    match &ty.kind {
                        TypeKind::Array { elem, .. } => elem.size(),
                        TypeKind::Pointer(elem) => elem.size(),
                        _ => 1,
                    }
                } else {
                    1
                };
                Some((sym, off - sub_off * (elem_size as i64)))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn escape_string_for_asm(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if c.is_ascii_graphic() || c == ' ' => out.push(c),
            c => {
                for b in c.to_string().bytes() {
                    out.push_str(&format!("\\{:03o}", b));
                }
            }
        }
    }
    out
}
