effect Select : 'a list -> 'a

let rec filter p = function
  | [] -> []
  | x :: xs ->
    if p x then (x :: filter p xs) else filter p xs

let rec forall p = function
  | [] -> true
  | x :: xs -> if p x then forall p xs else false

let no_attack (x,y) (x',y') =
  x <> x' && y <> y' && abs (x - x') <> abs (y - y')

let available x qs l =
  filter (fun y -> forall (no_attack (x,y)) qs) l

let find_solution n =
  try
    let l = ref [] in
    for i = n downto 1 do
      l := i::!l;
    done;
    let rec place x qs =
      if x = n+1 then Some qs else
        let y = perform @@ Select (available x qs !l) in
        place (x+1) ((x, y) :: qs)
    in place 1 []
  with
  | effect (Select lst) k ->
      let rec loop = function
        | [] -> None
        | x::xs ->
            match continue (Obj.clone_continuation k) x with
            | None -> loop xs
            | Some x -> Some x
      in loop lst

let iterations = try int_of_string Sys.argv.(1) with _ -> 100
let size = try int_of_string Sys.argv.(2) with _ -> 8


let () =
  for _i = 1 to iterations do
    ignore(find_solution size)
  done
