let doc =
"
A minimal multicore OCaml implementation of an Evolutionary Algorithm.
Per Kristian Lehre <p.k.lehre@cs.bham.ac.uk>
May 6th, 2020
"

type bitstring = bool array
type individual = { chromosome : bitstring; fitness : float }
type fitness_function = bitstring -> float
type population = individual array

let chromosome_length = Array.length
let population_size = Array.length

let chromosome { chromosome=x; fitness=_ } = x
let evaluate f x = { chromosome=x; fitness=(f x) }

let fittest x y = if x.fitness > y.fitness then x else y
let pop_fittest pop = Array.fold_left fittest pop.(0) pop
let state = Random.State.make_self_init ()

let random_bitstring n =
  Array.init n (fun _ -> Random.State.bool state)

let random_individual n f =
  evaluate f (random_bitstring n)

(* Onemax is a simple fitness function. *)
let add_bit x b = if b then x +. 1.0 else x
let onemax : fitness_function = Array.fold_left add_bit 0.0

(* Choose fittest out of k uniformly sampled individuals *)
let rec k_tournament k pop =
  let x = pop.(Random.State.int state (population_size pop)) in
  if k <= 1 then x
  else
    let y = k_tournament (k-1) pop in
    fittest x y

let flip_coin p = (Random.State.float state 1.0) < p

let xor a b = if a then not b else b
(* Naive Theta(n) implementation of mutation. *)
let mutate chi x =
  let n = chromosome_length x in
  let p = chi /. (float n) in
  Array.map (fun b -> xor b (flip_coin p)) x

(* Command line arguments *)


let runtime = 100000

let chi = 0.4

let lambda = try int_of_string Sys.argv.(2) with _ -> 1000

let k = 2

let n = try int_of_string Sys.argv.(1) with _ -> 1000

let init n f =
  let a = Array.init n (fun _ -> f ()) in
  a


let evolutionary_algorithm

    runtime       (* Runtime budget  *)
    lambda        (* Population size *)
    k             (* Tournament size *)
    chi           (* Mutation rate   *)
    fitness       (* Fitness function *)
    n             (* Problem instance size n *)

  =

  (* let adam = random_individual n fitness in *)
  let init_pop = init  in
  let pop0 = init_pop lambda (fun _ -> random_individual n fitness) in


  let rec generation time pop =
    let next_pop =
      init_pop lambda
	(fun _ ->
             (k_tournament k pop)
          |> chromosome
          |> (mutate chi)
          |> (evaluate fitness))
    in
    if time * lambda > runtime then
      (
       Printf.printf "fittest: %f\n"
         (pop_fittest next_pop).fitness)
    else
      generation (time+1) next_pop
  in generation 0 pop0

let ea () = evolutionary_algorithm runtime lambda k chi onemax n

let () =
  ea ()
