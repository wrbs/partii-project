open Printf

let test_string_get () =
    let s = "Hello, world!" in
    let c = String.unsafe_get s 5 in
    printf "%c\n" c

let print_bytes x =
    x
    |> Bytes.to_seq
    |> Seq.map int_of_char
    |> Seq.iter (printf "%d ")

let test_bytes () =
    let x = Bytes.init 20 char_of_int in
    let () = Bytes.unsafe_set x 5 'a' in
    let () = print_bytes x in
    print_newline ()

let () =
    test_string_get ();
    test_bytes ()
