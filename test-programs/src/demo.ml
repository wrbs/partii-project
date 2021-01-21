module StringMap = Map.Make(String)

module IntMap = Map.Make(struct
    type t = int
    let compare = compare
end)


let while_loop () =
    let i = ref 0 in
    while !i < 10 do
        Printf.printf "%d\n" !i;
        i := (!i + 1)
    done

let looprec () =
    let rec aux i =
        if i >= 10 then
            ()
        else
            (Printf.printf "%d\n" i;
            aux (i + 1)) in
    aux 0

let () =
    while_loop ();
    looprec ()