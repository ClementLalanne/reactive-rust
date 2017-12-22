use continuation::Continuation;
use runtime::Runtime;
use std::rc::Rc;
use std::cell::Cell;

/// The implementation of the trait Process.
pub trait Process: 'static + Sized {
    ///Type value which is the return type of a process
    type Value;

    ///Method call which execute the process in the given runtime and executes the continuation C on the return of the process.
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

/// A process that can be executed multiple times, modifying its environment each time.
pub trait ProcessMut: Process {
    /// Executes the mutable process in the runtime, then calls `next` with the process and the
    /// process's return value.
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where
        Self: Sized, C: Continuation<(Self, Self::Value)>;

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


/// IMPLEMENTATION OF VALUE
/// Implementation of the structure needed for the function value.
pub struct Value<V> {
    value: V,
}

///Function value that creates a process that returns the value v.
pub fn value<V>(v: V) -> Value<V>{
    Value::new(v)
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

    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value> {
        next.call(runtime, self.value)
    }
}

impl<V> ProcessMut for Value<V> where V : 'static + Clone{
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where Self: Sized, C: Continuation<(Self, Self::Value)>{
        let v = self.value.clone();
        next.call(runtime, (self, v))
    }
}

/// IMPLEMENTATION OF MAP
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

impl<P,F,Y> ProcessMut for Map<P, F> where P: ProcessMut, F: FnMut(P::Value) -> Y + 'static {
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where Self: Sized, C: Continuation<(Self, Self::Value)> {
        let mut f = self.map;
        self.process.call_mut(runtime, |runtime2: &mut Runtime, (process, value): (P, P::Value)| {
            let fv = f(value);
            next.call(runtime2, (process.map(f), fv))
        });
    }
}

/// IMPLEMENTATION OF PAUSE
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

impl<P> ProcessMut for Pause<P> where P: ProcessMut{
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where Self: Sized, C: Continuation<(Self, Self::Value)> {
        runtime.on_next_instant(
            Box::new(move |runtime2 : &mut Runtime, val: ()|{
                self.process.pause().call_mut(runtime2, next)
            })
        )
    }
}

/// IMPLEMENTATION OF FLATTEN
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

impl<P> ProcessMut for Flatten<P> where P: ProcessMut, P::Value: Process {
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<(Self, Self::Value)> {
        self.process.call_mut(
            runtime, |runtime2: &mut Runtime, (p, p_v): (P, P::Value)| {
                p_v.call(runtime2, |runtime3: &mut Runtime, value: <P::Value as Process>::Value| {
                    next.call(runtime3, (p.flatten(), value));
                })
            })
    }
}

/// IMPLEMENTATION OF JOIN
/// Implementation of the structure needed for the join method.
struct JoinPoint<V1, V2> {
    return1: Cell<Option<V1>>,
    return2: Cell<Option<V2>>,
    continuation: Box<Continuation<(V1, V2)>>,
}

impl<V1, V2> JoinPoint<V1, V2> {
    pub fn new(c: Box<Continuation<(V1, V2)>>) -> Self {
        JoinPoint {
            return1: Cell::new(None),
            return2: Cell::new(None),
            continuation: c,
        }
    }
}

pub struct Join<P1, P2>{
    process1: P1,
    process2: P2,
}

impl<P1, P2, V1, V2> Process for Join<P1, P2>
    where P1: Process<Value = V1>, P2: Process<Value = V2>, V1: 'static, V2: 'static, {
    type Value = (V1, V2);
    fn call<C>(self, runtime: &mut Runtime, next: C)
        where
            C: Continuation<Self::Value>,
    {
        let join_point_1 = Rc::new(JoinPoint::new(Box::new(next)));
        let join_point_2 = join_point_1.clone();

        self.process1.call(
            runtime,
            move |runtime2: &mut Runtime, v1: V1|{
                if let Some(v2) = join_point_1.return2.take() {
                    if let Ok(join_point_1) = Rc::try_unwrap(join_point_1) {
                        join_point_1.continuation.call_box(runtime2, (v1, v2));
                    }
                } else {
                    join_point_1.return1.set(Some(v1));
                }
            });
        self.process2.call(
            runtime,
            move |runtime2: &mut Runtime, v2: V2|{
                if let Some(v1) = join_point_2.return1.take() {
                    if let Ok(join_point_2) = Rc::try_unwrap(join_point_2) {
                        join_point_2.continuation.call_box(runtime2, (v1, v2));
                    }
                } else {
                    join_point_2.return2.set(Some(v2));
                }
            });
    }
}

impl<P1, P2, V1, V2> ProcessMut for Join<P1, P2> where P1: Process<Value = V1>, P2: Process<Value = V2>,
                                                       P1: ProcessMut, P2: ProcessMut,
                                                       V1: 'static, V2: 'static,
{
    fn call_mut<C>(self, runtime: &mut Runtime, next: C)
        where
            C: Continuation<(Self, Self::Value)>,
    {
        let join_point_1 : Rc<JoinPoint<(P1, V1), (P2, V2)>> = Rc::new(JoinPoint::new(
            Box::new(|r: &mut Runtime, ((p1, v1), (p2, v2)): ((P1, V1), (P2, V2))| {
                next.call(r, (Join { process1: p1, process2: p2 }, (v1, v2)));
            })));

        let join_point_2 = join_point_1.clone();

        self.process1.call_mut(
            runtime,
            move |runtime2: &mut Runtime, v1: (P1, V1)|{
                if let Some(v2) = join_point_1.return2.take() {
                    if let Ok(join_point_1) = Rc::try_unwrap(join_point_1) {
                        join_point_1.continuation.call_box(runtime2, (v1, v2));
                    }
                } else {
                    join_point_1.return1.set(Some(v1));
                }
            });

        self.process2.call_mut(
            runtime,
            move |runtime2: &mut Runtime, v2: (P2, V2)|{
                if let Some(v1) = join_point_2.return1.take() {
                    if let Ok(join_point_2) = Rc::try_unwrap(join_point_2) {
                        join_point_2.continuation.call_box(runtime2, (v1, v2));
                    }
                } else {
                    join_point_2.return2.set(Some(v2));
                }
            });
    }
}



/// IMPLEMENTATION FOR THE WHILE METHOD
/// Implementation of the structure needed for the while method.

/// Indicates if a loop is finished.
pub enum LoopStatus<V> { Continue, Exit(V) }

pub struct While<P>{
    process: P,
}

impl<P, V> Process for While<P> where P: ProcessMut, P: Process<Value = LoopStatus<V>>{
    type Value = V;
    fn call<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<Self::Value>{
        self.process.call_mut(
            runtime,
            |runtime2: &mut Runtime, (process, val): (P, LoopStatus<V>)|{
                match val{
                    LoopStatus::Exit(v) => next.call(runtime2, v),
                    LoopStatus::Continue => (While {process}).call(runtime2, next),
                }
            }
        );
    }
}

impl<P, V> ProcessMut for While<P> where P: ProcessMut, P: Process<Value = LoopStatus<V>> {
    fn call_mut<C>(self, runtime: &mut Runtime, next: C) where C: Continuation<(Self, Self::Value)> {
        self.process.call_mut(
            runtime,
            |runtime2: &mut Runtime, (process, val): (P, LoopStatus<V>)| {
                match val {
                    LoopStatus::Exit(v) => next.call(runtime2, (While{process}, v)),
                    LoopStatus::Continue => (While {process}).call_mut(runtime2, next),
                }
            }
        )
    }
}
