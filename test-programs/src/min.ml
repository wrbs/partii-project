(* Calls min a 15 times to get it to be considered hot *)

let cases = [
  0, 1;
  1, 0;
  1, 2;
  2, 1;
  -1, 1;
  1, -1;
  0, 1;
  1, 0;
  1, 2;
  2, 1;
  -1, 1;
  1, -1;
  0, 1;
  1, 0;
  1, 2;
  2, 1;
  -1, 1;
  1, -1;
]

let () = List.iter (fun (a, b) -> Printf.printf "min %d %d = %d\n" a b (min a b)) cases