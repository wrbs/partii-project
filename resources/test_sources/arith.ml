let nums = [0; 1; -1; 3; 15; -7; 21; Int.max_int; Int.min_int]

let ops = [("+", (+)); ("-", (-)); ("*", ( * )); ("/", ( / )); ("mod", ( mod ))]

let preds = [("==", (==)); ("!=", (!=)); (">", (>)); ("<", (<)); (">=", (>=)); ("<=", (<=))]

open Printf

let do_op_test (a, b) (op, f) = try 
    printf "%d %s %d = %d\n" a op b (f a b)
with Division_by_zero -> printf "%d %s %d = Division_by_zero\n" a op b

let do_pred_test (a, b) (op, f) = printf "%d %s %d = %s\n" a op b (f a b |> string_of_bool)

let do_tests ns =
    List.iter (do_op_test ns) ops;
    List.iter (do_pred_test ns) preds

let rec allpairs x = function
    | [] -> []
    | (v::vs) -> (x, v) :: allpairs x vs

let pairs_of ns =
    let rec aux = function
        | [] -> []
        | (x::xs) -> allpairs x ns @ aux xs in
    aux ns

let () =
    List.iter do_tests (pairs_of nums)

