(* This test various kind of variables in mutually recursive functions *)

let show_int x =
  print_int x; print_newline ()

let mk_mutrec x y =
  let rec f1 n a b =
    if n = 0 then () else
    print_endline "f1";
    show_int x;
    show_int y;
    show_int a;
    show_int b;
    f1 (n - 1) a b;
    f2 (n - 1) a b;
    f3 (n - 1) a b
  and f2 n a b =
    if n = 0 then () else
    print_endline "f2";
    show_int x;
    show_int y;
    show_int a;
    show_int b;
    f1 (n - 1) a b;
    f2 (n - 1) a b;
    f3 (n - 1) a b
  and f3 n a b =
    if n = 0 then () else
    print_endline "f3";
    show_int x;
    show_int y;
    show_int a;
    show_int b;
    f1 (n - 1) a b;
    f2 (n - 1) a b;
    f3 (n - 1) a b in
  f1 2 3 4;
  f2 2 3 4;
  f3 2 3 4
