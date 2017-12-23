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
    fn reset_value(&self);
    fn is_simple(&self) -> bool;
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
    fn clone(&self) -> Self { SignalRuntimeRef { runtime: self.runtime.clone() }
    }
}

impl<SIO> SignalRuntimeRef<SIO> where SIO: SignalIO + 'static {
    pub fn new(io: SIO) -> Self {
        let runtime = SignalRuntime {
            is_emited: RefCell::new(false),
            io,
            await: RefCell::new(vec!()),
            await_in: RefCell::new(vec!()),
            await_immediate: RefCell::new(vec!()),
            await_immediate_in: RefCell::new(vec!()),
            present: RefCell::new(vec!()),
        };

        SignalRuntimeRef { runtime: Rc::new(runtime) }
    }

    /// Sets the signal as emitted for the current instant.
    fn emit(&self, runtime: &mut Runtime, v: SIO::Value) {
        let self_clone = self.clone();
        self.runtime.io.set(v);
        let mut is_emited = self.runtime.is_emited.borrow_mut();
        *is_emited = true;

        // AWAIT_IMMEDIATE
        let mut await_immediate = self.runtime.await_immediate.borrow_mut();
        while let Some(c) = await_immediate.pop() {
            runtime.on_current_instant(c);
        }


        //AWAIT_IMMEDIATE_IN
        let mut await_immediate_in = self.runtime.await_immediate_in.borrow_mut();
        while let Some(c) = await_immediate_in.pop() {
            let v = self.runtime.io.get();
            let c2 = Box::new(move |runtime2 : &mut Runtime, ()| {
                c.call_box(runtime2, v);
            });
            runtime.on_current_instant(c2)
        }

        // AWAIT and AWAIT_IN
        // If the signal is at multiple consumption we execute all the AWAIT and AWAIT_IN
        // Otherwise we only execute one arbitrary chosen
        if !self.runtime.io.is_simple(){
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
        }
        else {
            let mut treated = false;
            let mut await = self.runtime.await.borrow_mut();
            while let Some(c) = await.pop() {
                runtime.on_next_instant(c);
                treated = true;
                break;
            }
            if !treated{
                let mut await_in = self.runtime.await_in.borrow_mut();
                while let Some(c) = await_in.pop() {
                    let v = self.runtime.io.get();
                    let c2 = Box::new(move |runtime2 : &mut Runtime, ()| {
                        c.call_box(runtime2, v);
                    });
                    runtime.on_next_instant(c2);
                    break;
                }
            }
    }

        let mut present = self.runtime.present.borrow_mut();
        while let Some(c) = present.pop() {
            runtime.on_current_instant(Box::new(move |runtime2: &mut Runtime, ()| {
                c.call_box(runtime2, true);
            }));
        }

        // ON REMET IS_EMITED À FALSE POUR NE PAS AVOIR DE PROBLEME À L'INSTANT SUIVANT
        let c_close = move |runtime2: &mut Runtime, ()| {
            let mut is_emited2 = self_clone.runtime.is_emited.borrow_mut();
            *is_emited2 = false;
            self_clone.runtime.io.reset_value();
        };
        runtime.on_end_of_instant(Box::new(c_close));
    }

    /// Calls `c` at the first cycle where the signal is present.
    fn on_signal<C>(self, runtime: &mut Runtime, c: C) where C: Continuation<()> {
        if *self.runtime.is_emited.borrow() {
            c.call(runtime, ());
        } else {
            self.runtime.await_immediate.borrow_mut().push(Box::new(c));
        }
    }
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
pub struct AwaitImmediate<SIO> where SIO: SignalIO{
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
pub struct AwaitImmediateIn<SIO> where SIO: SignalIO{
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
pub struct Await<SIO> where SIO: SignalIO {
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

/// IMPLEMENTATION AWAIT_IN
pub struct AwaitIn<SIO> where SIO: SignalIO {
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

impl<SIO> ProcessMut for AwaitIn<SIO> where SIO: SignalIO + 'static {
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<(Self, Self::Value)> {
        if *(self.signal_runtime_ref.runtime.is_emited.borrow()) {
            let v = self.signal_runtime_ref.runtime.io.get();
            next.call(runtime, (self, v));
        } else {
            let signal_runtime_ref = self.signal_runtime_ref.clone();
            let c2 = Box::new(move |runtime2: &mut Runtime, v: SIO::Value| {
                next.call(runtime2, (AwaitIn {signal_runtime_ref}, v))
            });
            self.signal_runtime_ref.runtime.await_in.borrow_mut().push(c2);
        }
    }
}

/// IMPLEMENTATION OF PRESENT
pub struct Present<SIO, P1, P2> where SIO: SignalIO + 'static {
    signal_runtime_ref: SignalRuntimeRef<SIO>,
    p1: P1,
    p2: P2,
}

impl<SIO, P1, P2, V> Process for Present<SIO, P1, P2> where SIO: SignalIO + 'static, P1: Process<Value = V>, P2: Process<Value = V> {
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

impl<SIO, P1, P2, V> ProcessMut for Present<SIO, P1, P2> where SIO: SignalIO + 'static, P1: ProcessMut<Value = V>, P2: ProcessMut<Value = V> {
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

///IMPLEMENTATION OF SIMPLE SIGNALS
pub struct SimpleSignalIO {}

impl SimpleSignalIO {
    pub fn new() -> SimpleSignalIO {
        SimpleSignalIO { }
    }
}

impl SignalIO for SimpleSignalIO {
    type Value = ();
    fn set(&self, v: ()) {}
    fn get(&self) -> () { () }
    fn reset_value(&self) {}
    fn is_simple(&self) -> bool{
        false
    }
}

pub struct SimpleSignal<V> where V: SignalIO<Value = ()> {
    signal: SignalRuntimeRef<V>,
}

impl<V> SimpleSignal<V> where V: SignalIO<Value = ()> {
    pub fn new() -> SimpleSignal<SimpleSignalIO> {
        let signal = SignalRuntimeRef::new(SimpleSignalIO::new());
        SimpleSignal {
            signal,
        }
    }
}

///IMPLEMENTATION OF SIGNALS WITH MULTIPLE CONSUMPTION
pub struct MCSignalIO<V> {
    value: RefCell<V>,
    default_value: V,
}

impl<V> MCSignalIO<V>
    where V: Clone {
    pub fn new(default_value: V) -> MCSignalIO<V> {
        MCSignalIO {
            value: RefCell::new(default_value.clone()),
            default_value,
        }
    }
}

impl<V> SignalIO for MCSignalIO<V> where V: Clone{
    type Value = V;
    fn set(&self, v: V) {
        *self.value.borrow_mut() = v;
    }

    fn get(&self) -> V {
        self.value.borrow().clone()
    }

    fn reset_value(&self) {
        *self.value.borrow_mut() = self.default_value.clone()
    }

    fn is_simple(&self) -> bool{
        false
    }
}

pub struct MCSignal<V> where V: SignalIO{
    signal: SignalRuntimeRef<V>,
}

impl<V> MCSignal<V> where V: SignalIO + 'static {
    pub fn new(v: V) -> Self {
        let signal = SignalRuntimeRef::new(v);
        MCSignal {
            signal,
        }
    }
}
impl<V> Signal<V> for MCSignal<V> where V: SignalIO{
    fn runtime(self) -> SignalRuntimeRef<V> {
        self.signal.clone()
    }
}

///IMPLEMENTATION OF SIGNALS WITH SIMPLE CONSUMPTION

pub struct SCSignalIO<V> {
    value: RefCell<V>,
    default_value: V,
}

impl<V> SCSignalIO<V>
    where V: Clone {
    pub fn new(default_value: V) -> SCSignalIO<V> {
        SCSignalIO {
            value: RefCell::new(default_value.clone()),
            default_value,
        }
    }
}

impl<V> SignalIO for SCSignalIO<V> where V: Clone{
    type Value = V;
    fn set(&self, v: V) {
        *self.value.borrow_mut() = v;
    }

    fn get(&self) -> V {
        self.value.borrow().clone()
    }

    fn reset_value(&self) {
        *self.value.borrow_mut() = self.default_value.clone()
    }

    fn is_simple(&self) -> bool{
        true
    }
}

pub struct SCSignal<V> where V: SignalIO{
    signal: SignalRuntimeRef<V>,
}

impl<V> SCSignal<V> where V: SignalIO + 'static {
    pub fn new(v: V) -> Self {
        let signal = SignalRuntimeRef::new(v);
        SCSignal {
            signal,
        }
    }
}
impl<V> Signal<V> for SCSignal<V> where V: SignalIO{
    fn runtime(self) -> SignalRuntimeRef<V> {
        self.signal.clone()
    }
}
