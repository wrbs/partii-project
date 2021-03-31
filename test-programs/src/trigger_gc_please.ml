let g () = 1

let rec f n acc =
  if n = 0 then
    acc
  else
    let to_add = (
      g(), g(), g(), g(), g(), g(), g(), g(), g(), g(),
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

let _ = f 1000000 []