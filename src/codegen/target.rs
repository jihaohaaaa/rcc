use crate::sema::TypedProgram;

pub trait TargetEmitter {
    fn emit_program(&mut self, prog: &TypedProgram) -> String;
}
