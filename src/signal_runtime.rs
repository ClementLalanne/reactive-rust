use std::rc::Rc;
use continuation::Continuation;
use runtime::Runtime;
use process::Process;
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
    await_immediate: RefCell<Vec<Box<Continuation<()>>>>,
}

impl SignalRuntimeRef {
    /// Sets the signal as emitted for the current instant.
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

struct SimpleSignal{
    signal_runtime_ref : SignalRuntimeRef
}

impl Signal for SimpleSignal{
    fn runtime(self){
        self.signal_runtime_ref
    }

    fn await_immediate(self){
        AwaitImmediate {
            signal_runtime_ref : self.signal_runtime_ref
        }
    }
}

struct AwaitImmediate {
    signal_runtime_ref : SignalRuntimeRef
}

impl Process for AwaitImmediate {
    type Value = ();

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<()> {
        let f = Box::new(|runtime2: &mut Runtime, ()| {
            if self.signal_runtime_ref.is_emited {
                next.call(runtime2, ())
            }
            else {
                runtime2.on_next_instant(f)
            }
        });
        runtime.on_end_of_instant(f)
    }
}

impl ProcessMut for AwaitImmediate {
    // TODO
}
