type t = { mutable a : float; mutable b : float }

let () = if { a = 0.1; b = 0.2 }.a <> 0.1 then raise Not_found

let () =
    let a = { a = 0.1; b = 0.2 } in
    let () = a.b <- 3.5 in
    if a.b <> 3.5 then raise Not_found
