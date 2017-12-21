use continuation::Continuation;
use runtime::Runtime;
use process::Process;
use process::ProcessMut;
use std::rc::Rc;
use std::cell::Cell;
use std::cell::RefCell;
use std::mem;

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
        let mut is_emited = self.runtime.is_emited.borrow_mut();
        *is_emited = true;

        // AWAIT_IMMEDIATE
        let mut await_immediate = self.runtime.await_immediate.borrow_mut();
        while let Some(c) = await_immediate.pop() {
            runtime.on_current_instant(c);
        }

        let mut await = self.runtime.await.borrow_mut();
        while let Some(c) = await.pop() {
            runtime.on_next_instant(c);
        }

        let mut present = self.runtime.present.borrow_mut();
        while let Some(c) = present.pop() {
            runtime.on_next_instant(c);
        }
    }

    /// Calls `c` at the first cycle where the signal is present.
    fn on_signal<C>(self, runtime: &mut Runtime, c: C) where C: Continuation<()> {
        if *self.runtime.is_emited.borrow() {
            c.call(runtime, ());
        } else {
            self.runtime.await_immediate.borrow_mut().push(Box::new(c));
        }
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
        AwaitImmediate {
            signal_runtime_ref : self.runtime()
        }
    }

    fn await(self) -> Await where Self: Sized {
        Await {
            signal_runtime_ref : self.runtime()
        }
    }

    fn present<C1, C2>(self, c1: C1, c2: C2) -> Present<C1, C2>  where C1: Continuation<()>, C2: Continuation<()>, Self: Sized{
        unimplemented!() // TODO
    }

    // TODO: add other methods if needed.
}

struct AwaitImmediate {
    signal_runtime_ref : SignalRuntimeRef
}

/*impl Process for AwaitImmediate {
    // TODO
}

impl ProcessMut for AwaitImmediate {
    // TODO
}*/

struct Await {
    signal_runtime_ref : SignalRuntimeRef
}

/*impl Process for Await {
    // TODO
}

impl ProcessMut for Await {
    // TODO
}*/

struct Present<C1, C2> where C1: Continuation<()>, C2: Continuation<()> {
    signal_runtime_ref : SignalRuntimeRef,
    c1 : C1,
    c2 : C2,
}

/*impl Process for Present {
    // TODO
}

impl ProcessMut for Present {
    // TODO
}*/
