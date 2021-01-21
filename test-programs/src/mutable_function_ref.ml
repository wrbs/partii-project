module Thing = struct

    let log_handler = ref (fun _ -> ())

    let set_log_handler f = log_handler := f
    let log s = (!log_handler) s
end


let () =
    Thing.log "started";
    Thing.set_log_handler (Printf.printf "Log: %s");
    Thing.log "ended"
