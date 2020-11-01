let x =
    let x = ref 32 in
    incr x;
    if !x <> 33 then raise Not_found;
    x