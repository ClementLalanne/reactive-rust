use continuation::Continuation;
use runtime::Runtime;
use process::Process;
use process::ProcessMut;
use std::rc::Rc;
use std::cell::Cell;
use std::cell::RefCell;

/// A shared pointer to a signal runtime.
#[derive(Clone)]
pub struct SignalRuntimeRef {
    runtime: Rc<SignalRuntime>,
}

/// Runtime for pure signals.
struct SignalRuntime {
    is_emited: RefCell<bool>,
    await: RefCell<Vec<Box<Continuation<()>>>>,
    await_immediate: RefCell<Vec<Box<Continuation<()>>>>,
    present: RefCell<Vec<Box<Continuation<()>>>>,
}

impl SignalRuntime {
    pub fn new() -> Self {
        SignalRuntime {
            is_emited: RefCell::new(false),
            await: RefCell::new(vec!()),
            await_immediate: RefCell::new(vec!()),
            present: RefCell::new(vec!()),
        }
    }

}

impl SignalRuntimeRef {
    /// Sets the signal as emitted f    or the current instant.
    fn emit(self, runtime: &mut Runtime) {
        unimplemented!() // TODO
    }

    /// Calls `c` at the first cycle where the signal is present.
    fn on_signal<C>(self, runtime: &mut Runtime, c: C) where C: Continuation<()> {
        unimplemented!() // TODO
    }

    // TODO: add other methods when needed.
}

/// A reactive signal.
pub trait Signal {
    /// Returns a reference to the signal's runtime.
    fn runtime(self) -> SignalRuntimeRef;

    /// Returns a process that waits for the next emission of the signal, current instant
    /// included.
    fn await_immediate(self) -> AwaitImmediate where Self: Sized {
      unimplemented!() // TODO
    }

    // TODO: add other methods if needed.
}

struct AwaitImmediate {
    // TODO
}

/*impl Process for AwaitImmediate {
    // TODO
}

impl ProcessMut for AwaitImmediate {
    // TODO
}*/
