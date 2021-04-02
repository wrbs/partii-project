type combined =
  | A0
  | A1
  | A2
  | B0 of int
  | B1 of int
  | B2 of int

type only_nullary =
  | C0
  | C1
  | C2

type only_unary =
  | D0 of int
  | D1 of int
  | D2 of int

let f = function
  | A0 -> 0
  | A1 -> 1
  | A2 -> 2
  | B0 _ -> 3
  | B1 _ -> 4
  | B2 _ -> 5

let g = function
  | C0 -> 0
  | C1 -> 1
  | C2 -> 2

let h = function
  | D0 _ -> 3
  | D1 _ -> 4
  | D2 _ -> 5

let x = (
  f A0,
  f A1,
  f A2,
  f (B0 0),
  f (B1 0),
  f (B2 0),
  g C0,
  g C1,
  g C2,
  h (D0 0),
  h (D0 1),
  h (D0 2)
)