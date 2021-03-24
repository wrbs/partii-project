(* Does a variety of parital application/tail call scenarios to test handling of extra args *)

(* Partial application *)
let f1 a b c = Printf.printf "%d %d %d\n" a b c

let f2 = f1 1
let f3 = f1 2 3
let f4 = f2 4

let () =
  f1 1 2 3;
  f2 1 2;
  f3 1;
  f4 1

let f5 a = f4 a; f1

let f6 a = f5 a a a

(* Auto tail-calls with extra args *)
let () =
  f5 1 2 3 4;
  f6 1 1