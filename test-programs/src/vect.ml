let run () = 
    if Array.unsafe_get [| 1; 2 |] 1 <> 2 then failwith "getvectitem";
    if Array.length [| 1; 2 |] <> 2 then failwith "vectlength";
    let x = [| 1; 2 |] in
    Array.unsafe_set x 1 3;
    if x.(1) <> 3 then failwith "setvectitem";
    ()

let () = run ()