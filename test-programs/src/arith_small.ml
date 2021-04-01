let nums = [0; 1; -1; Int.max_int; Int.min_int]

let ops = [
    ( + );
    ( - );
    ( * );
    ( / );
    ( mod );
    ( lor );
    ( land );
    ( lxor );
    ( lsl );
    ( lsr );
    ( asr );
]

let preds = [
    (fun (x : int) y -> x = y);
    (fun (x : int) y -> x <> y);
    (fun (x : int) y -> x > y);
    (fun (x : int) y -> x < y);
    (fun (x : int) y -> x >= y);
    (fun (x : int) y -> x <= y);
]

let do_op_test (a, b) f = try
    let _ = f a b in ()
with Division_by_zero -> ()

let do_pred_test (a, b) f = let _ =  f a b in ()

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

let do_not x = not x

let () =
    let _ = do_not true in
    let _ = do_not false in
    let _ = List.map (fun x -> -x) nums in
    List.iter do_tests (pairs_of nums)
