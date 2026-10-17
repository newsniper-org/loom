fn main() {
    let path = std::env::args().nth(1).expect("usage: print_demo <path.tsx>");
    let source = std::fs::read_to_string(&path).unwrap();
    match loom_compiler::compile_tsx_to_rust(&path, &source) {
        Ok(c) => {
            if !c.warnings.is_empty() {
                eprint!("{}", c.warnings.render());
            }
            println!("{}", c.rust);
        }
        Err(d) => {
            eprint!("{}", d.render());
            std::process::exit(1);
        }
    }
}
