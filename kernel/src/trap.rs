//! Platform independent trap handler functions

use process::*;
use arch::interrupt::TrapFrame;

/// Called in timer interrupt.
pub fn timer() {
    let mut processor = processor();
    processor.tick();
}

/// Called before return from interrupt handler.
pub fn before_return() {
    if let Some(processor) = PROCESSOR.try() {
        processor.lock().schedule();
    }
}

/// Called when a error occured in interrupt handler.
/// 
/// Argument: 
/// 
/// + `tf`: the TrapFrame in stack when the error occurs
pub fn error(tf: &TrapFrame) -> ! {
    if let Some(processor) = PROCESSOR.try() {
        let mut processor = processor.lock();
        let pid = processor.current_pid();
        error!("Process {} error:\n{:#x?}", pid, tf);
        processor.exit(pid, 0x100); // TODO: Exit code for error
        processor.schedule();
        unreachable!();
    } else {
        panic!("Exception when processor not inited\n{:#x?}", tf);
    }
}