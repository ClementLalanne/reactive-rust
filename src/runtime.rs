/// IMPLEMENTATION DU RUNTIME
use continuation::Continuation;
use std;

/// Structure du runtime, suivant les structures utilisees, la continuation ne sera pas au même endroit.
pub struct Runtime {
    current_instant: Vec<Box<Continuation<()>>>,
    end_of_instant: Vec<Box<Continuation<()>>>,
    next_instant: Vec<Box<Continuation<()>>>,
}

/// IMPLEMENTATION DE RUNTIME
impl Runtime {

    /// CREATION D'UN RUNTIME
    pub fn new() -> Self {
        Runtime {
            current_instant: vec!(),
            end_of_instant: vec!(),
            next_instant: vec!(),
        }
    }

    /// FONCTION POUR EXECUTER LES ELEMENTS D'UN INSTANT
    pub fn instant(&mut self) -> bool {
        while let Some(p) = self.current_instant.pop() {
            p.call_box(self, ())
        };
        std::mem::swap(&mut self.current_instant, &mut self.next_instant);
        let mut end_of_curent_instant = vec!();
        std::mem::swap(&mut self.end_of_instant, &mut end_of_curent_instant);
        while let Some(p) = end_of_curent_instant.pop() {
            p.call_box(self, ())
        };
        !self.current_instant.is_empty() || !self.next_instant.is_empty() || !self.end_of_instant.is_empty()
    }

    /// FONCTION POUR EXECUTER LES ELEMENTS DE CHAQUE INSTANT TANT QUE L'INSTANT SUIVANT N'EST PAS VIDE
    pub fn execute(&mut self) {
        while self.instant() {
            continue;
        }
    }

    /// FONCTION POUR RAJOUTER UNE CONTINUATION A L'INSTANT PRESENT
    pub fn on_current_instant(&mut self, c: Box<Continuation<()>>) {
    self.current_instant.push(c)
  }

    /// FONCTION POUR RAJOUTER UNE CONTINUATION A L'INSTANT SUIVANT
    pub fn on_next_instant(&mut self, c: Box<Continuation<()>>) {
    self.next_instant.push(c)
  }

    /// FONCTION POUR RAJOUTER UNE CONTINUATION A LA FIN DE L'INSTANT PRESENT
    pub fn on_end_of_instant(&mut self, c: Box<Continuation<()>>) {
    self.end_of_instant.push(c)
  }
}
