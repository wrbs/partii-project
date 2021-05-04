let add a b = a + b

let clamp h l (x : int) =
  if h < x then
    h
  else if l > x then
    l
  else
    x