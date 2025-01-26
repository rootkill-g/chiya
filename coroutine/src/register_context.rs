use crate::stack::Register;

#[derive(Debug)]
pub struct RegisterContext {
    /// Hold the registeres while the task or scheduler is suspended
    pub(crate) regs: Register,
}

impl RegisterContext {
    pub fn empty() -> RegisterContext {
        RegisterContext {
            regs: Register::new(),
        }
    }
}
