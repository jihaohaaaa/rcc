use crate::ast::*;
use crate::codegen::target::TargetEmitter;
use crate::sema::*;
use crate::types::*;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

pub struct AArch64Emitter {
    is_macos: bool,
    output: String,
    label_counter: usize,
    current_func: String,
    current_func_scratch_offset: usize,
    loop_labels: Vec<(String, String)>, // (continue_label, break_label)
    depth: Cell<usize>,
    defined_globals: HashSet<String>,
}

impl Default for AArch64Emitter {
    fn default() -> Self {
        Self::new()
    }
}

impl AArch64Emitter {
    pub fn new() -> Self {
        let is_macos = cfg!(target_os = "macos");
        Self {
            is_macos,
            output: String::new(),
            label_counter: 0,
            current_func: String::new(),
            current_func_scratch_offset: 0,
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
        self.emit("str x0, [sp, #-16]!");
    }

    fn pop(&mut self, reg: &str) {
        self.emit(&format!("ldr {}, [sp], #16", reg));
    }

    fn emit_load_imm(&mut self, reg: &str, val: i64) {
        let u = val as u64;
        if u == 0 {
            self.emit(&format!("mov {}, #0", reg));
            return;
        }
        let chunks = [
            (u & 0xffff) as u16,
            ((u >> 16) & 0xffff) as u16,
            ((u >> 32) & 0xffff) as u16,
            ((u >> 48) & 0xffff) as u16,
        ];
        let mut first = true;
        for (i, &chunk) in chunks.iter().enumerate() {
            if chunk != 0 {
                let shift = i * 16;
                if first {
                    if shift == 0 {
                        self.emit(&format!("movz {}, #{}", reg, chunk));
                    } else {
                        self.emit(&format!("movz {}, #{}, lsl #{}", reg, chunk, shift));
                    }
                    first = false;
                } else {
                    self.emit(&format!("movk {}, #{}, lsl #{}", reg, chunk, shift));
                }
            }
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
                if *offset <= 4095 {
                    self.emit(&format!("sub x0, fp, #{}", offset));
                } else {
                    self.emit_load_imm("x16", *offset as i64);
                    self.emit("sub x0, fp, x16");
                }
            }
            TypedExprKind::GlobalVar(name) => {
                let sym = self.symbol_name(name);
                if self.is_macos {
                    if self.defined_globals.contains(name) {
                        self.emit(&format!("adrp x0, {}@PAGE", sym));
                        self.emit(&format!("add x0, x0, {}@PAGEOFF", sym));
                    } else {
                        self.emit(&format!("adrp x0, {}@GOTPAGE", sym));
                        self.emit(&format!("ldr x0, [x0, {}@GOTPAGEOFF]", sym));
                    }
                } else {
                    self.emit(&format!("adrp x0, {}", sym));
                    self.emit(&format!("add x0, x0, :lo12:{}", sym));
                }
            }
            TypedExprKind::Deref(inner) => {
                self.gen_expr(inner);
            }
            TypedExprKind::Member { expr, offset, .. } => {
                self.gen_lval(expr);
                if *offset > 0 {
                    if *offset <= 4095 {
                        self.emit(&format!("add x0, x0, #{}", offset));
                    } else {
                        self.emit_load_imm("x16", *offset as i64);
                        self.emit("add x0, x0, x16");
                    }
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
            // Arrays, structs, and functions decay to pointer address in x0
            return;
        }
        match ty.size() {
            1 => {
                if ty.is_signed_integer() {
                    self.emit("ldrsb w0, [x0]");
                } else {
                    self.emit("ldrb w0, [x0]");
                }
            }
            2 => {
                if ty.is_signed_integer() {
                    self.emit("ldrsh w0, [x0]");
                } else {
                    self.emit("ldrh w0, [x0]");
                }
            }
            4 => {
                if ty.is_signed_integer() {
                    self.emit("ldrsw x0, [x0]");
                } else {
                    self.emit("ldr w0, [x0]");
                }
            }
            8 => {
                self.emit("ldr x0, [x0]");
            }
            _ => {
                self.emit("ldr x0, [x0]");
            }
        }
    }

    fn store(&mut self, ty: &Type) {
        if ty.is_struct() || ty.is_union() {
            let size = ty.size();
            if size <= 128 {
                let mut offset = 0;
                while offset + 8 <= size {
                    self.emit(&format!("ldr x2, [x0, #{}]", offset));
                    self.emit(&format!("str x2, [x1, #{}]", offset));
                    offset += 8;
                }
                if offset + 4 <= size {
                    self.emit(&format!("ldr w2, [x0, #{}]", offset));
                    self.emit(&format!("str w2, [x1, #{}]", offset));
                    offset += 4;
                }
                if offset + 2 <= size {
                    self.emit(&format!("ldrh w2, [x0, #{}]", offset));
                    self.emit(&format!("strh w2, [x1, #{}]", offset));
                    offset += 2;
                }
                if offset < size {
                    self.emit(&format!("ldrb w2, [x0, #{}]", offset));
                    self.emit(&format!("strb w2, [x1, #{}]", offset));
                }
            } else {
                let loop_lbl = self.new_label("struct_copy");
                self.emit_load_imm("x2", size as i64);
                self.emit_label(&loop_lbl);
                self.emit("ldrb w3, [x0], #1");
                self.emit("strb w3, [x1], #1");
                self.emit("subs x2, x2, #1");
                self.emit(&format!("b.ne {}", loop_lbl));
            }
            return;
        }
        match ty.size() {
            1 => self.emit("strb w0, [x1]"),
            2 => self.emit("strh w0, [x1]"),
            4 => self.emit("str w0, [x1]"),
            8 => self.emit("str x0, [x1]"),
            _ => self.emit("str x0, [x1]"),
        }
    }

    fn emit_store_local(&mut self, reg: &str, offset: i32, size: usize) {
        let r_name = reg.trim_start_matches('x');
        if offset <= 256 {
            match size {
                1 => self.emit(&format!("strb w{}, [fp, #-{}]", r_name, offset)),
                2 => self.emit(&format!("strh w{}, [fp, #-{}]", r_name, offset)),
                4 => self.emit(&format!("str w{}, [fp, #-{}]", r_name, offset)),
                _ => self.emit(&format!("str x{}, [fp, #-{}]", r_name, offset)),
            }
        } else {
            self.emit_load_imm("x16", offset as i64);
            self.emit("sub x16, fp, x16");
            match size {
                1 => self.emit(&format!("strb w{}, [x16]", r_name)),
                2 => self.emit(&format!("strh w{}, [x16]", r_name)),
                4 => self.emit(&format!("str w{}, [x16]", r_name)),
                _ => self.emit(&format!("str x{}, [x16]", r_name)),
            }
        }
    }

    fn emit_store_pair_local(&mut self, r1: &str, r2: &str, offset: i32) {
        if (0..=504).contains(&offset) {
            self.emit(&format!("stp {}, {}, [fp, #-{}]", r1, r2, offset));
        } else {
            self.emit_load_imm("x18", offset as i64);
            self.emit("sub x18, fp, x18");
            self.emit(&format!("stp {}, {}, [x18]", r1, r2));
        }
    }

    fn store_bitfield(&mut self, ty: &Type, bw: usize, boff: usize) {
        if bw == 0 {
            return;
        }
        // Load existing word
        match ty.size() {
            1 => self.emit("ldrb w2, [x1]"),
            2 => self.emit("ldrh w2, [x1]"),
            4 => self.emit("ldr w2, [x1]"),
            8 => self.emit("ldr x2, [x1]"),
            _ => self.emit("ldr x2, [x1]"),
        }
        // Insert bw bits from x0 into x2 starting at boff
        if ty.size() == 8 {
            self.emit(&format!("bfi x2, x0, #{}, #{}", boff, bw));
            self.emit("str x2, [x1]");
        } else {
            self.emit(&format!("bfi w2, w0, #{}, #{}", boff, bw));
            match ty.size() {
                1 => self.emit("strb w2, [x1]"),
                2 => self.emit("strh w2, [x1]"),
                4 => self.emit("str w2, [x1]"),
                _ => self.emit("str w2, [x1]"),
            }
        }
        // Result of assignment in x0: extract/mask bitfield value
        if ty.is_signed_integer() {
            self.emit(&format!("sbfx x0, x0, #0, #{}", bw));
        } else {
            self.emit(&format!("ubfx x0, x0, #0, #{}", bw));
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
                if *v >= 0 && *v <= 65535 {
                    self.emit(&format!("mov x0, #{}", v));
                } else if *v < 0 && *v >= -65536 {
                    self.emit(&format!("movn x0, #{}", (!v) & 0xffff));
                } else {
                    self.emit_load_imm("x0", *v);
                }
            }
            TypedExprKind::Float(v) => {
                self.emit_load_imm("x0", v.to_bits() as i64);
            }
            TypedExprKind::Char(c) => {
                self.emit(&format!("mov x0, #{}", *c as u32));
            }
            TypedExprKind::StringLiteral(label) => {
                if self.is_macos {
                    self.emit(&format!("adrp x0, {}@PAGE", label));
                    self.emit(&format!("add x0, x0, {}@PAGEOFF", label));
                } else {
                    self.emit(&format!("adrp x0, {}", label));
                    self.emit(&format!("add x0, x0, :lo12:{}", label));
                }
            }
            TypedExprKind::AddrOfLabel(lbl) => {
                let sym = format!(".L.user.{}", lbl);
                if self.is_macos {
                    self.emit(&format!("adrp x0, {}@PAGE", sym));
                    self.emit(&format!("add x0, x0, {}@PAGEOFF", sym));
                } else {
                    self.emit(&format!("adrp x0, {}", sym));
                    self.emit(&format!("add x0, x0, :lo12:{}", sym));
                }
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
                    if expr.ty.is_signed_integer() {
                        self.emit(&format!("sbfx x0, x0, #{}, #{}", boff, bw));
                    } else {
                        self.emit(&format!("ubfx x0, x0, #{}, #{}", boff, bw));
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
                        self.emit("fmov d0, x0");
                        self.emit("fcmp d0, #0.0");
                        self.emit("cset x0, ne");
                    } else if matches!(inner.ty.kind, TypeKind::Float) {
                        self.emit("fmov s0, w0");
                        self.emit("fcmp s0, #0.0");
                        self.emit("cset x0, ne");
                    } else {
                        self.emit("cmp x0, #0");
                        self.emit("cset x0, ne");
                    }
                } else if matches!(expr.ty.kind, TypeKind::Double) {
                    if matches!(inner.ty.kind, TypeKind::Double) {
                        // no-op
                    } else if matches!(inner.ty.kind, TypeKind::Float) {
                        self.emit("fmov s0, w0");
                        self.emit("fcvt d0, s0");
                        self.emit("fmov x0, d0");
                    } else if inner.ty.is_integer() {
                        if inner.ty.size() == 8 {
                            if inner.ty.is_signed_integer() {
                                self.emit("scvtf d0, x0");
                            } else {
                                self.emit("ucvtf d0, x0");
                            }
                        } else if inner.ty.is_signed_integer() {
                            self.emit("scvtf d0, w0");
                        } else {
                            self.emit("ucvtf d0, w0");
                        }
                        self.emit("fmov x0, d0");
                    }
                } else if matches!(expr.ty.kind, TypeKind::Float) {
                    if matches!(inner.ty.kind, TypeKind::Double) {
                        self.emit("fmov d0, x0");
                        self.emit("fcvt s0, d0");
                        self.emit("fmov w0, s0");
                    } else if matches!(inner.ty.kind, TypeKind::Float) {
                        // no-op
                    } else if inner.ty.is_integer() {
                        if inner.ty.size() == 8 {
                            if inner.ty.is_signed_integer() {
                                self.emit("scvtf s0, x0");
                            } else {
                                self.emit("ucvtf s0, x0");
                            }
                        } else if inner.ty.is_signed_integer() {
                            self.emit("scvtf s0, w0");
                        } else {
                            self.emit("ucvtf s0, w0");
                        }
                        self.emit("fmov w0, s0");
                    }
                } else if matches!(inner.ty.kind, TypeKind::Double) {
                    // Casting from Double to Integer
                    self.emit("fmov d0, x0");
                    match expr.ty.size() {
                        8 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("fcvtzs x0, d0");
                            } else {
                                self.emit("fcvtzu x0, d0");
                            }
                        }
                        4 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("fcvtzs w0, d0");
                                self.emit("sxtw x0, w0");
                            } else {
                                self.emit("fcvtzu w0, d0");
                                self.emit("uxtw x0, w0");
                            }
                        }
                        2 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("fcvtzs w0, d0");
                                self.emit("sxth x0, w0");
                            } else {
                                self.emit("fcvtzu w0, d0");
                                self.emit("uxth w0, w0");
                            }
                        }
                        1 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("fcvtzs w0, d0");
                                self.emit("sxtb x0, w0");
                            } else {
                                self.emit("fcvtzu w0, d0");
                                self.emit("uxtb w0, w0");
                            }
                        }
                        _ => {
                            self.emit("fcvtzs x0, d0");
                        }
                    }
                } else if matches!(inner.ty.kind, TypeKind::Float) {
                    // Casting from Float to Integer
                    self.emit("fmov s0, w0");
                    match expr.ty.size() {
                        8 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("fcvtzs x0, s0");
                            } else {
                                self.emit("fcvtzu x0, s0");
                            }
                        }
                        4 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("fcvtzs w0, s0");
                                self.emit("sxtw x0, w0");
                            } else {
                                self.emit("fcvtzu w0, s0");
                                self.emit("uxtw x0, w0");
                            }
                        }
                        2 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("fcvtzs w0, s0");
                                self.emit("sxth x0, w0");
                            } else {
                                self.emit("fcvtzu w0, s0");
                                self.emit("uxth w0, w0");
                            }
                        }
                        1 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("fcvtzs w0, s0");
                                self.emit("sxtb x0, w0");
                            } else {
                                self.emit("fcvtzu w0, s0");
                                self.emit("uxtb w0, w0");
                            }
                        }
                        _ => {
                            self.emit("fcvtzs x0, s0");
                        }
                    }
                } else {
                    // Integer to Integer cast
                    match expr.ty.size() {
                        1 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("sxtb x0, w0");
                            } else {
                                self.emit("uxtb w0, w0");
                            }
                        }
                        2 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("sxth x0, w0");
                            } else {
                                self.emit("uxth w0, w0");
                            }
                        }
                        4 => {
                            if expr.ty.is_signed_integer() {
                                self.emit("sxtw x0, w0");
                            } else {
                                self.emit("uxtw x0, w0");
                            }
                        }
                        8 if inner.ty.is_integer() && inner.ty.size() < 8 => {
                            if inner.ty.is_signed_integer() {
                                match inner.ty.size() {
                                    1 => self.emit("sxtb x0, w0"),
                                    2 => self.emit("sxth x0, w0"),
                                    4 => self.emit("sxtw x0, w0"),
                                    _ => {}
                                }
                            } else {
                                match inner.ty.size() {
                                    1 => self.emit("uxtb w0, w0"),
                                    2 => self.emit("uxth w0, w0"),
                                    4 => self.emit("uxtw x0, w0"),
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
                    self.pop("x1");
                    self.emit("mov x2, x0");
                    self.emit("mov x0, x1");
                    self.emit("mov x1, x2");
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
                    self.push();
                    self.gen_expr(lhs);
                    self.push();
                    self.gen_expr(rhs);
                    self.pop("x1");
                    match op {
                        BinaryOp::PlusAssign => {
                            if matches!(lhs.ty.kind, TypeKind::Double) {
                                self.emit("fmov d0, x0");
                                self.emit("fmov d1, x1");
                                self.emit("fadd d0, d1, d0");
                                self.emit("fmov x0, d0");
                            } else if matches!(lhs.ty.kind, TypeKind::Float) {
                                self.emit("fmov s0, w0");
                                self.emit("fmov s1, w1");
                                self.emit("fadd s0, s1, s0");
                                self.emit("fmov w0, s0");
                            } else {
                                self.emit("add x0, x1, x0");
                            }
                        }
                        BinaryOp::MinusAssign => {
                            if matches!(lhs.ty.kind, TypeKind::Double) {
                                self.emit("fmov d0, x0");
                                self.emit("fmov d1, x1");
                                self.emit("fsub d0, d1, d0");
                                self.emit("fmov x0, d0");
                            } else if matches!(lhs.ty.kind, TypeKind::Float) {
                                self.emit("fmov s0, w0");
                                self.emit("fmov s1, w1");
                                self.emit("fsub s0, s1, s0");
                                self.emit("fmov w0, s0");
                            } else {
                                self.emit("sub x0, x1, x0");
                            }
                        }
                        BinaryOp::StarAssign => {
                            if matches!(lhs.ty.kind, TypeKind::Double) {
                                self.emit("fmov d0, x0");
                                self.emit("fmov d1, x1");
                                self.emit("fmul d0, d1, d0");
                                self.emit("fmov x0, d0");
                            } else if matches!(lhs.ty.kind, TypeKind::Float) {
                                self.emit("fmov s0, w0");
                                self.emit("fmov s1, w1");
                                self.emit("fmul s0, s1, s0");
                                self.emit("fmov w0, s0");
                            } else {
                                self.emit("mul x0, x1, x0");
                            }
                        }
                        BinaryOp::SlashAssign => {
                            if matches!(lhs.ty.kind, TypeKind::Double) {
                                self.emit("fmov d0, x0");
                                self.emit("fmov d1, x1");
                                self.emit("fdiv d0, d1, d0");
                                self.emit("fmov x0, d0");
                            } else if matches!(lhs.ty.kind, TypeKind::Float) {
                                self.emit("fmov s0, w0");
                                self.emit("fmov s1, w1");
                                self.emit("fdiv s0, s1, s0");
                                self.emit("fmov w0, s0");
                            } else if lhs.ty.is_signed_integer() {
                                self.emit("sdiv x0, x1, x0");
                            } else {
                                self.emit("udiv x0, x1, x0");
                            }
                        }
                        BinaryOp::PercentAssign => {
                            if lhs.ty.is_signed_integer() {
                                self.emit("sdiv x2, x1, x0");
                            } else {
                                self.emit("udiv x2, x1, x0");
                            }
                            self.emit("msub x0, x2, x0, x1");
                        }
                        BinaryOp::AmpAssign => self.emit("and x0, x1, x0"),
                        BinaryOp::PipeAssign => self.emit("orr x0, x1, x0"),
                        BinaryOp::CaretAssign => self.emit("eor x0, x1, x0"),
                        BinaryOp::ShlAssign => self.emit("lsl x0, x1, x0"),
                        BinaryOp::ShrAssign => {
                            if lhs.ty.is_signed_integer() {
                                self.emit("asr x0, x1, x0");
                            } else {
                                self.emit("lsr x0, x1, x0");
                            }
                        }
                        _ => unreachable!(),
                    }
                    self.pop("x1");
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
                    self.emit("cmp x0, #0");
                    self.emit(&format!("b.eq {}", false_label));

                    self.gen_expr(rhs);
                    self.emit("cmp x0, #0");
                    self.emit(&format!("b.eq {}", false_label));

                    self.emit("mov x0, #1");
                    self.emit(&format!("b {}", end_label));

                    self.emit_label(&false_label);
                    self.emit("mov x0, #0");

                    self.emit_label(&end_label);
                }
                BinaryOp::LogicalOr => {
                    let true_label = self.new_label("lor_true");
                    let end_label = self.new_label("lor_end");

                    self.gen_expr(lhs);
                    self.emit("cmp x0, #0");
                    self.emit(&format!("b.ne {}", true_label));

                    self.gen_expr(rhs);
                    self.emit("cmp x0, #0");
                    self.emit(&format!("b.ne {}", true_label));

                    self.emit("mov x0, #0");
                    self.emit(&format!("b {}", end_label));

                    self.emit_label(&true_label);
                    self.emit("mov x0, #1");

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
                    self.pop("x1");

                    match op {
                        BinaryOp::Add => {
                            if matches!(expr.ty.kind, TypeKind::Double) {
                                self.emit("fmov d0, x0");
                                self.emit("fmov d1, x1");
                                self.emit("fadd d0, d1, d0");
                                self.emit("fmov x0, d0");
                            } else if matches!(expr.ty.kind, TypeKind::Float) {
                                self.emit("fmov s0, w0");
                                self.emit("fmov s1, w1");
                                self.emit("fadd s0, s1, s0");
                                self.emit("fmov w0, s0");
                            } else {
                                self.emit("add x0, x1, x0");
                            }
                        }
                        BinaryOp::Sub => {
                            if matches!(expr.ty.kind, TypeKind::Double) {
                                self.emit("fmov d0, x0");
                                self.emit("fmov d1, x1");
                                self.emit("fsub d0, d1, d0");
                                self.emit("fmov x0, d0");
                            } else if matches!(expr.ty.kind, TypeKind::Float) {
                                self.emit("fmov s0, w0");
                                self.emit("fmov s1, w1");
                                self.emit("fsub s0, s1, s0");
                                self.emit("fmov w0, s0");
                            } else {
                                self.emit("sub x0, x1, x0");
                            }
                        }
                        BinaryOp::Mul => {
                            if matches!(expr.ty.kind, TypeKind::Double) {
                                self.emit("fmov d0, x0");
                                self.emit("fmov d1, x1");
                                self.emit("fmul d0, d1, d0");
                                self.emit("fmov x0, d0");
                            } else if matches!(expr.ty.kind, TypeKind::Float) {
                                self.emit("fmov s0, w0");
                                self.emit("fmov s1, w1");
                                self.emit("fmul s0, s1, s0");
                                self.emit("fmov w0, s0");
                            } else {
                                let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                                if is_32bit {
                                    self.emit("mul w0, w1, w0");
                                    if expr.ty.is_signed_integer() {
                                        self.emit("sxtw x0, w0");
                                    } else {
                                        self.emit("uxtw x0, w0");
                                    }
                                } else {
                                    self.emit("mul x0, x1, x0");
                                }
                            }
                        }
                        BinaryOp::Div => {
                            if matches!(expr.ty.kind, TypeKind::Double) {
                                self.emit("fmov d0, x0");
                                self.emit("fmov d1, x1");
                                self.emit("fdiv d0, d1, d0");
                                self.emit("fmov x0, d0");
                            } else if matches!(expr.ty.kind, TypeKind::Float) {
                                self.emit("fmov s0, w0");
                                self.emit("fmov s1, w1");
                                self.emit("fdiv s0, s1, s0");
                                self.emit("fmov w0, s0");
                            } else {
                                let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                                if is_32bit {
                                    if expr.ty.is_signed_integer() {
                                        self.emit("sdiv w0, w1, w0");
                                        self.emit("sxtw x0, w0");
                                    } else {
                                        self.emit("udiv w0, w1, w0");
                                        self.emit("uxtw x0, w0");
                                    }
                                } else {
                                    if expr.ty.is_signed_integer() {
                                        self.emit("sdiv x0, x1, x0");
                                    } else {
                                        self.emit("udiv x0, x1, x0");
                                    }
                                }
                            }
                        }
                        BinaryOp::Rem => {
                            let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                            if is_32bit {
                                if expr.ty.is_signed_integer() {
                                    self.emit("sdiv w2, w1, w0");
                                    self.emit("msub w0, w2, w0, w1");
                                    self.emit("sxtw x0, w0");
                                } else {
                                    self.emit("udiv w2, w1, w0");
                                    self.emit("msub w0, w2, w0, w1");
                                    self.emit("uxtw x0, w0");
                                }
                            } else {
                                if expr.ty.is_signed_integer() {
                                    self.emit("sdiv x2, x1, x0");
                                    self.emit("msub x0, x2, x0, x1");
                                } else {
                                    self.emit("udiv x2, x1, x0");
                                    self.emit("msub x0, x2, x0, x1");
                                }
                            }
                        }
                        BinaryOp::BitAnd => {
                            let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                            if is_32bit {
                                self.emit("and w0, w1, w0");
                                self.emit("uxtw x0, w0");
                            } else {
                                self.emit("and x0, x1, x0");
                            }
                        }
                        BinaryOp::BitOr => {
                            let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                            if is_32bit {
                                self.emit("orr w0, w1, w0");
                                self.emit("uxtw x0, w0");
                            } else {
                                self.emit("orr x0, x1, x0");
                            }
                        }
                        BinaryOp::BitXor => {
                            let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                            if is_32bit {
                                self.emit("eor w0, w1, w0");
                                self.emit("uxtw x0, w0");
                            } else {
                                self.emit("eor x0, x1, x0");
                            }
                        }
                        BinaryOp::Shl => {
                            let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                            if is_32bit {
                                self.emit("lsl w0, w1, w0");
                                if expr.ty.is_signed_integer() {
                                    self.emit("sxtw x0, w0");
                                } else {
                                    self.emit("uxtw x0, w0");
                                }
                            } else {
                                self.emit("lsl x0, x1, x0");
                            }
                        }
                        BinaryOp::Shr => {
                            let is_32bit = expr.ty.size() <= 4 && !expr.ty.is_pointer();
                            if is_32bit {
                                if expr.ty.is_signed_integer() {
                                    self.emit("asr w0, w1, w0");
                                    self.emit("sxtw x0, w0");
                                } else {
                                    self.emit("lsr w0, w1, w0");
                                    self.emit("uxtw x0, w0");
                                }
                            } else {
                                if expr.ty.is_signed_integer() {
                                    self.emit("asr x0, x1, x0");
                                } else {
                                    self.emit("lsr x0, x1, x0");
                                }
                            }
                        }
                        BinaryOp::Eq => {
                            if matches!(lhs.ty.kind, TypeKind::Double)
                                || matches!(rhs.ty.kind, TypeKind::Double)
                            {
                                self.emit("fmov d0, x0");
                                self.emit("fmov d1, x1");
                                self.emit("fcmp d1, d0");
                            } else if matches!(lhs.ty.kind, TypeKind::Float)
                                || matches!(rhs.ty.kind, TypeKind::Float)
                            {
                                self.emit("fmov s0, w0");
                                self.emit("fmov s1, w1");
                                self.emit("fcmp s1, s0");
                            } else {
                                let is_32bit = lhs.ty.size() <= 4
                                    && rhs.ty.size() <= 4
                                    && !lhs.ty.is_pointer()
                                    && !rhs.ty.is_pointer();
                                if is_32bit {
                                    self.emit("cmp w1, w0");
                                } else {
                                    self.emit("cmp x1, x0");
                                }
                            }
                            self.emit("cset x0, eq");
                        }
                        BinaryOp::Ne => {
                            if matches!(lhs.ty.kind, TypeKind::Double)
                                || matches!(rhs.ty.kind, TypeKind::Double)
                            {
                                self.emit("fmov d0, x0");
                                self.emit("fmov d1, x1");
                                self.emit("fcmp d1, d0");
                            } else if matches!(lhs.ty.kind, TypeKind::Float)
                                || matches!(rhs.ty.kind, TypeKind::Float)
                            {
                                self.emit("fmov s0, w0");
                                self.emit("fmov s1, w1");
                                self.emit("fcmp s1, s0");
                            } else {
                                let is_32bit = lhs.ty.size() <= 4
                                    && rhs.ty.size() <= 4
                                    && !lhs.ty.is_pointer()
                                    && !rhs.ty.is_pointer();
                                if is_32bit {
                                    self.emit("cmp w1, w0");
                                } else {
                                    self.emit("cmp x1, x0");
                                }
                            }
                            self.emit("cset x0, ne");
                        }
                        BinaryOp::Lt => {
                            if matches!(lhs.ty.kind, TypeKind::Double)
                                || matches!(rhs.ty.kind, TypeKind::Double)
                            {
                                self.emit("fmov d0, x0");
                                self.emit("fmov d1, x1");
                                self.emit("fcmp d1, d0");
                                self.emit("cset x0, mi");
                            } else if matches!(lhs.ty.kind, TypeKind::Float)
                                || matches!(rhs.ty.kind, TypeKind::Float)
                            {
                                self.emit("fmov s0, w0");
                                self.emit("fmov s1, w1");
                                self.emit("fcmp s1, s0");
                                self.emit("cset x0, mi");
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
                                    self.emit("cmp w1, w0");
                                } else {
                                    self.emit("cmp x1, x0");
                                }
                                if is_signed {
                                    self.emit("cset x0, lt");
                                } else {
                                    self.emit("cset x0, lo");
                                }
                            }
                        }
                        BinaryOp::Le => {
                            if matches!(lhs.ty.kind, TypeKind::Double)
                                || matches!(rhs.ty.kind, TypeKind::Double)
                            {
                                self.emit("fmov d0, x0");
                                self.emit("fmov d1, x1");
                                self.emit("fcmp d1, d0");
                                self.emit("cset x0, ls");
                            } else if matches!(lhs.ty.kind, TypeKind::Float)
                                || matches!(rhs.ty.kind, TypeKind::Float)
                            {
                                self.emit("fmov s0, w0");
                                self.emit("fmov s1, w1");
                                self.emit("fcmp s1, s0");
                                self.emit("cset x0, ls");
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
                                    self.emit("cmp w1, w0");
                                } else {
                                    self.emit("cmp x1, x0");
                                }
                                if is_signed {
                                    self.emit("cset x0, le");
                                } else {
                                    self.emit("cset x0, ls");
                                }
                            }
                        }
                        BinaryOp::Gt => {
                            if matches!(lhs.ty.kind, TypeKind::Double)
                                || matches!(rhs.ty.kind, TypeKind::Double)
                            {
                                self.emit("fmov d0, x0");
                                self.emit("fmov d1, x1");
                                self.emit("fcmp d1, d0");
                                self.emit("cset x0, gt");
                            } else if matches!(lhs.ty.kind, TypeKind::Float)
                                || matches!(rhs.ty.kind, TypeKind::Float)
                            {
                                self.emit("fmov s0, w0");
                                self.emit("fmov s1, w1");
                                self.emit("fcmp s1, s0");
                                self.emit("cset x0, gt");
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
                                    self.emit("cmp w1, w0");
                                } else {
                                    self.emit("cmp x1, x0");
                                }
                                if is_signed {
                                    self.emit("cset x0, gt");
                                } else {
                                    self.emit("cset x0, hi");
                                }
                            }
                        }
                        BinaryOp::Ge => {
                            if matches!(lhs.ty.kind, TypeKind::Double)
                                || matches!(rhs.ty.kind, TypeKind::Double)
                            {
                                self.emit("fmov d0, x0");
                                self.emit("fmov d1, x1");
                                self.emit("fcmp d1, d0");
                                self.emit("cset x0, ge");
                            } else if matches!(lhs.ty.kind, TypeKind::Float)
                                || matches!(rhs.ty.kind, TypeKind::Float)
                            {
                                self.emit("fmov s0, w0");
                                self.emit("fmov s1, w1");
                                self.emit("fcmp s1, s0");
                                self.emit("cset x0, ge");
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
                                    self.emit("cmp w1, w0");
                                } else {
                                    self.emit("cmp x1, x0");
                                }
                                if is_signed {
                                    self.emit("cset x0, ge");
                                } else {
                                    self.emit("cset x0, hs");
                                }
                            }
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
                        self.emit("fmov d0, x0");
                        self.emit("fneg d0, d0");
                        self.emit("fmov x0, d0");
                    } else if matches!(inner.ty.kind, TypeKind::Float) {
                        self.emit("fmov s0, w0");
                        self.emit("fneg s0, s0");
                        self.emit("fmov w0, s0");
                    } else {
                        self.emit("neg x0, x0");
                    }
                }
                UnaryOp::BitNot => {
                    self.gen_expr(inner);
                    self.emit("mvn x0, x0");
                }
                UnaryOp::LogNot => {
                    self.gen_expr(inner);
                    self.emit("cmp x0, #0");
                    self.emit("cset x0, eq");
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
                    self.emit(&format!("add x0, x0, #{}", step));
                    self.pop("x1");
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
                    self.emit(&format!("sub x0, x0, #{}", step));
                    self.pop("x1");
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
                    self.push();
                    self.gen_expr(inner);
                    self.push();
                    self.emit(&format!("add x0, x0, #{}", step));
                    self.pop("x3"); // previous value
                    self.pop("x1"); // address
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
                    self.emit("mov x0, x3");
                }
                UnaryOp::PostDec => {
                    let step = if inner.ty.is_pointer() {
                        inner.ty.get_pointer_base().map_or(1, |b| b.size())
                    } else {
                        1
                    };
                    self.gen_lval(inner);
                    self.push();
                    self.gen_expr(inner);
                    self.push();
                    self.emit(&format!("sub x0, x0, #{}", step));
                    self.pop("x3"); // previous value
                    self.pop("x1"); // address
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
                    self.emit("mov x0, x3");
                }
                UnaryOp::Deref
                | UnaryOp::AddrOf
                | UnaryOp::Sizeof
                | UnaryOp::Alignof
                | UnaryOp::AddrOfLabel => {
                    // Handled in other paths
                }
            },
            TypedExprKind::Ternary {
                cond,
                then_expr,
                else_expr,
            } => {
                let else_label = self.new_label("ternary_else");
                let end_label = self.new_label("ternary_end");

                self.gen_expr(cond);
                self.emit("cmp x0, #0");
                self.emit(&format!("b.eq {}", else_label));

                self.gen_expr(then_expr);
                self.emit(&format!("b {}", end_label));

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
                                self.emit("mov x0, #0");
                            }
                            return;
                        }
                        "__builtin_clz" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            self.emit("clz w0, w0");
                            self.emit("uxtw x0, w0");
                            return;
                        }
                        "__builtin_clzll" | "__builtin_clzl" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            self.emit("clz x0, x0");
                            return;
                        }
                        "__builtin_ctz" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            self.emit("rbit w0, w0");
                            self.emit("clz w0, w0");
                            self.emit("uxtw x0, w0");
                            return;
                        }
                        "__builtin_ctzll" | "__builtin_ctzl" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            self.emit("rbit x0, x0");
                            self.emit("clz x0, x0");
                            return;
                        }
                        "__builtin_bswap16" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            self.emit("rev16 w0, w0");
                            self.emit("uxtw x0, w0");
                            return;
                        }
                        "__builtin_bswap32" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            self.emit("rev w0, w0");
                            self.emit("uxtw x0, w0");
                            return;
                        }
                        "__builtin_bswap64" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            self.emit("rev x0, x0");
                            return;
                        }
                        "__builtin_unreachable" => {
                            self.emit("brk #0");
                            return;
                        }
                        "__builtin_constant_p" => {
                            self.emit("mov x0, #0");
                            return;
                        }
                        "__builtin_frame_address" => {
                            self.emit("mov x0, fp");
                            return;
                        }
                        "__builtin_inff" | "__builtin_inf" | "__builtin_huge_val" => {
                            self.emit_load_imm("x0", 0x7ff0000000000000_u64 as i64);
                            return;
                        }
                        "__builtin_nanf" | "__builtin_nan" => {
                            self.emit_load_imm("x0", 0x7ff8000000000000_u64 as i64);
                            return;
                        }
                        "alloca" | "__builtin_alloca" => {
                            if !args.is_empty() {
                                self.gen_expr(&args[0]);
                            }
                            self.emit("add x0, x0, #15");
                            self.emit("bic x0, x0, #15");
                            self.emit("sub sp, sp, x0");
                            self.emit("mov x0, sp");
                            return;
                        }
                        _ => {}
                    }
                }

                let is_variadic_on_macos = self.is_macos
                    && match &callee.ty.kind {
                        TypeKind::Function { is_variadic, .. } => *is_variadic,
                        _ => false,
                    };

                let named_count = if is_variadic_on_macos {
                    match &callee.ty.kind {
                        TypeKind::Function { params, .. } => params.len(),
                        _ => 0,
                    }
                } else {
                    args.len()
                };

                enum ArgLocation {
                    Reg1,
                    Reg2,
                    Stack(usize, usize),
                }

                let mut arg_locs = Vec::new();
                let mut reg_idx = 0;
                let mut stack_byte_offset = 0;

                for (i, arg) in args.iter().enumerate() {
                    if is_variadic_on_macos && i >= named_count {
                        arg_locs.push(ArgLocation::Stack(stack_byte_offset, 8));
                        stack_byte_offset += 8;
                    } else if arg.ty.is_struct() || arg.ty.is_union() {
                        let sz = arg.ty.size();
                        if sz <= 8 {
                            if reg_idx < 8 {
                                arg_locs.push(ArgLocation::Reg1);
                                reg_idx += 1;
                            } else {
                                arg_locs.push(ArgLocation::Stack(stack_byte_offset, 8));
                                stack_byte_offset += 8;
                            }
                        } else if sz <= 16 {
                            if reg_idx <= 6 {
                                arg_locs.push(ArgLocation::Reg2);
                                reg_idx += 2;
                            } else {
                                reg_idx = 8;
                                arg_locs.push(ArgLocation::Stack(stack_byte_offset, 16));
                                stack_byte_offset += 16;
                            }
                        } else {
                            arg_locs.push(ArgLocation::Stack(stack_byte_offset, sz));
                            stack_byte_offset += sz.div_ceil(8) * 8;
                        }
                    } else {
                        if reg_idx < 8 {
                            arg_locs.push(ArgLocation::Reg1);
                            reg_idx += 1;
                        } else {
                            arg_locs.push(ArgLocation::Stack(stack_byte_offset, 8));
                            stack_byte_offset += 8;
                        }
                    }
                }

                let total_stack_bytes = stack_byte_offset.div_ceil(16) * 16;
                if total_stack_bytes > 0 {
                    if total_stack_bytes <= 4095 {
                        self.emit(&format!("sub sp, sp, #{}", total_stack_bytes));
                    } else {
                        self.emit_load_imm("x16", total_stack_bytes as i64);
                        self.emit("sub sp, sp, x16");
                    }
                    for (arg, loc) in args.iter().zip(arg_locs.iter()) {
                        if let ArgLocation::Stack(off, sz) = loc {
                            self.gen_expr(arg);
                            if arg.ty.is_struct() || arg.ty.is_union() {
                                if *sz <= 8 {
                                    self.emit("ldr x0, [x0]");
                                    self.emit(&format!("str x0, [sp, #{}]", off));
                                } else if *sz <= 16 {
                                    self.emit("ldp x1, x2, [x0]");
                                    self.emit(&format!("stp x1, x2, [sp, #{}]", off));
                                }
                            } else {
                                self.emit(&format!("str x0, [sp, #{}]", off));
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
                                self.emit("ldr x0, [x0]");
                            }
                            self.push();
                            pushed_regs += 1;
                        }
                        ArgLocation::Reg2 => {
                            self.gen_expr(arg);
                            self.emit("ldp x1, x2, [x0]");
                            self.emit("str x1, [sp, #-16]!"); // push low 8 bytes first
                            self.emit("str x2, [sp, #-16]!"); // push high 8 bytes second
                            pushed_regs += 2;
                        }
                        ArgLocation::Stack(_, _) => {}
                    }
                }

                for i in (0..pushed_regs).rev() {
                    self.pop(&format!("x{}", i));
                }

                if is_direct {
                    if let TypedExprKind::GlobalVar(name) = &callee.kind {
                        let sym = self.symbol_name(name);
                        self.emit(&format!("bl {}", sym));
                    }
                } else {
                    self.pop("x8");
                    self.emit("blr x8");
                }

                if expr.ty.is_struct() || expr.ty.is_union() {
                    let sz = expr.ty.size();
                    let scratch_offset = self.current_func_scratch_offset;
                    if sz <= 8 {
                        self.emit_store_local("x0", scratch_offset as i32, 8);
                        if scratch_offset <= 4095 {
                            self.emit(&format!("sub x0, fp, #{}", scratch_offset));
                        } else {
                            self.emit_load_imm("x16", scratch_offset as i64);
                            self.emit("sub x0, fp, x16");
                        }
                    } else if sz <= 16 {
                        self.emit_store_pair_local("x0", "x1", scratch_offset as i32);
                        if scratch_offset <= 4095 {
                            self.emit(&format!("sub x0, fp, #{}", scratch_offset));
                        } else {
                            self.emit_load_imm("x16", scratch_offset as i64);
                            self.emit("sub x0, fp, x16");
                        }
                    }
                }

                if total_stack_bytes > 0 {
                    if total_stack_bytes <= 4095 {
                        self.emit(&format!("add sp, sp, #{}", total_stack_bytes));
                    } else {
                        self.emit_load_imm("x16", total_stack_bytes as i64);
                        self.emit("add sp, sp, x16");
                    }
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
                            self.emit("ldr x0, [x0]");
                        } else if sz <= 16 {
                            self.emit("ldp x0, x1, [x0]");
                        }
                    }
                }
                self.emit(&format!("b .L.return.{}", self.current_func));
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
                self.emit("cmp x0, #0");

                if let Some(else_s) = else_stmt {
                    self.emit(&format!("b.eq {}", else_label));
                    self.gen_stmt(then_stmt);
                    self.emit(&format!("b {}", end_label));
                    self.emit_label(&else_label);
                    self.gen_stmt(else_s);
                } else {
                    self.emit(&format!("b.eq {}", end_label));
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
                self.emit("cmp x0, #0");
                self.emit(&format!("b.eq {}", break_label));

                self.gen_stmt(body);
                self.emit(&format!("b {}", loop_label));

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
                self.emit("cmp x0, #0");
                self.emit(&format!("b.ne {}", loop_label));

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
                    self.emit("cmp x0, #0");
                    self.emit(&format!("b.eq {}", break_label));
                }

                self.gen_stmt(body);

                self.emit_label(&step_label);
                if let Some(s) = step {
                    self.gen_expr(s);
                }
                self.emit(&format!("b {}", loop_label));

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
                        if (0..=4095).contains(&imm) {
                            self.emit(&format!("cmp w0, #{}", imm));
                        } else {
                            self.emit_load_imm("x1", *val);
                            self.emit("cmp w0, w1");
                        }
                    } else if *val >= 0 && *val <= 4095 {
                        self.emit(&format!("cmp x0, #{}", val));
                    } else {
                        self.emit_load_imm("x1", *val);
                        self.emit("cmp x0, x1");
                    }
                    self.emit(&format!("b.eq {}", label));
                }

                if let Some(def_lbl) = default_label {
                    self.emit(&format!("b {}", def_lbl));
                } else {
                    self.emit(&format!("b {}", break_label));
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
                    self.emit(&format!("b {}", break_lbl));
                }
            }
            TypedStmt::Continue(_) => {
                if let Some((cont_lbl, _)) =
                    self.loop_labels.iter().rev().find(|(c, _)| !c.is_empty())
                {
                    self.emit(&format!("b {}", cont_lbl));
                }
            }
            TypedStmt::Goto(lbl, _) => {
                self.emit(&format!("b .L.user.{}", lbl));
            }
            TypedStmt::GotoExpr(expr, _) => {
                self.gen_expr(expr);
                self.emit("br x0");
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
        let sym = self.symbol_name(&func.name);

        if !func.is_static {
            writeln!(self.output, ".globl {}", sym).unwrap();
        }
        writeln!(self.output, ".p2align 2").unwrap();
        self.emit_label(&sym);

        // Prologue
        self.emit("stp fp, lr, [sp, #-16]!");
        self.emit("mov fp, sp");

        let raw_stack_size = func.stack_size.max(16);
        let scratch_offset = raw_stack_size.div_ceil(16) * 16 + 16;
        let stack_size = scratch_offset + 16;
        self.current_func_scratch_offset = scratch_offset;

        if stack_size <= 4095 {
            self.emit(&format!("sub sp, sp, #{}", stack_size));
        } else {
            self.emit_load_imm("x16", stack_size as i64);
            self.emit("sub sp, sp, x16");
        }

        // Save incoming argument registers into their stack slots
        let mut reg_idx = 0;
        let mut stack_arg_offset = 16;
        for param in &func.params {
            let offset = param.offset;
            if param.ty.is_struct() || param.ty.is_union() {
                let sz = param.ty.size();
                if sz <= 8 {
                    if reg_idx < 8 {
                        self.emit_store_local(&format!("x{}", reg_idx), offset, sz);
                        reg_idx += 1;
                    } else {
                        self.emit(&format!("ldr x16, [fp, #{}]", stack_arg_offset));
                        self.emit_store_local("x16", offset, sz);
                        stack_arg_offset += 8;
                    }
                } else if sz <= 16 {
                    if reg_idx <= 6 {
                        self.emit_store_pair_local(
                            &format!("x{}", reg_idx),
                            &format!("x{}", reg_idx + 1),
                            offset,
                        );
                        reg_idx += 2;
                    } else {
                        reg_idx = 8;
                        self.emit(&format!("ldp x16, x17, [fp, #{}]", stack_arg_offset));
                        self.emit_store_pair_local("x16", "x17", offset);
                        stack_arg_offset += 16;
                    }
                } else {
                    self.emit(&format!("ldr x16, [fp, #{}]", stack_arg_offset));
                    self.emit_store_local("x16", offset, 8);
                    stack_arg_offset += sz.div_ceil(8) * 8;
                }
            } else {
                let sz = param.ty.size();
                if reg_idx < 8 {
                    self.emit_store_local(&format!("x{}", reg_idx), offset, sz);
                    reg_idx += 1;
                } else {
                    self.emit(&format!("ldr x16, [fp, #{}]", stack_arg_offset));
                    self.emit_store_local("x16", offset, sz);
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
        self.emit("mov sp, fp");
        self.emit("ldp fp, lr, [sp], #16");
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

impl TargetEmitter for AArch64Emitter {
    fn emit_program(&mut self, prog: &TypedProgram) -> String {
        self.output.clear();
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
            // Deduplicate globals: skip extern, prefer definitions with initializers
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
