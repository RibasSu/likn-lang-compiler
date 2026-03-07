use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let code = likn_lang_compiler::run(&args);
    if code != 0 {
        std::process::exit(code);
    }
}
