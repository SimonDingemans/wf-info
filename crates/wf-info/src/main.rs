use shared::AppContext;

fn main() {
    let context = AppContext::new("wf-info");

    application::run(&context);
    overlay::run(&context);
}
