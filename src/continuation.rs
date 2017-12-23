/// IMPLEMENTATION DES CONTINUATIONS
use runtime::Runtime;

/// TRAIT CONTINUATION
pub trait Continuation<V>: 'static {
    /// CALL POUR APPELER LA CONTINUATION AVEC SA VALEUR
    fn call(self, runtime: &mut Runtime, value: V);

    /// CALL POUR APPELER CONTINUATION QUI EST DANS UNE BOX
    fn call_box(self: Box<Self>, runtime: &mut Runtime, value: V);

    /// FONCTION POUR CREER UNE CONTINUATION DE TYPE MAP
    fn map<F, V2>(self, map: F) -> Map<Self, F> where Self: Sized, F: FnOnce(V2) -> V + 'static {
        Map {
            continuation: self,
            map: map
        }
    }

    /// FONCTION POUR CREER UNE PAUSE
    fn pause(self) -> Pause<Self> where Self: Sized {
        Pause {
            continuation: self
        }
    }
}

/// IMPLEMENTATION DE CONTINUATION
impl<V, F> Continuation<V> for F where F: FnOnce(&mut Runtime, V) + 'static {
    
    fn call(self, runtime: &mut Runtime, value: V)  {
        self(runtime, value);
    }

    fn call_box(self: Box<Self>, runtime: &mut Runtime, value: V) {
        (*self).call(runtime, value);
    }
}

/// IMPLEMENTATION POUR MAP
pub struct Map<C, F> {
    continuation: C,
    map: F
}
impl<C, F, X, Y> Continuation<X> for Map<C, F> where C: Continuation<Y>, F: FnOnce(X) -> Y + 'static {

    fn call(self, runtime: &mut Runtime, value: X) {
        self.continuation.call(runtime, (self.map)(value))
    }

    fn call_box(self: Box<Self>, runtime: &mut Runtime, value: X) {
        (*self).call(runtime, value)
    }
}

/// IMPLEMENTATION POUR PAUSE
pub struct Pause<C> {
    continuation: C,
}

impl<C, V> Continuation<V> for Pause<C> where C: Continuation<V>, V: 'static {

    fn call(self, runtime: &mut Runtime, value: V)  {
        runtime.on_next_instant(Box::new(move |runtime2: &mut Runtime, value2 : ()| {
            self.continuation.call(runtime2, value);
        }));
    }

    fn call_box(self: Box<Self>, runtime: &mut Runtime, value: V) {
        (*self).call(runtime, value);
    }
}
