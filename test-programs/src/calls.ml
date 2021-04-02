let f1 a0 = ()
let f2 a0 a1 = ()
let f3 a0 a1 a2 = ()
let f4 a0 a1 a2 a3 = ()
let f5 a0 a1 a2 a3 a4 = ()

let run () =
    f1 0;
    f2 0 1;
    f3 0 1 2;
    f4 0 1 2 3;
    f5 0 1 2 3 4

let g1 x = 
    let f y =
        Printf.printf "%d %d" x y in
    f 3

let g2 x = 
    let rec f y =
        Printf.printf "%d %d" x y in
    f 3

let () = g1 2; g2 2

let _ = run ()
