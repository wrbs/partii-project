let () =
    let _ = 1 + 2 in
    let _ = 1 - 2 in
    let _ = 1 * 2 in
    let _ = 1 / 2 in
    let _ = 1 mod 2 in
    let _ = 1 lor 2 in
    let _ = 1 land 2 in
    let _ = 1 lxor 2 in
    let _ = 1 lsl 2 in
    let _ = 1 lsr 2 in
    let _ = 1 asr 2 in
    let a = 1 in
    let _ = a = 2 in
    let _ = a <> 2 in
    let _ = a > 2 in
    let b = a < 2 in
    let _ = a >= 2 in
    let _ = a <= 2 in
    let _ = -a in
    let _ = if b then 10 else -10 in
    ()

let t x =
    if x > 2 then
        5
    else
        7


let _ = t 5