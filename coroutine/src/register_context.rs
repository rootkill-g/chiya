#[derive(Debug)]
pub struct RegisterContext {
    /// Hold the registeres while the task or scheduler is suspended
    pub(crate) regs: Registers,
}

impl RegisterContext {
    pub fn empty() -> RegisterContext {
        RegisterContext {
            regs: Registers::new(),
        }
    }
}
