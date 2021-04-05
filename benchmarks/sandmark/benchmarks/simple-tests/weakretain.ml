type a_mutable_record = { mutable an_int : int; a_string : string ; a_float: float }

let weak_percent = int_of_string Sys.argv.(1)
let weak_array_size = int_of_string Sys.argv.(2)

let () = Random.init 42

let weak_array = Weak.create weak_array_size

let () =
  let v = ref 0 in
  for i = 1 to 10_000_000 do
    let t = { an_int = i; a_string = "foo"; a_float = 0.0 } in
    if Random.int 100 < weak_percent then
      begin
        Weak.set weak_array !v (Some t);
        v := (!v + 1) mod weak_array_size
      end
    else
      ignore(Sys.opaque_identity ref t)
  done
