let list_length = int_of_string Sys.argv.(1)
let percent_retain = int_of_string Sys.argv.(2)
let iterations = 25000

let () = Random.init 42

let rec init_list acc i n =
  if i >= n then acc
    else init_list ((ref (0,0)) :: acc) (i+1) n

let retain_list = ref (init_list [] 0 list_length)

let allocate () =
  for _ = 0 to 1000 do
    let v = (0,0) in
    if Random.int 100 < percent_retain then
      retain_list := (ref v) :: (List.tl !retain_list)
  else
    ignore(Sys.opaque_identity ref v)
  done

let () = for _ = 0 to iterations do
  allocate()
  done
