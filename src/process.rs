use continuation::Continuation;
use runtime::Runtime;
use std::rc::Rc;
use std::cell::Cell;

/// The implementation of the trait Process.
pub trait Process: 'static{
    ///Type value which is the return type of a process
    type Value;

    ///Method call which execute the process in the gimen runtime and executes the continuation C on the return of the process.
    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value>;

    ///Method map which transforms the process in a new process which applies the function map to the result of the initial process.
    fn map<F, V2>(self, map: F) -> Map<Self, F> where Self: Sized, F: FnOnce(Self::Value) -> V2 + 'static {
        Map {
            process: self,
            map: map
        }
    }

    ///Method pause which pauses on the current instant and starts again on the next instant.
    fn pause(self) -> Pause<Self> where Self: Sized {
        Pause {
            process: self
        }
    }

    ///Method flatten which transforms a process that returns a process in a new process that executes the process and executes its result.
    fn flatten(self)-> Flatten<Self> where Self: Sized {
        Flatten {
            process: self,
        }
    }

    ///Method and_then equivalent to map and then flatten.
    fn and_then<F, V2>(self, map: F) -> Flatten<Map<Self, F>> where Self: Sized, F: FnOnce(Self::Value) -> V2 + 'static {
        self.map(map).flatten()
    }

    ///Method join which takes a process and returns a process which returns the couple of the results of the first process and the second.
    fn join<P>(self, p: P) -> Join<Self, P> where Self: Sized, P: Process {
        Join{
            process1: self,
            process2: p,
        }
    }
}

/// A process that can be executed multiple times, modifying its environement each time.
pub trait ProcessMut: Process {
    /// Executes the mutable process in the runtime, then calls `next` with the process and the
    /// process's return value.
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where Self: Sized, C: Continuation<(Self, Self::Value)>;

    fn while_processmut<V>(self) -> While<Self> where Self: Sized{
        While{
            process: self,
        }
    }
}


///Function execute_process which takes a process, executes it in a new runtime and returns the result.
pub fn execute_process<P>(p: P) -> P::Value where P:Process {
    let mut runtime = Runtime::new();
    let ref_1_return = Rc::new(Cell::new(None));
    let ref_2_return = ref_1_return.clone();
    p.call(
        &mut runtime,
        move |runtime2: &mut Runtime, value: P::Value| {
            ref_2_return.set(Some(value))
        });
    runtime.execute();
    ref_1_return.take().unwrap()
}

///Function value that creates a process that returns the value v.
pub fn value<V>(v: V) -> Value<V>{
    Value::new(v)
}



/// Implementation of the structure needed for the function value.
pub struct Value<V> {
    value: V,
}

impl<V> Value<V> {
    pub fn new(v: V) -> Self{
        Value{
            value: v,
        }
    }
}

impl<V> Process for Value<V> where V : 'static {
    type Value = V;

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value>{
        next.call(runtime, self.value)
    }
}

impl<V> ProcessMut for Value<V> where V : 'static{
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where Self: Sized, C: Continuation<(Self, Self::Value)>{
        next.call(runtime, (self, self.value))
    }
}

/// Implementation of the structure needed for the map method.
pub struct Map<P, F> {
    process: P,
    map: F
}

impl<P, F, Y> Process for Map<P, F> where P: Process, F: FnOnce(P::Value) -> Y + 'static {
    type Value = Y;

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value> {
        let f = self.map;
        self.process.call(runtime,
            |runtime2: &mut Runtime, value: P::Value| {
                next.call(runtime2, f(value))
        })
    }
}

impl<P,F,Y> ProcessMut for Map<P, F> where P: Process, F: FnOnce(P::Value) -> Y + 'static {
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where Self: Sized, C: Continuation<(Self, Self::Value)> {
        let f = self.map;
        self.process.call(runtime,
            |runtime2: &mut Runtime, value: P::Value| {
                next.call(runtime2, (self, f(value)))
        })
    }
}

/// Implementation of the structure needed for the pause method.
pub struct Pause<P> {
    process: P,
}

impl<P> Process for Pause<P> where P: Process{
    type Value = P::Value;

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value> {
        runtime.on_next_instant(
            Box::new(move |runtime2 : &mut Runtime, val: ()|{
                self.process.call(runtime2, next)
            })
        )
    }
}

/*impl<P> ProcessMut for Pause<P> where P: ProcessMut{
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where Self: Sized, C: Continuation<(Self, Self::Value)> {
        runtime.on_next_instant(
            Box::new(move |runtime2 : &mut Runtime, val: ()|{
                self.process.call_mut(runtime2, next)
            })
        )
    }
}*/

/// Implementation of the structure needed for the flatten method.
pub struct Flatten<P> {
    process: P,
}

impl<P> Process for Flatten<P> where P: Process, P::Value: Process {
    type Value = <P::Value as Process>::Value;

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value> {
        self.process.call(
            runtime,
            |runtime2: &mut Runtime, v: P::Value|{
                v.call(runtime2, next)
            });
    }
}

/// Implementation of the structure needed for the join method.
struct JoinPoint<P1, P2> where P1: Process, P2: Process{
    return1: Box<Cell<Option<P1::Value>>>,
    return2: Box<Cell<Option<P2::Value>>>,
    continuation: Box<Continuation<(P1::Value, P2::Value)>>,
}

impl<P1, P2> JoinPoint<P1, P2> where P1: Process, P2: Process{
    pub fn new(c: Box<Continuation<(P1::Value, P2::Value)>>) -> Self {
        JoinPoint{
            return1: Box::new(Cell::new(None)),
            return2: Box::new(Cell::new(None)),
            continuation: c,
        }
    }
}

pub struct Join<P1, P2>{
    process1: P1,
    process2: P2,
}

impl<P1, P2> Process for Join<P1, P2> where P1: Process, P2: Process{
    type Value = (P1::Value, P2::Value);

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<(P1::Value, P2::Value)>{
        let join_point_1 = Rc::new(JoinPoint::new(Box<next>));
        let join_point_2 = join_point_1.clone();
        self.process1.call(
            runtime,
            move |runtime2: &mut Runtime, v1: P1::Value|{
                join_point_1.return1.set(Some(v1));
                if let Some(v2) = join_point_1.return2.take() {
                    join_point_1.continuation.call(runtime2, (v1, v2))
                };
            });
        self.process2.call(
            runtime,
            move |runtime2: &mut Runtime, v2: P2::Value|{
                join_point_2.return2.set(Some(v2));
                if let Some(v1) = join_point_2.return1.take() {
                    join_point_2.continuation.call(runtime2, (v1, v2))
                };
            });
    }
}

/// Implementation of the structure needed for the while method.

/// Indicates if a loop is finished.
pub enum LoopStatus<V> { Continue, Exit(V) }

pub struct While<P>{
    process: P,
}

/*impl<P, V> Process for While<P> where P: ProcessMut, P::Value: LoopStatus<V>{
    type Value = V;

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<V>{
        self.process.call_mut(
            runtime,
            |runtime2: &mut Runtime, (p, val): (ProcessMut, LoopStatus<V>)|{
                match val{
                    LoopStatus::Exit(V) => next.call(runtime2, val),
                    LoopStatus::Continue => p.call_mut(runtime2, next),
                }
            }
        );
    }
}*/
