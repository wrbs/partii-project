(* This aims to trigger the GC during a frame where there's a big stack map *)
let g () = 1

let rec f n acc =
  if n = 0 then
    acc
  else
    let to_add = (
      n,   g(), g(), g(), g(), g(), g(), g(), g(), g(),
      g(), g(), g(), g(), g(), g(), g(), g(), g(), g(),
      g(), g(), g(), g(), g(), g(), g(), g(), g(), g(),
      g(), g(), g(), g(), g(), g(), g(), g(), g(), g(),
      g(), g(), g(), g(), g(), g(), g(), g(), g(), g(),
      g(), g(), g(), g(), g(), g(), g(), g(), g(), g(),
      g(), g(), g(), g(), g(), g(), g(), g(), g(), g(),
      g(), g(), g(), g(), g(), g(), g(), g(), g(), g(),
      g(), g(), g(), g(), g(), g(), g(), g(), g(), g(),
      g(), g(), g(), g(), g(), g(), g(), g(), g(), g()
    ) in
    f (n - 1) (to_add :: acc)

let getfst (
    n, _, _, _, _, _, _, _, _, _,
    _, _, _, _, _, _, _, _, _, _,
    _, _, _, _, _, _, _, _, _, _,
    _, _, _, _, _, _, _, _, _, _,
    _, _, _, _, _, _, _, _, _, _,
    _, _, _, _, _, _, _, _, _, _,
    _, _, _, _, _, _, _, _, _, _,
    _, _, _, _, _, _, _, _, _, _,
    _, _, _, _, _, _, _, _, _, _,
    _, _, _, _, _, _, _, _, _, _
  ) = n

let step acc v = match acc with
  | (n, false) -> (n, false)
  | (n, true) -> (n + 1, n = getfst v)

let do_for steps = 
  let res = f steps [] in
  let (n, ok) = List.fold_left step (1, true) res in
  if ok then
    print_endline "ok"
  else
    failwith "problem"

let () = do_for 10000000