pub mod nk_shell_cmd;
use alloc::{string::ToString, vec::Vec};
use crate::utils::print_to_vc;

pub fn nk_rust_example(a: i32, b: i32) -> i32 {
    let test_s = "Hello, this is the Rust example module!\n";
    print_to_vc(test_s);

    let sum = (a + b).to_string();
    let sum_str = sum.as_str();
    print_to_vc(sum_str);
    print_to_vc("\n");

    let mut vec = Vec::new();
    for i in 0..a {
        vec.push(i);
        print_to_vc(i.to_string().as_str());
        print_to_vc("\n");
    }

    a + b
}
