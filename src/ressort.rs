use continuation::Continuation;
use runtime::Runtime;
use process::Process;
use process::ProcessMut;
use signal_runtime_val::Signal;
use signal_runtime_val::MCSignal;
use signal_runtime_val::MCSignalIO;
use process::Value;

fn main(){
    let mut runtime = Runtime::new();
    //Masse du premier point
    let m1 = 10.;
    //Masse du second point
    let m2 = 1.;
    //Initialisation du signal qui contiendra les positions du point 1
    let s_pos_p1 = MCSignal::new(MCSignalIO::new(0.));
    //Initialisation du signal qui contiendra les vitesses du point 1
    let s_vit_p1 = MCSignal::new(MCSignalIO::new(0.));
    //Initialisation du signal qui contiendra les positions du point 2
    let s_pos_p2 = MCSignal::new(MCSignalIO::new(0.));
    //Initialisation du signal qui contiendra les vitesses du point 2
    let s_vit_p2 = MCSignal::new(MCSignalIO::new(0.));
    //Un processus renvoyant la position du point 1
    let pos_p1 = s_pos_p1.await_in();
    //Un processus renvoyant la vitesse du point 1
    let vit_p1 = s_vit_p1.await_in();
    //Un processus renvoyant la position du point 2
    let pos_p2 = s_pos_p2.await_in();
    //Un processus renvoyant la vitesse du point 2
    let vit_p2 = s_vit_p2.await_in();
    //Constante de raideur du ressort
    let k = 10.;
    //Longueur à l'équilibre du ressort
    let l0 = 1.;
    //durée entre deux instants
    let dt = 0.01;
    //Processus renvoyant la position d'origine du point 1
    let pos_p1_0 = Value::new(0.);
    //Processus renvoyant la vitesse d'origine du point 1
    let vit_p1_0 = Value::new(0.);
    //Processus renvoyant la position d'origine du point 2
    let pos_p2_0 = Value::new(2.);
    //Processus renvoyant la vitesse d'origine du point 1
    let vit_p2_0 = Value::new(0.);

    //Un processus qui lit les deux valeurs des positions et le affiche à l'ecran
    let printer = ((pos_p1).join(pos_p2)).map(|(x1, x2)|{
        println!("{}", x1);
        println!("{}", x2);
    } );

    //Un processus qui actualise la valeur de v1
    let pos_p1_update = s_pos_p1.emit(((s_pos_p1.await_in()).join(s_vit_p1.await_in())).map(|(x1, v1)|{
        x1 + v1 * dt
    } ));

    //Un processus qui actualise la valeur de v2
    /*let pos_p2_update = s_pos_p1.emit(((s_pos_p2.await_in()).join(s_vit_p2.await_in())).map(|x2: fsize, v2: fsize|{
        x2 + v2 * dt
    } ))*/

    //Un processus qui renvoit l'acceleration du point 1
    /*let acc_p1 = ((s_pos_p1.await_in()).join(s_pos_p2.await_in())).map(|x1: fsize, x2: fsize|{
        (x2-x1-l0)*k/m1
    } )*/

    //Un processus qui renvoit l'acceleration du point 2
    /*let acc_p2 = ((s_pos_p1.await_in()).join(s_pos_p2.await_in())).map(|x1: fsize, x2: fsize|{
        -(x2-x1-l0)*k/m2
    } )*/

    //Un processus qui actualise la valeur de v1
    /*let vit_p1_update = s_vit_p1.emit(((s_vit_p1.await_in()).join(acc_p1).map(|v1: fsize, a1: fsize|{
        v1 + a1 * dt
    } ))*/

    //Un processus qui actualise la valeur de v2
    /*let vit_p2_update = s_vit_p2.emit(((s_vit_p2.await_in()).join(acc_p2).map(|v2: fsize, a2: fsize|{
        v2 + a2 * dt
    } ))*/

    //Un processus qui update p1
    //let p1_update = (pos_p1_update.join(vit_p1_update)).map(|(), ()|{})

    //Un processus qui update p2
    //let p2_update = (pos_p2_update.join(vit_p2_update)).map(|(), ()|{})

    //Un processus qui update le systeme physique
    //let update = p1_update.join(p2_update).map(|(), ()|{})

    //Le processus core qui devra être répété
    //let core = update.join(printer).map(|(), ()| {(core, Continue)})

    //let core_multiple = core.while()

    //s_pos_p1.emit(pos_p1_0).call(|()|{});
    //s_vit_p1.emit(vit_p1_0).call(|()|{});
    //s_pos_p2.emit(pos_p2_0).call(|()|{});
    //s_vit_p2.emit(vit_p2_0).call(|()|{});

    //core_multiple.call_mut(runtime, |()|{})
    ()
}
