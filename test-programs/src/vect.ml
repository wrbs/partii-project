let () = if Array.unsafe_get [| 1; 2 |] 1 <> 2 then raise Not_found

let () = if Array.length [| 1; 2 |] <> 2 then raise Not_found

let () =
    let x = [| 1; 2 |] in
    Array.unsafe_set x 0 3;
    if x.(0) <> 3 then raise Not_found