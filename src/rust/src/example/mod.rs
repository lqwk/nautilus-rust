use crate::kernel::shell::register_shell_command;
use alloc::vec::Vec;

use crate::kernel::print::vc_println;

fn example(a: i32, b: i32) -> i32 {
    vc_println!("Hello, this is the Rust example module!");
    vc_println!("{} + {} = {}", a, b, a + b);

    let mut vec = Vec::new();
    for i in 0..a {
        vc_println!("Pushing: {}", i);
        vec.push(i);
    }

    vc_println!("vec = {:?}", &vec);

    a + b
}

register_shell_command!("rust", "rust", |_, _| {
    vc_println!("Entered Rust code.");
    example(8, 1);
});
