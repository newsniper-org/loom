fn main() {
    let source = std::fs::read_to_string(
        std::env::args().nth(1).expect("usage: print_demo <path.tsx>"),
    )
    .unwrap();
    println!("{}", loom_compiler::compile_tsx_to_rust(&source));
}
