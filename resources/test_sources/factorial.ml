let rec fact = function
    | 0 -> 1
    | n -> n * fact (n - 1)

let () =
    fact 5 |> string_of_int |> print_endline
