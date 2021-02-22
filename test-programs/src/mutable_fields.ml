type thing = {
    mutable name: string;
    mutable age: int;
}

let show_thing {name; age} =
    Printf.printf "name='%s' age=%d\n" name age

let () =
    let t = { name = "Bob"; age = 71 } in
    show_thing t;
    t.name <- "Alice";
    t.age <- 12;
    show_thing t