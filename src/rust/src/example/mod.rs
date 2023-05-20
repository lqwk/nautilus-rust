use crate::prelude::*;

fn example(a: i32, b: i32) -> i32 {
    vc_println!("Hello, this is the Rust example module!");
    vc_println!("{} + {} = {}", a, b, a + b);

    let mut vec = Vec::new();
    for i in 0..a {
        vc_println!("Pushing: {i}");
        vec.push(i);
    }

    vc_println!("vec = {:?}", &vec);

    a + b
}

register_shell_command!("rust", "rust", |_| {
    debug!("Entered Rust code.");
    example(8, 1);
    debug!("Exiting Rust code.");
    Ok(())
});
