open Printf

type test =
    | UF of (float -> float)
    | BF of (float -> float -> float)

let nums = [0.0; 1.0; -1.0; 3.2; -578.4; Float.infinity; Float.nan; Float.neg_infinity]

let tests = [
    (* caml_neg_float *)
    "neg", UF Float.neg;
    (* caml_abs_float *)
    (* caml_add_float *)
    "(+.)", BF (+.);

    (* caml_sub_float *)
    "(-.)", BF (-.);

    (* caml_mul_float *)
    "( *.)", BF ( *.);

    (* caml_div_float *)
    "(/.)", BF (/.);

    (* caml_exp_float *)
    (* caml_trunc_float *)
    (* caml_round_float *)
    (* caml_floor_float *)
    (* caml_nextafter_float *)
    (* caml_fma_float *)
    (* caml_fmod_float *)
    (* caml_frexp_float *)
    (* caml_ldexp_float *)
    (* caml_log_float *)
    (* caml_log10_float *)
    (* caml_modf_float *)
    (* caml_sqrt_float *)
    (* caml_power_float *)
    (* caml_sin_float *)
    (* caml_sinh_float *)
    (* caml_cos_float *)
    (* caml_cosh_float *)
    (* caml_tan_float *)
    (* caml_tanh_float *)
    (* caml_asin_float *)
    (* caml_acos_float *)
    (* caml_atan_float *)
    (* caml_atan2_float *)
    (* caml_ceil_float *)
    (* caml_hypot_float *)
    (* caml_expm1_float *)
    (* caml_log1p_float *)
    (* caml_copysign_float *)
    (* caml_signbit *)
    (* caml_signbit_float *)
    (* caml_neq_float *)
    (* caml_eq_float *)
    (* caml_le_float *)
    (* caml_lt_float *)
    (* caml_ge_float *)
    (* caml_gt_float *)
    (* caml_float_compare *)
    (* caml_classify_float *)
]

let unaries = List.filter_map (fun (n, t) -> match t with
    | UF f -> Some (n, f)
    | _ -> None
) tests

let binaries = List.filter_map (fun (n, t) -> match t with
    | BF f -> Some (n, f)
    | _ -> None
) tests

let unary_tests x =
    printf "Unary for %f\n" x;
    List.iter (fun (n, f) ->
        printf "%s %f = %f\n" n x (f x)
    ) unaries;
    print_newline()

let binary_tests x y =
    printf "Binary for %f, %f\n" x y;
    List.iter (fun (n, f) ->
        printf "%s %f %f = %f\n" n x y (f x y)
    ) binaries;
    print_newline()

let rec allpairs x = function
    | [] -> []
    | (v::vs) -> (x, v) :: allpairs x vs

let pairs_of ns =
    let rec aux = function
        | [] -> []
        | (x::xs) -> allpairs x ns @ aux xs in
    aux ns

let run_tests () =
    List.iter unary_tests nums;
    List.iter (fun (x, y) -> binary_tests x y) (pairs_of nums)

let () = run_tests ()