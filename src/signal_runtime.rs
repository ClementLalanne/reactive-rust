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
    present: RefCell<Vec<Box<Continuation<bool>>>>,
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
            runtime.on_current_instant(Box::new(move |runtime2: &mut Runtime, ()| {
                c.call_box(runtime2, true);
            }));
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

    fn present<P1, P2, V>(self, p1: P1, p2: P2) -> Present<P1, P2>  where P1: Process<Value = V>, P2: Process<Value = V>, Self: Sized{
        Present {
            signal_runtime_ref : self.runtime(),
            p1,
            p2,
        }
    }

    // TODO: add other methods if needed.
}

/// IMPLEMENTATION OF EMIT
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

impl ProcessMut for Emit {
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<(Self, Self::Value)> {
        let signal_runtime_ref = self.signal_runtime_ref.clone();
        signal_runtime_ref.emit(runtime);
        next.call(runtime, (self, ()))
    }
}

/// IMPLEMENTATION OF AWAIT_IMMEDIATE
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

/// IMPLEMENTATION OF AWAIT
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

/// IMPLEMENTATION OF PRESENT
pub struct Present<P1, P2> {
    signal_runtime_ref: SignalRuntimeRef,
    p1: P1,
    p2: P2,
}

impl<P1, P2, V> Process for Present<P1, P2> where P1: Process<Value = V>, P2: Process<Value = V> {
    type Value = V;

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value> {
        let p1 = self.p1;
        let p2 = self.p2;
        if *(self.signal_runtime_ref.runtime.is_emited.borrow()) {
            p1.call(runtime, next)
        } else {
            let c = Box::new(
                move |runtime2: &mut Runtime, emited: bool| {
                    if emited {
                        p1.call(runtime2, next);
                    } else {
                        p2.call(runtime2, next);
                    }
                }
            );
            self.signal_runtime_ref.runtime.present.borrow_mut().push(c);

            let sig = self.signal_runtime_ref.clone();
            let c2 = Box::new(
                move |runtime2: &mut Runtime, ()| {
                    let mut present = sig.runtime.present.borrow_mut();
                    while let Some(c) = present.pop() {
                        c.call_box(runtime2, false);
                    }
                }
            );
            runtime.on_end_of_instant(c2);
        }
    }
}

impl<P1, P2, V> ProcessMut for Present<P1, P2> where P1: ProcessMut<Value = V>, P2: ProcessMut<Value = V> {
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<(Self, Self::Value)> {
        let signal = self.signal_runtime_ref.clone();
        if *(self.signal_runtime_ref.runtime.is_emited.borrow()) {
            let p2 = self.p2;
            let c = |runtime2: &mut Runtime, (process, value): (P1, P1::Value)| {
                next.call(runtime2, (Present { signal_runtime_ref: signal, p1: process, p2 }, value))
            };
            self.p1.call_mut(runtime, c);
        } else {
            let sig = self.signal_runtime_ref.clone();
            let c = Box::new(
                move |runtime2: &mut Runtime, emited: bool| {
                    if emited {
                        let p2 = self.p2;
                        let c2 = |runtime2: &mut Runtime, (process, value): (P1, P1::Value)| {
                            next.call(runtime2, (Present { signal_runtime_ref: signal, p1: process, p2 }, value))
                        };
                        self.p1.call_mut(runtime2, c2);
                    } else {
                        let p1 = self.p1;
                        let c2 = |runtime2: &mut Runtime, (process, value): (P2, P2::Value)| {
                            next.call(runtime2, (Present { signal_runtime_ref: signal, p1, p2: process }, value))
                        };
                        self.p2.call_mut(runtime2, c2);
                    }
                }
            );
            sig.runtime.present.borrow_mut().push(c);

            let c2 = Box::new(
                move |runtime2: &mut Runtime, ()| {
                    let mut present = sig.runtime.present.borrow_mut();
                    while let Some(c) = present.pop() {
                        c.call_box(runtime2, false);
                    }
                }
            );
            runtime.on_end_of_instant(c2);
        }
    }
}