use continuation::Continuation;
use runtime::Runtime;
use process::Process;
use process::ProcessMut;
use std::rc::Rc;
use std::cell::Cell;
use std::cell::RefCell;
use std::mem;

/// A shared pointer to a signal runtime.
pub struct SignalRuntimeRef<SIO> where SIO : SignalIO {
    runtime: Rc<SignalRuntime<SIO>>,
}

pub trait SignalIO {
    type Value ;

    fn set(&self, v: Self::Value);
    fn get(&self) -> Self::Value;
}

/// Runtime for pure signals.
pub struct SignalRuntime<SIO> where SIO : SignalIO{
    is_emited: RefCell<bool>,
    io: SIO,
    await: RefCell<Vec<Box<Continuation<()>>>>,
    await_in: RefCell<Vec<Box<Continuation<SIO::Value>>>>,
    await_immediate: RefCell<Vec<Box<Continuation<()>>>>,
    await_immediate_in: RefCell<Vec<Box<Continuation<SIO::Value>>>>,
    present: RefCell<Vec<Box<Continuation<bool>>>>,
}

impl<SIO> Clone for SignalRuntimeRef<SIO> where SIO: SignalIO {
    fn clone(&self) -> Self {
        SignalRuntimeRef { runtime: self.runtime.clone() }
    }
}

impl<SIO> SignalRuntimeRef<SIO> where SIO: SignalIO + 'static {
    /// Sets the signal as emitted f    or the current instant.
    fn emit(&self, runtime: &mut Runtime, v: SIO::Value) {
        self.runtime.io.set(v);
        let mut is_emited = self.runtime.is_emited.borrow_mut();
        *is_emited = true;

        // AWAIT_IMMEDIATE
        let mut await_immediate = self.runtime.await_immediate.borrow_mut();
        while let Some(c) = await_immediate.pop() {
            runtime.on_current_instant(c);
        }


        let mut await_immediate_in = self.runtime.await_immediate_in.borrow_mut();
        while let Some(c) = await_immediate_in.pop() {
            let v = self.runtime.io.get();
            let c2 = Box::new(move |runtime2 : &mut Runtime, ()| {
                c.call_box(runtime2, v);
            });
            runtime.on_current_instant(c2)
        }

        let mut await = self.runtime.await.borrow_mut();
        while let Some(c) = await.pop() {
            runtime.on_next_instant(c);
        }

        let mut await_in = self.runtime.await_in.borrow_mut();
        while let Some(c) = await_in.pop() {
            let v = self.runtime.io.get();
            let c2 = Box::new(move |runtime2 : &mut Runtime, ()| {
                c.call_box(runtime2, v);
            });
            runtime.on_next_instant(c2);
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
pub trait Signal<SIO> where SIO: SignalIO {
    /// Returns a reference to the signal's runtime.
    fn runtime(self) -> SignalRuntimeRef<SIO>;

    fn emit<P>(self, p: P) -> Emit<SIO, P> where Self: Sized, P: ProcessMut<Value = SIO::Value> {
        Emit {
            p,
            signal_runtime_ref : self.runtime(),
        }
    }

    /// Returns a process that waits for the next emission of the signal, current instant
    /// included.
    fn await_immediate(self) -> AwaitImmediate<SIO> where Self: Sized {
        AwaitImmediate {
            signal_runtime_ref : self.runtime()
        }
    }

    fn await_immediate_in(self) -> AwaitImmediateIn<SIO> where Self: Sized {
        AwaitImmediateIn {
            signal_runtime_ref : self.runtime()
        }
    }

    fn await(self) -> Await<SIO> where Self: Sized {
        Await {
            signal_runtime_ref : self.runtime()
        }
    }

    fn await_in(self) -> AwaitIn<SIO> where Self: Sized {
        AwaitIn {
            signal_runtime_ref : self.runtime()
        }
    }

    fn present<P1, P2, V>(self, p1: P1, p2: P2) -> Present<SIO, P1, P2>  where P1: Process<Value = V>, P2: Process<Value = V>, Self: Sized{
        Present {
            signal_runtime_ref : self.runtime(),
            p1,
            p2,
        }
    }

    // TODO: add other methods if needed.
}

/// IMPLEMENTATION OF EMIT
struct Emit<SIO, P> where SIO: SignalIO {
    p: P,
    signal_runtime_ref : SignalRuntimeRef<SIO>
}

impl<SIO, P> Process for Emit<SIO, P> where SIO: SignalIO + 'static, P: Process<Value = SIO::Value> {
type Value = ();

fn call<C> ( self, runtime: & mut Runtime, next: C) where C: Continuation<Self::Value > {
    let signal = self.signal_runtime_ref;
    self.p.call(runtime, move |runtime2: &mut Runtime, v: SIO::Value| {
        signal.emit(runtime2, v);
        next.call(runtime2, ())
    })
    }
}

impl<SIO, P> ProcessMut for Emit<SIO, P> where SIO: SignalIO + 'static, P: ProcessMut<Value = SIO::Value> {
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<(Self, Self::Value)> {
        let signal_runtime_ref = self.signal_runtime_ref.clone();
        let signal = self.signal_runtime_ref;

        self.p.call_mut(runtime, move |runtime2: &mut Runtime, (p, v): (P, P::Value)| {
            signal.emit(runtime2, v);
            next.call(runtime2, (Emit{signal_runtime_ref, p}, ()))
        })
    }
}

/// IMPLEMENTATION OF AWAIT_IMMEDIATE
struct AwaitImmediate<SIO> where SIO: SignalIO{
    signal_runtime_ref : SignalRuntimeRef<SIO>
}

impl<SIO> Process for AwaitImmediate<SIO> where SIO: SignalIO + 'static {
    type Value = ();

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value> {
        if *(self.signal_runtime_ref.runtime.is_emited.borrow()) {
            next.call(runtime, ())
        }
        else {
            self.signal_runtime_ref.runtime.await_immediate.borrow_mut().push(Box::new(next))
        }
    }
}


impl<SIO> ProcessMut for AwaitImmediate<SIO> where SIO: SignalIO + 'static {
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<(Self, Self::Value)> {
        if *(self.signal_runtime_ref.runtime.is_emited.borrow()) {
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

/// IMPLEMENTATION OF AWAIT_IMMEDIATE_IN
struct AwaitImmediateIn<SIO> where SIO: SignalIO{
    signal_runtime_ref : SignalRuntimeRef<SIO>
}

impl<SIO> Process for AwaitImmediateIn<SIO> where SIO: SignalIO + 'static {
    type Value = SIO::Value;

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value> {
        if *(self.signal_runtime_ref.runtime.is_emited.borrow()) {
            let v = self.signal_runtime_ref.runtime.io.get();
            next.call(runtime, v);
        } else {
            let c2 = Box::new(move |runtime2: &mut Runtime, v: SIO::Value| {
                next.call(runtime2, v)
            });
            self.signal_runtime_ref.runtime.await_immediate_in.borrow_mut().push(c2)
        }
    }
}

impl<SIO> ProcessMut for AwaitImmediateIn<SIO> where SIO: SignalIO + 'static {
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<(Self, Self::Value)> {
        if *(self.signal_runtime_ref.runtime.is_emited.borrow()) {
            let v = self.signal_runtime_ref.runtime.io.get();
            next.call(runtime, (self, v))
        } else {
            let signal_runtime_ref = self.signal_runtime_ref.clone();
            let c2 = Box::new(move |runtime2: &mut Runtime, v: SIO::Value| {
                next.call(runtime2, (AwaitImmediateIn {signal_runtime_ref}, v))
            });
            self.signal_runtime_ref.runtime.await_immediate_in.borrow_mut().push(c2);
        }
    }
}

/// IMPLEMENTATION OF AWAIT
struct Await<SIO> where SIO: SignalIO {
    signal_runtime_ref : SignalRuntimeRef<SIO>
}

impl<SIO> Process for Await <SIO> where SIO: SignalIO + 'static{
    type Value = ();

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value> {
        if *(self.signal_runtime_ref.runtime.is_emited.borrow()) {
            runtime.on_next_instant(Box::new(next))
        } else {
            self.signal_runtime_ref.runtime.await.borrow_mut().push(Box::new(next))
        }
    }
}

impl<SIO> ProcessMut for Await <SIO> where SIO: SignalIO + 'static{
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

struct AwaitIn<SIO> where SIO: SignalIO {
    signal_runtime_ref : SignalRuntimeRef<SIO>
}

impl<SIO> Process for AwaitIn <SIO> where SIO: SignalIO + 'static{
    type Value = SIO::Value;

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value> {
        if *(self.signal_runtime_ref.runtime.is_emited.borrow()) {
            let v = self.signal_runtime_ref.runtime.io.get();
            let c2 = Box::new(
                move |runtime2: &mut Runtime, ()| {
                    next.call(runtime2, v);
                }
            );
            runtime.on_next_instant(c2);
        } else {
            let c2 = Box::new(move |runtime2: &mut Runtime, v: SIO::Value| {
                next.call(runtime2, v)
            });
            self.signal_runtime_ref.runtime.await_in.borrow_mut().push(c2)
        }
    }
}

/// IMPLEMENTATION OF PRESENT
pub struct Present<SIO, P1, P2> where SIO: SignalIO + 'static {
    signal_runtime_ref: SignalRuntimeRef<SIO>,
    p1: P1,
    p2: P2,
}

/*impl<C1, C2, SIO> Process for Present<C1, C2, SIO> where C1: Continuation<()>, C2: Continuation<()>, SIO: SignalIO + 'static {
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
}*/

/*impl ProcessMut for Present {
    // TODO
}*/


///IMPLEMENTATION OF SIMPLE SIGNALS

struct SimpleSignal {}

/*impl SignalIO for SimpleSignal {
    type Value = ();


}*/

///IMPLEMENTATION OF SIGNALS WITH MULTIPLE CONSUMPTION

struct MCSignal {}

///IMPLEMENTATION OF SIGNALS WITH SIMPLE CONSUMPTION

struct SCSignal {}
