open Printf

let () =
    let a = Array.init 20 (fun n -> float_of_int n) in
    let b = Array.get a 15 in
    printf "%f\n" b