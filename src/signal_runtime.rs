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
            runtime.on_current_instant(c);
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

    fn emit(self) -> Emit where Self: Sized {
        Emit {
            signal_runtime_ref : self.runtime(),
        }
    }

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
        Present {
            signal_runtime_ref : self.runtime(),
            c1 : c1,
            c2 : c2,
            is_present : RefCell::new(false)
        }
    }

    // TODO: add other methods if needed.
}

struct Emit {
    signal_runtime_ref : SignalRuntimeRef
}

impl Process for Emit {
    type Value = ();

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value> {
        self.signal_runtime_ref.emit(runtime);
        next.call(runtime, ())
    }
}

/*impl ProcessMut for Emit {
    // TODO
}*/

struct AwaitImmediate {
    signal_runtime_ref : SignalRuntimeRef
}

impl Process for AwaitImmediate {
    type Value = ();

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value> {
        if *(self.signal_runtime_ref.runtime.is_emited.borrow_mut()) {
            next.call(runtime, ())
        }
        else {
            self.signal_runtime_ref.runtime.await_immediate.borrow_mut().push(Box::new(next))
        }
    }
}


impl ProcessMut for AwaitImmediate {
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<(Self, Self::Value)> {
        if *(self.signal_runtime_ref.runtime.is_emited.borrow_mut()) {
            next.call(runtime, (self, ()))
        } else {
            let signal = self.signal_runtime_ref.clone();
            self.signal_runtime_ref.runtime.await_immediate.borrow_mut().push(Box::new(
                move |runtime2: &mut Runtime, ()| {
                    next.call(runtime2, (AwaitImmediate { signal_runtime_ref: signal}, ()))
                }
            ))
        }
    }
}

struct Await {
    signal_runtime_ref : SignalRuntimeRef
}

impl Process for Await {
    type Value = ();

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value> {
        if *(self.signal_runtime_ref.runtime.is_emited.borrow_mut()) {
            runtime.on_next_instant(Box::new(next))
        }
        else {
            self.signal_runtime_ref.runtime.await.borrow_mut().push(Box::new(next))
        }
    }
}

impl ProcessMut for Await {
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<(Self, Self::Value)> {
        //let signal = self.signal_runtime_ref.clone();
        let signal_runtime_ref = self.signal_runtime_ref.clone();
        let c = Box::new(move |runtime2: &mut Runtime, v: Self::Value| {
            next.call(runtime2, (Await {signal_runtime_ref}, v))
        });
        if *(self.signal_runtime_ref.runtime.is_emited.borrow()) {
            runtime.on_next_instant(c);
        } else {
            self.signal_runtime_ref.runtime.await.borrow_mut().push(c);
        }
    }
}

struct Present<C1, C2> where C1: Continuation<()>, C2: Continuation<()> {
    signal_runtime_ref : SignalRuntimeRef,
    c1 : C1,
    c2 : C2,
    is_present: RefCell<bool>,  
}

impl<C1, C2> Process for Present<C1, C2> where C1: Continuation<()>, C2: Continuation<()> {
    type Value = ();

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value> {
        if *(self.signal_runtime_ref.runtime.is_emited.borrow_mut()) {
            self.c1.call(runtime, ());
            next.call(runtime, ())
        }
        else {
            let mut present = self.signal_runtime_ref.runtime.present.borrow_mut();
            let c1 = self.c1;
            let c2 = self.c2;
            let is_present1 = self.is_present.clone();
            let is_present2 = self.is_present.clone();
            present.push(Box::new(
                move |runtime2 : &mut Runtime, ()|{
                    if !*(is_present1.borrow_mut()) {
                        *(is_present1.borrow_mut()) = true;
                        c1.call(runtime2, ());
                    }
                }
            ));
            runtime.on_end_of_instant(Box::new(
                move |runtime2: &mut Runtime, ()|{
                    if !*(is_present2.borrow_mut()) {
                        *(is_present2.borrow_mut()) = true;
                        c2.call(runtime2, ());
                    }
                    next.call(runtime2, ());
                })
            )
        }
    }
}

/*impl ProcessMut for Present {
    // TODO
}*/
