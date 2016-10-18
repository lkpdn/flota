use nickel::{Nickel, HttpRouter, MediaType};
use nickel::Mountable;
use serde_json::value::ToJson;
use ::flota::config::Config;
use ::util::errors::*;

pub fn run() -> Result<i32> {
    let mut server = Nickel::new();
    let mut router = Nickel::router();

    // [GET] /configs
    router.get("/configs", middleware! {|_, mut res|
        res.set(MediaType::Json);
        //Config::get_all().unwrap().to_json().as_str().unwrap()
        unimplemented!()
    });
    // [GET] /configs/:id
    server.get("/configs/:id", middleware! {|_, mut res|
        res.set(MediaType::Json);
        unimplemented!()
    });
    // [GET] /clusters
    //
    // return:
    // [{"id":NUM, "name":STRING},
    //  {"id":NUM, "name":STRING},...]
    // -------------------------------
    router.get("/clusters", middleware! {|_, mut res|
        unimplemented!()
    });
    // [GET] /clusters/:id
    //
    // returns:
    // {"id":NUM, "name":STRING, "watchpoints":ARRAY(NUM)}, "histories":ARRAY(NUM)}

    // [DELETE] /clusters/:id
    //
    // returns:
    // ()

    // [GET] /clusters/:id/watchpoints
    //
    // returns:
    // [{"id":NUM}...]

    // [POST] /clusters/:id/watchpoints
    //
    // params:
    // {"type":ENUM, "ident":ENUM(STRUCT)}
    // returns:
    // ()

    // [GET] /clusters/:id/watchpoints/:id
    //
    // returns:
    // {"id":NUM, "type":ENUM, "ident":ENUM(STRUCT), "histories":ARRAY(NUM)}

    // [GET] /clusters/:id/histories
    //
    // returns:
    // [{"id":NUM},...]

    // [DELETE] /clusters/:id/histories
    //
    // returns:
    // ()

    // [GET] /clusters/:id/histories/:id
    //
    // returns:
    // {"id":NUM, "config_id":NUM, "results":ARRAY(STRUCT), "passed":bool}

    // [GET] /clusters/:id/hosts
    //
    // returns:
    // [{"id",NUM},...]

    // [GET] /clusters/:id/hosts/:id
    //
    // return:
    // {"id": NUM, "name":STRING, "state":ENUM, "histories": ARRAY(NUM)}

    // [GET] /clusters/:id/hosts/:id/histories
    //
    // returns:
    // {"id": NUM, "config_id":NUM, "results":ARRAY(STRUCT), "passed":bool}

    server.mount("/api/v1/", router);
    let listening = server.listen("127.0.0.1:4472")
                          .expect("Failed to launch server");
    println!("Listening on: {:?}", listening.socket());
    Ok(0)
}
